//! Item activation logic for Grunner
//!
//! This module handles the activation of different types of items in the Grunner
//! application. It contains the logic for determining what action to perform
//! based on the item type and current application mode.

use crate::actions::{
    launch_app, open_file_or_line, open_obsidian_file_line, open_obsidian_file_path,
    perform_obsidian_action,
};
use crate::app_item::AppItem;
use crate::app_mode::AppMode;
use crate::cmd_item::CommandItem;
use crate::list_model::AppListModel;
use crate::obsidian_item::ObsidianActionItem;
use crate::search_result_item::SearchResultItem;
use gtk4::prelude::Cast;

/// Parse and open Obsidian grep result lines
///
/// This function handles grep output lines in the format "file:line:context"
/// and opens them in Obsidian at the appropriate line number.
///
/// # Arguments
/// * `line` - The grep result line to parse
/// * `cfg` - Obsidian configuration for vault path and settings
pub fn open_obsidian_grep_line(line: &str, cfg: &crate::config::ObsidianConfig) {
    if let Some((file_path, rest)) = line.split_once(':') {
        if let Some((line_num, _)) = rest.split_once(':') {
            // File with line number: open at specific line
            open_obsidian_file_line(file_path, line_num, cfg);
        } else {
            // File without line number: open file
            open_obsidian_file_path(file_path, cfg);
        }
    } else {
        // Not a grep format line: try to open as plain file
        open_obsidian_file_path(line, cfg);
    }
}

/// Activate an item based on its type and the current application mode
///
/// This is the main entry point for item activation in Grunner. It determines
/// what action to perform based on the type of item (application, command,
/// Obsidian action, or search result) and the current application mode.
///
/// # Arguments
/// * `obj` - The GTK object representing the item to activate
/// * `model` - The application list model containing configuration and state
/// * `mode` - The current application mode (Normal, Obsidian, FileSearch, etc.)
pub fn activate_item(obj: &glib::Object, model: &AppListModel, mode: AppMode) {
    // Handle desktop application items
    if let Ok(app_item) = obj.clone().downcast::<AppItem>() {
        launch_app(&app_item.exec(), app_item.terminal());
    }
    // Handle command line items (file paths, grep results, etc.)
    else if let Ok(cmd_item) = obj.clone().downcast::<CommandItem>() {
        let line = cmd_item.line();
        match mode {
            // Obsidian grep mode: open grep results in Obsidian
            AppMode::ObsidianGrep => {
                if let Some(cfg) = &model.obsidian_cfg {
                    open_obsidian_grep_line(&line, cfg);
                }
            }
            // Obsidian file mode: open files in Obsidian
            AppMode::Obsidian => {
                if let Some(cfg) = &model.obsidian_cfg {
                    open_obsidian_file_path(&line, cfg);
                }
            }
            // Other modes: open files or execute commands
            _ => {
                open_file_or_line(&line);
            }
        }
    }
    // Handle Obsidian action items (vault open, new note, etc.)
    else if let Ok(obs_item) = obj.clone().downcast::<ObsidianActionItem>() {
        if let Some(cfg) = &model.obsidian_cfg {
            perform_obsidian_action(obs_item.action(), obs_item.arg().as_deref(), cfg);
        }
    }
    // Handle GNOME Shell search provider results
    else if let Ok(sr_item) = obj.clone().downcast::<SearchResultItem>() {
        let (bus, path, id, terms) = (
            sr_item.bus_name(),
            sr_item.object_path(),
            sr_item.id(),
            sr_item.terms(),
        );
        // Activate search result in background thread to avoid blocking UI
        std::thread::spawn(move || {
            crate::search_provider::activate_result(&bus, &path, &id, &terms);
        });
    }
}
