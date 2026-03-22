//! Command handling logic for Grunner
//!
//! This module extracts command handling logic from the `AppListModel`,
//! separating concerns between data management and command execution.
//!
//! It handles colon-prefixed commands like `:ob`, `:f`, `:sh`, etc.
//!
//! ## Design
//!
//! `CommandHandler` is generic over `T: CommandSink`, allowing it to be tested
//! with mock implementations. For file-search operations that require the concrete
//! `AppListModel` type, a companion `AppCommandHandler` type alias and extension
//! trait are provided.

use crate::app_mode::ActiveMode;
use crate::model::items::CommandItem;
use crate::model::list_model::{AppListModel, CommandSink};

use log::debug;
use std::path::{Path, PathBuf};

/// Parse a colon-prefixed command into command name and argument
pub(crate) fn parse_colon_command(query: &str) -> (&str, &str) {
    let rest = &query[1..];
    match rest.split_once(' ') {
        Some((cmd, arg)) => (cmd, arg.trim()),
        None => (rest, ""),
    }
}

/// Command handler generic over any `CommandSink` implementation.
///
/// Provides the `:sh` command handler that works purely through the trait.
/// File-search commands (`:ob`, `:f`, `:fg`) require the concrete
/// [`AppCommandHandler`] wrapper because `file_search::run_*` needs `&AppListModel`.
pub struct CommandHandler<T: CommandSink> {
    pub model: T,
}

impl<T: CommandSink> CommandHandler<T> {
    pub fn new(model: T) -> Self {
        Self { model }
    }

    /// Handle `:sh` — list custom scripts and a "Run:" fallback.
    pub fn handle_sh(&self, arg: &str) {
        debug!("Setting active_mode to CustomScript");
        self.model.set_mode(ActiveMode::CustomScript);
        self.clear_store();

        let filtered = self.model.get_commands(arg);

        debug!("handle_sh: arg='{arg}', commands={}", filtered.len());

        for cmd in &filtered {
            let item_str = format!("{} | {}", cmd.name, cmd.command);
            self.model.push(&CommandItem::new_with_options(
                item_str,
                cmd.working_dir.clone(),
                cmd.keep_open,
            ));
        }

        if !arg.is_empty() {
            self.model.push(&CommandItem::new_with_options(
                format!("Run: {arg}"),
                None,
                true,
            ));
        }

        debug!("Final store count: {}", self.model.count());
    }

    fn show_error(&self, msg: impl Into<String>) {
        self.model.clear();
        self.model.push(&CommandItem::new(msg.into()));
        self.model.select(0);
    }

    fn clear_store(&self) {
        self.model.clear();
        self.model.select(gtk4::INVALID_LIST_POSITION);
    }
}

/// Concrete command handler for `AppListModel`.
///
/// This type alias exists so that the file-search integration (which needs
/// `&AppListModel`) can be provided alongside the generic `CommandHandler<T>`.
pub type AppCommandHandler = CommandHandler<AppListModel>;

impl AppCommandHandler {
    pub fn handle_colon_command(&self, query: &str) {
        let (cmd, arg) = parse_colon_command(query);
        debug!("handle_colon_command: query='{query}', cmd='{cmd}', arg='{arg}'");

        match cmd {
            "ob" | "obg" => self.handle_obsidian(cmd, arg),
            "f" => self.handle_file_search(arg),
            "fg" => self.handle_file_grep(arg),
            "sh" => {
                debug!("Calling handle_sh with arg: '{arg}'");
                // Delegate to the generic method on CommandHandler<T>
                CommandHandler::handle_sh(self, arg);
            }
            _ => {
                if !cmd.is_empty() {
                    self.show_error(format!("Unknown command: :{cmd}"));
                }
            }
        }
    }

    fn handle_obsidian(&self, cmd_name: &str, arg: &str) {
        let Some(vault_path) = self.validated_vault_path() else {
            return;
        };
        let vault_str = vault_path.to_string_lossy().into_owned();

        if arg.is_empty() {
            let mode = if cmd_name == "ob" {
                ActiveMode::ObsidianAction
            } else {
                ActiveMode::ObsidianGrep
            };
            self.model.set_mode(mode);
            self.clear_store();
            return;
        }

        let mode = if cmd_name == "ob" {
            ActiveMode::ObsidianFile
        } else {
            ActiveMode::ObsidianGrep
        };

        let arg = arg.to_string();
        let model = self.model.clone();
        self.model.set_mode(mode);
        self.model.bump_gen();

        if cmd_name == "ob" {
            self.model.schedule(move || {
                crate::providers::file_search::run_find_in_vault(
                    &model,
                    Path::new(&vault_str),
                    &arg,
                );
            });
        } else {
            self.model.schedule(move || {
                crate::providers::file_search::run_rg_in_vault(&model, Path::new(&vault_str), &arg);
            });
        }
    }

    fn handle_file_search(&self, arg: &str) {
        if arg.is_empty() {
            self.clear_store();
            return;
        }
        let arg = arg.to_string();
        let model = self.model.clone();
        self.model.bump_and_schedule(move || {
            crate::providers::file_search::run_file_search(&model, &arg);
        });
    }

    fn handle_file_grep(&self, arg: &str) {
        if arg.is_empty() {
            self.clear_store();
            return;
        }
        let arg = arg.to_string();
        let model = self.model.clone();
        self.model.bump_and_schedule(move || {
            crate::providers::file_search::run_file_grep(&model, &arg);
        });
    }

    fn validated_vault_path(&self) -> Option<PathBuf> {
        use crate::utils::expand_home;
        let Some(obs_cfg) = self.model.obsidian_config() else {
            self.show_error("Obsidian not configured - edit config");
            return None;
        };
        let vault_path = expand_home(&obs_cfg.vault);
        if !vault_path.exists() {
            self.show_error(format!(
                "Vault path does not exist: {}",
                vault_path.display()
            ));
            return None;
        }
        Some(vault_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_colon_command_with_arg() {
        assert_eq!(parse_colon_command(":f foo"), ("f", "foo"));
    }

    #[test]
    fn test_parse_colon_command_no_arg() {
        assert_eq!(parse_colon_command(":ob"), ("ob", ""));
    }

    #[test]
    fn test_parse_colon_command_obg_with_arg() {
        assert_eq!(parse_colon_command(":obg pattern"), ("obg", "pattern"));
    }

    #[test]
    fn test_parse_colon_command_sh_with_arg() {
        assert_eq!(parse_colon_command(":sh ls -la"), ("sh", "ls -la"));
    }

    #[test]
    fn test_parse_colon_command_trims_leading_arg_space() {
        // split_once gives ("f", " foo"), then trim() → "foo"
        assert_eq!(parse_colon_command(":f  foo"), ("f", "foo"));
    }

    #[test]
    fn test_parse_colon_command_arg_trims_trailing_spaces() {
        // split_once gives ("f", "foo  "), then trim() → "foo"
        assert_eq!(parse_colon_command(":f foo  "), ("f", "foo"));
    }

    #[test]
    fn test_parse_colon_command_fg() {
        assert_eq!(
            parse_colon_command(":fg search term"),
            ("fg", "search term")
        );
    }

    #[test]
    fn test_parse_colon_command_single_char() {
        assert_eq!(parse_colon_command(":x"), ("x", ""));
    }
}
