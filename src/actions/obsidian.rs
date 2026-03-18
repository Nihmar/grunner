use crate::actions::open_uri;
use crate::core::config::ObsidianConfig;
use crate::model::items::ObsidianAction;
use crate::utils::expand_home;
use chrono::Local;
use log::{debug, error, info};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// Perform an Obsidian-related action
///
/// # Arguments
/// * `action` - The `ObsidianAction` to perform
/// * `text` - Optional text content for note actions
/// * `cfg` - Obsidian configuration for vault paths and settings
///
/// Handles all Obsidian operations: opening vault, creating new notes,
/// daily notes, and quick notes.
#[allow(clippy::unnecessary_debug_formatting, clippy::too_many_lines)]
pub fn perform_obsidian_action(action: ObsidianAction, text: Option<&str>, cfg: &ObsidianConfig) {
    debug!("Performing Obsidian action: {action:?} with text: {text:?}");
    let vault_path = expand_home(&cfg.vault);
    debug!("Obsidian vault path: {}", vault_path.display());

    // Validate vault path exists
    if !vault_path.exists() {
        error!(
            "Obsidian vault path does not exist: {}",
            vault_path.display()
        );
        return;
    }

    match action {
        ObsidianAction::OpenVault => {
            // Open entire vault in Obsidian
            info!("Opening Obsidian vault");
            let vault_name = vault_path.file_name().unwrap_or_default().to_string_lossy();
            let uri = format!("obsidian://open?vault={}", urlencoding::encode(&vault_name));
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian vault: {e}");
            }
        }
        ObsidianAction::NewNote => {
            // Create a new note with timestamp in the configured folder
            info!("Creating new Obsidian note");
            let folder = vault_path.join(&cfg.new_notes_folder);
            debug!("New note folder: {}", folder.display());
            if let Err(e) = fs::create_dir_all(&folder) {
                error!("Failed to create new note folder {}: {e}", folder.display());
                return;
            }

            // Generate filename with current timestamp
            let now = Local::now();
            let filename = format!("New Note {}.md", now.format("%Y-%m-%d %H-%M-%S"));
            let path = folder.join(filename);

            // Create the note file
            debug!("Creating note file: {}", path.display());
            let mut file = match File::create(&path) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to create note file {}: {e}", path.display());
                    return;
                }
            };

            // Write optional text content to the note
            if let Some(t) = text
                && !t.is_empty()
            {
                debug!("Writing {} characters to note", t.len());
                if let Err(e) = writeln!(file, "{t}") {
                    error!("Failed to write text to note {}: {e}", path.display());
                }
            }

            // Open the new note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian file: {e}");
            }
        }
        ObsidianAction::DailyNote => {
            // Open or create today's daily note
            info!("Opening/creating daily Obsidian note");
            let folder = vault_path.join(&cfg.daily_notes_folder);
            debug!("Daily notes folder: {}", folder.display());
            if let Err(e) = fs::create_dir_all(&folder) {
                error!(
                    "Failed to create daily notes folder {}: {e}",
                    folder.display()
                );
                return;
            }

            // Use today's date for filename
            let today = Local::now().format("%Y-%m-%d").to_string();
            let path = folder.join(format!("{today}.md"));

            // Open in append mode to preserve existing content
            debug!("Opening daily note file: {}", path.display());
            let mut file = match fs::OpenOptions::new().create(true).append(true).open(&path) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to open daily note file {}: {e}", path.display());
                    return;
                }
            };

            // Append optional text to the daily note
            if let Some(t) = text
                && !t.is_empty()
            {
                debug!("Appending {} characters to daily note", t.len());
                if let Err(e) = writeln!(file, "{t}") {
                    error!(
                        "Failed to append text to daily note {}: {e}",
                        path.display()
                    );
                }
            }

            // Open the daily note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian daily note: {e}");
            }
        }
        ObsidianAction::QuickNote => {
            // Append text to the configured quick note file
            info!("Updating quick Obsidian note");
            let path = vault_path.join(&cfg.quick_note);
            debug!("Quick note path: {}", path.display());

            // Ensure parent directory exists
            if let Some(parent) = path.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                error!(
                    "Failed to create quick note parent directory {}: {e}",
                    parent.display()
                );
                return;
            }

            // Append text to quick note if provided
            if let Some(t) = text
                && !t.is_empty()
            {
                debug!("Appending {} characters to quick note", t.len());
                let mut file = match fs::OpenOptions::new().create(true).append(true).open(&path) {
                    Ok(f) => f,
                    Err(e) => {
                        error!("Failed to open quick note file {}: {e}", path.display());
                        return;
                    }
                };
                if let Err(e) = writeln!(file, "{t}") {
                    error!("Failed to write to quick note {}: {e}", path.display());
                }
            }

            // Open the quick note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian quick note: {e}");
            }
        }
    }
}

/// Open an Obsidian file by its path
///
/// # Arguments
/// * `file_path` - Path to the file within the Obsidian vault
/// * `cfg` - Obsidian configuration for vault location
///
/// Opens the specified file in Obsidian using the obsidian:// URI scheme.
pub fn open_obsidian_file_path(file_path: &str, cfg: &ObsidianConfig) {
    debug!("Opening Obsidian file path: {file_path}");
    let vault_path = expand_home(&cfg.vault);

    // Validate vault exists
    if !vault_path.exists() {
        error!(
            "Obsidian vault path does not exist: {}",
            vault_path.display()
        );
        return;
    }

    // Construct and open Obsidian URI
    let uri = format!("obsidian://open?path={}", urlencoding::encode(file_path));
    if let Err(e) = open_uri(&uri) {
        error!("Failed to open Obsidian file: {e}");
    }
}

/// Open an Obsidian file at a specific line
///
/// # Arguments
/// * `file_path` - Path to the file within the Obsidian vault
/// * `line` - Line number to jump to
/// * `cfg` - Obsidian configuration for vault location
///
/// Opens the specified file in Obsidian and jumps to the given line number.
pub fn open_obsidian_file_line(file_path: &str, line: &str, cfg: &ObsidianConfig) {
    debug!("Opening Obsidian file at line: {file_path}:{line}");
    let vault_path = expand_home(&cfg.vault);

    // Validate vault exists
    if !vault_path.exists() {
        error!(
            "Obsidian vault path does not exist: {}",
            vault_path.display()
        );
        return;
    }

    // Handle both absolute and relative paths
    let path = if file_path.starts_with('/') {
        PathBuf::from(file_path)
    } else {
        vault_path.join(file_path)
    };
    debug!("Resolved path: {}", path.display());

    // Construct Obsidian URI with line parameter
    let uri = format!(
        "obsidian://open?path={}&line={}",
        urlencoding::encode(&path.to_string_lossy()),
        line
    );
    if let Err(e) = open_uri(&uri) {
        error!("Failed to open Obsidian file at line: {e}");
    }
}
