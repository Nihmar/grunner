//! Item activation logic for Grunner
//!
//! This module handles the activation of different types of items in the Grunner
//! application. It contains the logic for determining what action to perform
//! based on the item type and current application mode.

use crate::actions::{
    launch_app, open_file_or_line, open_obsidian_file_line, open_obsidian_file_path,
    perform_obsidian_action,
};
use crate::app_mode::AppMode;
use crate::core::config::ObsidianConfig;
use crate::model::items::{AppItem, CommandItem, ObsidianActionItem, SearchResultItem};
use crate::model::list_model::AppListModel;
use crate::providers::dbus;
use crate::utils::is_calculator_result;
use gtk4::prelude::{Cast, DisplayExt};
use log::{debug, info, warn};

// ─── Activation Context ────────────────────────────────────────────────────────

/// Context for item activation, containing all necessary data
pub struct ActivationContext<'a> {
    pub model: &'a AppListModel,
    pub mode: AppMode,
    pub timestamp: u32,
}

impl<'a> ActivationContext<'a> {
    pub fn new(model: &'a AppListModel, mode: AppMode, timestamp: u32) -> Self {
        Self {
            model,
            mode,
            timestamp,
        }
    }

    #[must_use]
    pub fn obsidian_config(&self) -> Option<&'a ObsidianConfig> {
        self.model.config.obsidian_cfg.as_ref()
    }
}

// ─── GrunnerItem Enum ──────────────────────────────────────────────────────────

/// Enum representing all item types in Grunner
pub enum GrunnerItem<'a> {
    App(&'a AppItem),
    Command(&'a CommandItem),
    ObsidianAction(&'a ObsidianActionItem),
    SearchResult(&'a SearchResultItem),
}

impl<'a> GrunnerItem<'a> {
    /// Try to downcast a `glib::Object` to a `GrunnerItem`
    pub fn from_object(obj: &'a glib::Object) -> Option<Self> {
        if let Some(item) = obj.downcast_ref::<AppItem>() {
            Some(GrunnerItem::App(item))
        } else if let Some(item) = obj.downcast_ref::<CommandItem>() {
            Some(GrunnerItem::Command(item))
        } else if let Some(item) = obj.downcast_ref::<ObsidianActionItem>() {
            Some(GrunnerItem::ObsidianAction(item))
        } else {
            obj.downcast_ref::<SearchResultItem>()
                .map(GrunnerItem::SearchResult)
        }
    }
}

// ─── Activation Functions ──────────────────────────────────────────────────────

fn activate_app(item: &AppItem) {
    info!(
        "Launching application: {} (terminal: {})",
        item.exec(),
        item.terminal()
    );
    launch_app(&item.exec(), item.terminal(), None);
}

fn activate_command(item: &CommandItem, ctx: &ActivationContext) {
    let line = item.line();
    debug!(
        "Activating command line item: {line} in mode {:?}",
        ctx.mode
    );

    if is_calculator_result(&line) {
        if let Some((_expr, result)) = line.split_once('=') {
            let result_text = result.trim().to_string();
            info!("Copying calculator result to clipboard: {result_text}");
            if let Some(display) = gtk4::gdk::Display::default() {
                let clipboard = display.clipboard();
                clipboard.set_text(&result_text);
            }
        }
        return;
    }

    match ctx.mode {
        AppMode::ObsidianGrep => {
            if let Some(cfg) = ctx.obsidian_config() {
                open_obsidian_grep_line(&line, cfg);
            } else {
                warn!("Obsidian configuration missing for grep line activation");
            }
        }
        AppMode::Obsidian => {
            if let Some(cfg) = ctx.obsidian_config() {
                open_obsidian_file_path(&line, cfg);
            } else {
                warn!("Obsidian configuration missing for file activation");
            }
        }
        AppMode::CustomScript => {
            let command_to_run = if let Some((_name, cmd)) = line.split_once(" | ") {
                cmd.trim()
            } else if let Some(stripped) = line.strip_prefix("Run: ") {
                stripped.trim()
            } else {
                line.trim()
            };

            if !command_to_run.is_empty() {
                info!("Executing custom script command: {command_to_run}");
                let working_dir = item.working_dir();
                let keep_open = item.keep_open();

                let final_command = if keep_open {
                    format!("{command_to_run}; exec $SHELL")
                } else {
                    command_to_run.to_string()
                };

                launch_app(&final_command, true, working_dir);
            }
        }
        _ => {
            open_file_or_line(&line);
        }
    }
}

fn activate_obsidian_action(item: &ObsidianActionItem, ctx: &ActivationContext) {
    debug!(
        "Activating Obsidian action: {:?} with arg: {:?}",
        item.action(),
        item.arg()
    );
    if let Some(cfg) = ctx.obsidian_config() {
        perform_obsidian_action(item.action(), item.arg().as_deref(), cfg);
    } else {
        warn!("Obsidian configuration missing for action activation");
    }
}

fn activate_search_result(item: &SearchResultItem, ctx: &ActivationContext) {
    let (bus, path, id, terms) = (item.bus_name(), item.object_path(), item.id(), item.terms());

    if let Some(text) = item.clipboard_text()
        && let Some(display) = gtk4::gdk::Display::default()
    {
        let clipboard = display.clipboard();
        clipboard.set_text(&text);
        info!("Copied '{text}' to clipboard from search result activation");
    }

    info!("Activating search result: {id} from provider {bus}");
    let timestamp = ctx.timestamp;
    std::thread::spawn(move || {
        dbus::activate_result(&bus, &path, &id, &terms, timestamp);
    });
}

// ─── Legacy API (for backwards compatibility) ─────────────────────────────────

/// Parse and open Obsidian grep result lines
///
/// This function handles grep output lines in the format `<file:line:context>`
/// and opens them in Obsidian at the appropriate line number.
///
/// # Arguments
/// * `line` - The grep result line to parse
/// * `cfg` - Obsidian configuration for vault path and settings
pub fn open_obsidian_grep_line(line: &str, cfg: &ObsidianConfig) {
    debug!("Processing Obsidian grep line: {line}");
    if let Some((file_path, rest)) = line.split_once(':') {
        if let Some((line_num, _)) = rest.split_once(':') {
            info!("Opening Obsidian file at line: {file_path}:{line_num}");
            open_obsidian_file_line(file_path, line_num, cfg);
        } else {
            info!("Opening Obsidian file: {file_path}");
            open_obsidian_file_path(file_path, cfg);
        }
    } else {
        info!("Opening Obsidian file (non-grep format): {line}");
        open_obsidian_file_path(line, cfg);
    }
}

/// Activate an item based on its type and the current application mode
///
/// This is the main entry point for item activation in Grunner. It determines
/// what action to perform based on the type of item (application, command,
/// Obsidian action, or search result) and the current application mode.
pub fn activate_item(obj: &glib::Object, model: &AppListModel, mode: AppMode, timestamp: u32) {
    debug!("Activating item in mode {mode:?}");
    let ctx = ActivationContext::new(model, mode, timestamp);

    match GrunnerItem::from_object(obj) {
        Some(GrunnerItem::App(item)) => activate_app(item),
        Some(GrunnerItem::Command(item)) => activate_command(item, &ctx),
        Some(GrunnerItem::ObsidianAction(item)) => activate_obsidian_action(item, &ctx),
        Some(GrunnerItem::SearchResult(item)) => activate_search_result(item, &ctx),
        None => warn!("Unknown item type, cannot activate"),
    }
}
