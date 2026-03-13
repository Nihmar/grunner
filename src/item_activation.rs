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
use crate::items::AppItem;
use crate::items::CommandItem;
use crate::items::ObsidianActionItem;
use crate::items::SearchResultItem;
use crate::list_model::AppListModel;
use gtk4::prelude::{Cast, DisplayExt};
use log::{debug, info, warn};

/// Parse and open Obsidian grep result lines
///
/// This function handles grep output lines in the format `<file:line:context>`
/// and opens them in Obsidian at the appropriate line number.
///
/// # Arguments
/// * `line` - The grep result line to parse
/// * `cfg` - Obsidian configuration for vault path and settings
pub fn open_obsidian_grep_line(line: &str, cfg: &crate::config::ObsidianConfig) {
    debug!("Processing Obsidian grep line: {line}");
    if let Some((file_path, rest)) = line.split_once(':') {
        if let Some((line_num, _)) = rest.split_once(':') {
            // File with line number: open at specific line
            info!("Opening Obsidian file at line: {file_path}:{line_num}");
            open_obsidian_file_line(file_path, line_num, cfg);
        } else {
            // File without line number: open file
            info!("Opening Obsidian file: {file_path}");
            open_obsidian_file_path(file_path, cfg);
        }
    } else {
        // Not a grep format line: try to open as plain file
        info!("Opening Obsidian file (non-grep format): {line}");
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
pub fn activate_item(obj: &glib::Object, model: &AppListModel, mode: AppMode, timestamp: u32) {
    debug!("Activating item in mode {mode:?}");
    // Handle desktop application items
    if let Some(app_item) = obj.downcast_ref::<AppItem>() {
        info!(
            "Launching application: {} (terminal: {})",
            app_item.exec(),
            app_item.terminal()
        );
        launch_app(&app_item.exec(), app_item.terminal(), None);
    }
    // Handle command line items (file paths, grep results, calculator results, etc.)
    else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
        let line = cmd_item.line();
        debug!("Activating command line item: {line} in mode {mode:?}");

        // Check if this is a calculator result and copy to clipboard
        if is_calculator_result(&line) {
            // Extract the result part (after the equals sign)
            if let Some((_expr, result)) = line.split_once('=') {
                let result_text = result.trim().to_string();
                info!("Copying calculator result to clipboard: {}", result_text);

                // Copy to clipboard
                if let Some(display) = gtk4::gdk::Display::default() {
                    let clipboard = display.clipboard();
                    clipboard.set_text(&result_text);
                }
            }
            return;
        }

        match mode {
            // Obsidian grep mode: open grep results in Obsidian
            AppMode::ObsidianGrep => {
                if let Some(cfg) = &model.obsidian_cfg {
                    open_obsidian_grep_line(&line, cfg);
                } else {
                    warn!("Obsidian configuration missing for grep line activation");
                }
            }
            // Obsidian file mode: open files in Obsidian
            AppMode::Obsidian => {
                if let Some(cfg) = &model.obsidian_cfg {
                    open_obsidian_file_path(&line, cfg);
                } else {
                    warn!("Obsidian configuration missing for file activation");
                }
            }
            // Custom script mode: execute saved or custom commands
            AppMode::CustomScript => {
                let command_to_run = if let Some((_name, cmd)) = line.split_once(" | ") {
                    // Saved command format: "Name | Command"
                    cmd.trim()
                } else if let Some(stripped) = line.strip_prefix("Run: ") {
                    // Custom command format: "Run: <command>"
                    stripped.trim()
                } else {
                    // Fallback: try to run the whole line
                    line.trim()
                };

                if !command_to_run.is_empty() {
                    info!("Executing custom script command: {}", command_to_run);
                    let working_dir = cmd_item.working_dir();
                    let keep_open = cmd_item.keep_open();

                    // Build command: optionally keep terminal open after execution
                    let final_command = if keep_open {
                        format!("{}; exec $SHELL", command_to_run)
                    } else {
                        command_to_run.to_string()
                    };

                    launch_app(&final_command, true, working_dir);
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
        debug!(
            "Activating Obsidian action: {:?} with arg: {:?}",
            obs_item.action(),
            obs_item.arg()
        );
        if let Some(cfg) = &model.obsidian_cfg {
            perform_obsidian_action(obs_item.action(), obs_item.arg().as_deref(), cfg);
        } else {
            warn!("Obsidian configuration missing for action activation");
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
        info!("Activating search result: {} from provider {}", id, bus);
        std::thread::spawn(move || {
            // Pass the timestamp
            crate::search_provider::activate_result(&bus, &path, &id, &terms, timestamp);
        });
    }
}

/// Check if a line is a calculator result
///
/// A calculator result has the format "expression = result" where:
/// - expression contains only valid calculator characters (digits, operators, spaces, parentheses)
/// - there's an equals sign in the middle
fn is_calculator_result(line: &str) -> bool {
    // Check if line contains '='
    if !line.contains('=') {
        return false;
    }

    // Split at the equals sign
    let parts: Vec<&str> = line.split('=').collect();
    if parts.len() != 2 {
        return false;
    }

    let expr = parts[0].trim();
    let result = parts[1].trim();

    // Expression should not be empty
    if expr.is_empty() {
        return false;
    }

    // Check if expression contains only valid calculator characters
    if !expr.chars().all(|c| {
        c.is_ascii_digit()
            || c == '.'
            || c == '+'
            || c == '-'
            || c == '*'
            || c == '/'
            || c == '%'
            || c == '^'
            || c == '('
            || c == ')'
            || c.is_whitespace()
    }) {
        return false;
    }

    // Check if result looks like a number (starts with digit or minus for negative numbers)
    if !result.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }

    true
}
