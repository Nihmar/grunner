//! Command handling logic for Grunner
//!
//! This module extracts command handling logic from the AppListModel,
//! separating concerns between data management and command execution.
//!
//! It handles colon-prefixed commands like `:ob`, `:f`, `:sh`, etc.

use crate::app_mode::ActiveMode;
use crate::model::items::CommandItem;
use crate::model::list_model::AppListModel;
use gtk4::prelude::ListModelExt;
use log::{debug, error};
use std::path::PathBuf;

/// Parse a colon-prefixed command into command name and argument
///
/// Colon commands follow the format ":command argument" where:
/// - `:` is the command prefix
/// - `command` is the command name (e.g., "f", "ob", "s")
/// - `argument` is the optional search argument (trimmed)
///
/// # Examples
/// - `":f foo"` → `("f", "foo")`
/// - `":ob"` → `("ob", "")`
/// - `":obg pattern"` → `("obg", "pattern")`
fn parse_colon_command(query: &str) -> (&str, &str) {
    let rest = &query[1..];
    match rest.split_once(' ') {
        Some((cmd, arg)) => (cmd, arg.trim()),
        None => (rest, ""),
    }
}

/// Command handler that operates on an AppListModel instance
pub struct CommandHandler<'a> {
    model: &'a AppListModel,
}

impl<'a> CommandHandler<'a> {
    /// Create a new command handler for the given model
    pub fn new(model: &'a AppListModel) -> Self {
        Self { model }
    }

    /// Handle colon-prefixed commands by routing to appropriate handlers
    pub fn handle_colon_command(&self, query: &str) {
        let (cmd_part, arg) = parse_colon_command(query);
        debug!("handle_colon_command: query='{query}', cmd_part='{cmd_part}', arg='{arg}'");
        debug!("Active mode: {:?}", self.model.active_mode.get());

        match cmd_part {
            "ob" | "obg" => self.handle_obsidian(cmd_part, arg),
            "f" => self.handle_file_search(arg),
            "fg" => self.handle_file_grep(arg),
            "sh" => {
                debug!("Calling handle_sh with arg: '{arg}'");
                self.handle_sh(arg);
            }
            _ => {
                if !cmd_part.is_empty() {
                    self.show_error_item(format!("Unknown command: :{cmd_part}"));
                }
            }
        }
    }

    /// Handle Obsidian search modes triggered by `:ob` and `:obg` commands
    fn handle_obsidian(&self, cmd_name: &str, arg: &str) {
        let Some(vault_path) = self.validated_vault_path() else {
            return;
        };
        let vault_str = vault_path.to_string_lossy().into_owned();

        let (mode, runner): (ActiveMode, Box<dyn FnOnce()>) = match (cmd_name, arg.is_empty()) {
            ("ob", true) => {
                // Empty :ob command - show Obsidian action mode
                self.model.active_mode.set(ActiveMode::ObsidianAction);
                self.clear_store();
                return;
            }
            ("obg", true) => {
                // Empty :obg command - show Obsidian grep mode
                self.model.active_mode.set(ActiveMode::ObsidianGrep);
                self.clear_store();
                return;
            }
            ("ob", false) => {
                // :ob with argument - file search in vault
                let arg = arg.to_string();
                let model_clone = self.model.clone();
                (
                    ActiveMode::ObsidianFile,
                    Box::new(move || model_clone.run_find_in_vault(PathBuf::from(vault_str), &arg)),
                )
            }
            ("obg", false) => {
                // :obg with argument - ripgrep (with grep fallback) search in vault
                let arg = arg.to_string();
                let model_clone = self.model.clone();
                (
                    ActiveMode::ObsidianGrep,
                    Box::new(move || model_clone.run_rg_in_vault(PathBuf::from(vault_str), &arg)),
                )
            }
            _ => {
                // Should never happen as cmd_name comes from known commands
                error!("Unexpected obsidian command: {cmd_name}");
                return;
            }
        };

        self.model.active_mode.set(mode);
        self.model.task_gen.set(self.model.task_gen.get() + 1);
        self.model.schedule_command(runner);
    }

    fn handle_file_search(&self, arg: &str) {
        if arg.is_empty() {
            self.clear_store();
            return;
        }

        let current_gen = self.model.task_gen.get() + 1;
        self.model.task_gen.set(current_gen);
        let arg = arg.to_string();
        let model_clone = self.model.clone();
        self.model.schedule_command(move || {
            if model_clone.task_gen.get() == current_gen {
                model_clone.run_file_search(&arg);
            }
        });
    }

    fn handle_file_grep(&self, arg: &str) {
        if arg.is_empty() {
            self.clear_store();
            return;
        }

        let current_gen = self.model.task_gen.get() + 1;
        self.model.task_gen.set(current_gen);
        let arg = arg.to_string();
        let model_clone = self.model.clone();
        self.model.schedule_command(move || {
            if model_clone.task_gen.get() == current_gen {
                model_clone.run_file_grep(&arg);
            }
        });
    }

    pub(crate) fn handle_sh(&self, arg: &str) {
        use crate::providers::CommandProvider;
        debug!("Setting active_mode to CustomScript");
        self.model.active_mode.set(ActiveMode::CustomScript);
        self.clear_store();

        // Get filtered commands using the CommandProvider trait
        let filtered_commands = self.model.get_commands(arg);

        debug!(
            "handle_sh called with arg: '{}', commands count: {}",
            arg,
            filtered_commands.len()
        );

        // Add filtered commands to store
        for cmd in filtered_commands {
            // Format as "Name | Command" for display
            let item_str = format!("{} | {}", cmd.name, cmd.command);
            self.model.store.append(&CommandItem::new_with_options(
                item_str,
                cmd.working_dir.clone(),
                cmd.keep_open,
            ));
        }

        // If user typed a command that doesn't match saved ones, add "Run: ..." option
        if !arg.is_empty() {
            let run_item_str = format!("Run: {arg}");
            // Custom commands default to keep_open=true
            self.model
                .store
                .append(&CommandItem::new_with_options(run_item_str, None, true));
        }

        debug!("Final store count: {}", self.model.store.n_items());
        debug!("Active mode is now: {:?}", self.model.active_mode.get());
    }

    /// Validate the Obsidian vault path from configuration
    ///
    /// Returns `Some(PathBuf)` if vault is configured and exists,
    /// otherwise shows an error and returns `None`.
    fn validated_vault_path(&self) -> Option<std::path::PathBuf> {
        use crate::utils::expand_home;
        let obs_cfg = if let Some(c) = &self.model.obsidian_cfg {
            c.clone()
        } else {
            self.show_error_item("Obsidian not configured - edit config");
            return None;
        };
        let vault_path = expand_home(&obs_cfg.vault);
        if !vault_path.exists() {
            self.show_error_item(format!(
                "Vault path does not exist: {}",
                vault_path.display()
            ));
            return None;
        }
        Some(vault_path)
    }

    /// Display an error message as the only item in the list
    ///
    /// Used for configuration errors, missing dependencies, etc.
    fn show_error_item(&self, msg: impl Into<String>) {
        self.model.store.remove_all();
        self.model.store.append(&CommandItem::new(msg.into()));
        self.model.selection.set_selected(0);
    }

    /// Clear all items from the list store and reset selection
    fn clear_store(&self) {
        self.model.store.remove_all();
        self.model
            .selection
            .set_selected(gtk4::INVALID_LIST_POSITION);
    }
}
