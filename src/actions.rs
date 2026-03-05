//! Action execution module for Grunner
//!
//! This module handles all external actions performed by the application:
//! - Launching applications (with or without terminal)
//! - Power management actions (logout, suspend, reboot, shutdown)
//! - File and line opening operations
//! - Obsidian vault and note management
//! - Settings management

use crate::config::ObsidianConfig;
use crate::items::ObsidianAction;
use crate::launcher;
use crate::settings_window;
use crate::utils::expand_home;
use chrono::Local;
use gtk4::prelude::DisplayExt;
use libadwaita;
use log::{debug, error, info, warn};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Check if a file at the given path is executable
///
/// On Unix systems, checks the file's execute permission bits.
/// On non-Unix systems, simply returns true if the file exists.
fn is_executable(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    true
}

/// Find an executable in the system PATH
///
/// Searches through directories in the PATH environment variable
/// and returns the first path where the executable is found.
pub fn which(prog: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(prog))
        .find(|p| is_executable(p))
}

/// Global lazy-loaded terminal emulator preference
///
/// This is computed once and reused throughout the application lifetime.
/// It searches for available terminal emulators in a specific order of preference.
pub static TERMINAL: OnceLock<Option<String>> = OnceLock::new();

fn terminal() -> &'static Option<String> {
    TERMINAL.get_or_init(|| find_terminal_impl())
}

/// Implementation of terminal emulator discovery
///
/// Searches for common terminal emulators in order of preference:
/// 1. Modern lightweight terminals (foot, alacritty, kitty, wezterm, ghostty)
/// 2. Desktop environment terminals (gnome-terminal, xfce4-terminal, konsole)
/// 3. Fallback to xterm
fn find_terminal_impl() -> Option<String> {
    let candidates = [
        "foot",
        "alacritty",
        "kitty",
        "wezterm",
        "ghostty",
        "gnome-terminal",
        "xfce4-terminal",
        "konsole",
        "xterm",
    ];
    candidates
        .iter()
        .find(|&&c| which(c).is_some())
        .map(|&c| c.to_string())
}

/// Get the preferred terminal emulator
///
/// Returns the cached terminal emulator found at startup.
fn find_terminal() -> Option<String> {
    terminal().clone()
}

/// Launch an application with optional terminal
///
/// # Arguments
/// * `exec` - Command string to execute
/// * `terminal` - Whether to run the command inside a terminal emulator
///
/// If `terminal` is true, launches the command inside the discovered terminal emulator.
/// Different terminals have different argument syntax for running commands.
pub fn launch_app(exec: &str, terminal: bool) {
    debug!("Launching application: {} (terminal: {})", exec, terminal);
    let clean = launcher::clean_exec(exec);
    debug!("Cleaned execution command: {}", clean);

    if terminal {
        debug!("Looking for terminal emulator");
        if let Some(term) = find_terminal() {
            info!("Using terminal emulator: {}", term);
            let mut cmd = std::process::Command::new(&term);
            match term.as_str() {
                // GNOME and XFCE terminals use "--" separator
                "gnome-terminal" | "xfce4-terminal" => {
                    cmd.arg("--").arg("sh").arg("-c").arg(&clean);
                }
                // KDE's Konsole and other terminals use "-e" flag
                "konsole" | "alacritty" | "foot" => {
                    cmd.arg("-e").arg("sh").arg("-c").arg(&clean);
                }
                // Kitty uses "--" separator
                "kitty" => {
                    cmd.arg("--").arg("sh").arg("-c").arg(&clean);
                }
                // Default to "-e" for unknown terminals
                _ => {
                    cmd.arg("-e").arg("sh").arg("-c").arg(&clean);
                }
            }
            debug!("Spawning terminal command: {:?}", cmd);
            if let Err(e) = cmd.spawn() {
                error!(
                    "Failed to launch terminal {} with command '{}': {}",
                    term, clean, e
                );
            } else {
                info!(
                    "Successfully launched application in terminal {}: {}",
                    term, clean
                );
            }
        } else {
            warn!("No terminal emulator found for command: {}", clean);
        }
    } else {
        // Run directly without terminal
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&clean);
        debug!("Spawning command directly: {:?}", cmd);
        if let Err(e) = cmd.spawn() {
            error!("Failed to launch command '{}': {}", clean, e);
        } else {
            info!("Successfully launched application: {}", clean);
        }
    }
}

/// Perform a power management action
///
/// # Arguments
/// * `action` - The action to perform: "logout", "suspend", "reboot", or "poweroff"
///
/// Uses systemctl for suspend, reboot, and poweroff actions.
/// logout_action() handles logout with various methods.
pub fn power_action(action: &str) {
    debug!("Performing power action: {}", action);
    let run_systemctl = |subcmd: &str| {
        debug!("Running systemctl {}", subcmd);
        if let Err(e) = std::process::Command::new("systemctl").arg(subcmd).spawn() {
            error!("Failed to run systemctl {}: {}", subcmd, e);
        } else {
            info!("Successfully initiated systemctl {}", subcmd);
        }
    };

    match action {
        "logout" => {
            info!("Logging out current session");
            logout_action()
        }
        "suspend" => {
            info!("Suspending system");
            run_systemctl("suspend")
        }
        "reboot" => {
            info!("Rebooting system");
            run_systemctl("reboot")
        }
        "poweroff" => {
            info!("Shutting down system");
            run_systemctl("poweroff")
        }
        _ => {
            warn!("Unknown power action: {}", action);
        }
    }
}

/// Log out the current user session
///
/// Attempts multiple logout methods in order:
/// 1. Use loginctl with XDG_SESSION_ID
/// 2. Use gnome-session-quit for GNOME sessions
/// 3. Use loginctl with current username as fallback
fn logout_action() {
    debug!("Attempting to log out current session");
    // First try: Use XDG_SESSION_ID if available
    if let Ok(session_id) = std::env::var("XDG_SESSION_ID") {
        if !session_id.is_empty() {
            debug!("Using XDG_SESSION_ID {} for logout", session_id);
            let status = std::process::Command::new("loginctl")
                .args(["terminate-session", &session_id])
                .status();
            if let Ok(status) = status {
                if status.success() {
                    info!("Successfully logged out via loginctl with XDG_SESSION_ID");
                    return;
                } else {
                    warn!(
                        "loginctl terminate-session failed with status: {:?}",
                        status
                    );
                }
            } else {
                error!("Failed to execute loginctl terminate-session command");
            }
        } else {
            debug!("XDG_SESSION_ID is empty");
        }
    } else {
        debug!("XDG_SESSION_ID environment variable not set");
    }

    // Second try: Use GNOME session quit command
    if let Some(path) = which("gnome-session-quit") {
        debug!("Using gnome-session-quit at {:?} for logout", path);
        let status = std::process::Command::new(path).arg("--logout").status();
        if let Ok(status) = status {
            if status.success() {
                info!("Successfully logged out via gnome-session-quit");
                return;
            } else {
                warn!("gnome-session-quit failed with status: {:?}", status);
            }
        } else {
            error!("Failed to execute gnome-session-quit command");
        }
    } else {
        debug!("gnome-session-quit not found in PATH");
    }

    // Final fallback: Terminate user session via loginctl
    debug!("Falling back to loginctl terminate-user");
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default();
    if !user.is_empty() {
        info!("Logging out user {} via loginctl terminate-user", user);
        if let Err(e) = std::process::Command::new("loginctl")
            .args(["terminate-user", &user])
            .spawn()
        {
            error!("Failed to execute loginctl terminate-user: {}", e);
        } else {
            info!("Successfully initiated logout for user {}", user);
        }
    } else {
        warn!("Cannot determine current user for logout");
    }
}

/// Open the application settings file
/// Open the settings GUI window
///
/// Opens a graphical interface for editing Grunner's configuration settings.
pub fn open_settings(window: &libadwaita::ApplicationWindow, entry: &gtk4::Entry) {
    info!("Opening GUI settings window");
    settings_window::open_settings_window(window, entry);
}

/// Parse a file:line:content pattern (like grep -n output)
///
/// Returns (file_path, line_number) if the input matches "path:line:" format
/// where line_number is a positive integer.
fn parse_file_line(line: &str) -> Option<(&str, u32)> {
    // Find the first colon that separates file path from line number
    // We look for pattern: file_path:line_number:rest
    // file_path cannot contain colon on Unix systems
    let mut parts = line.splitn(3, ':');
    let file = parts.next()?;
    if file.is_empty() {
        return None; // File path cannot be empty
    }
    let line_str = parts.next()?;
    // There must be a third part (the content after second colon)
    parts.next()?;

    // Parse line number
    let line_num = line_str.parse::<u32>().ok()?;
    if line_num == 0 {
        return None; // Line numbers start at 1
    }

    Some((file, line_num))
}

/// Open a file or file:line combination
///
/// # Arguments
/// * `line` - Either a file path or "file:line" format
///
/// If the input matches "file:line:content" format (like grep output),
/// opens the file at the specified line using the system EDITOR or xdg-open.
/// If it's just a file path, opens the file.
/// If the path doesn't exist, copies the text to clipboard as a fallback.
pub fn open_file_or_line(line: &str) {
    debug!("Opening file or line: {}", line);
    // Check if input matches "file:line:content" pattern (like grep -n output)
    if let Some((file, line_num)) = parse_file_line(line) {
        // Verify file exists before attempting to open
        if Path::new(file).exists() {
            info!("Opening file {} at line {}", file, line_num);
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "xdg-open".to_string());
            debug!("Using editor: {}", editor);
            let mut cmd = std::process::Command::new(&editor);

            // Add line number argument for text editors (not for xdg-open)
            if editor != "xdg-open" {
                cmd.arg(format!("+{}", line_num));
            }
            cmd.arg(file);

            debug!("Spawning command: {:?}", cmd);
            if let Err(e) = cmd.spawn() {
                error!("Failed to open file {} at line {}: {}", file, line_num, e);
            } else {
                info!("Successfully opened file {} at line {}", file, line_num);
            }
            return;
        }
    }

    // If not a file:line pattern or file doesn't exist, try opening as plain file
    if Path::new(line).exists() {
        info!("Opening file: {}", line);
        if let Err(e) = std::process::Command::new("xdg-open").arg(line).spawn() {
            error!("Failed to open file {} with xdg-open: {}", line, e);
        } else {
            info!("Successfully opened file: {}", line);
        }
    } else {
        // Path doesn't exist - copy text to clipboard as fallback
        warn!("Path does not exist, copying to clipboard: {}", line);
        let display = gtk4::gdk::Display::default().expect("cannot get display");
        let clipboard = display.clipboard();
        clipboard.set_text(line);
        info!("Copied text to clipboard: {}", line);
    }
}

/// Perform an Obsidian-related action
///
/// # Arguments
/// * `action` - The ObsidianAction to perform
/// * `text` - Optional text content for note actions
/// * `cfg` - Obsidian configuration for vault paths and settings
///
/// Handles all Obsidian operations: opening vault, creating new notes,
/// daily notes, and quick notes.
pub fn perform_obsidian_action(action: ObsidianAction, text: Option<&str>, cfg: &ObsidianConfig) {
    debug!(
        "Performing Obsidian action: {:?} with text: {:?}",
        action, text
    );
    let vault_path = expand_home(&cfg.vault);
    debug!("Obsidian vault path: {:?}", vault_path);

    // Validate vault path exists
    if !vault_path.exists() {
        error!("Obsidian vault path does not exist: {:?}", vault_path);
        return;
    }

    match action {
        ObsidianAction::OpenVault => {
            // Open entire vault in Obsidian
            info!("Opening Obsidian vault");
            let vault_name = vault_path.file_name().unwrap_or_default().to_string_lossy();
            let uri = format!("obsidian://open?vault={}", urlencoding::encode(&vault_name));
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian vault: {}", e);
            }
        }
        ObsidianAction::NewNote => {
            // Create a new note with timestamp in the configured folder
            info!("Creating new Obsidian note");
            let folder = vault_path.join(&cfg.new_notes_folder);
            debug!("New note folder: {:?}", folder);
            if let Err(e) = fs::create_dir_all(&folder) {
                error!("Failed to create new note folder {:?}: {}", folder, e);
                return;
            }

            // Generate filename with current timestamp
            let now = Local::now();
            let filename = format!("New Note {}.md", now.format("%Y-%m-%d %H-%M-%S"));
            let path = folder.join(filename);

            // Create the note file
            debug!("Creating note file: {:?}", path);
            let mut file = match File::create(&path) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to create note file {:?}: {}", path, e);
                    return;
                }
            };

            // Write optional text content to the note
            if let Some(t) = text {
                if !t.is_empty() {
                    debug!("Writing {} characters to note", t.len());
                    if let Err(e) = writeln!(file, "{}", t) {
                        error!("Failed to write text to note {:?}: {}", path, e);
                    }
                }
            }

            // Open the new note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian file: {}", e);
            }
        }
        ObsidianAction::DailyNote => {
            // Open or create today's daily note
            info!("Opening/creating daily Obsidian note");
            let folder = vault_path.join(&cfg.daily_notes_folder);
            debug!("Daily notes folder: {:?}", folder);
            if let Err(e) = fs::create_dir_all(&folder) {
                error!("Failed to create daily notes folder {:?}: {}", folder, e);
                return;
            }

            // Use today's date for filename
            let today = Local::now().format("%Y-%m-%d").to_string();
            let path = folder.join(format!("{}.md", today));

            // Open in append mode to preserve existing content
            debug!("Opening daily note file: {:?}", path);
            let mut file = match fs::OpenOptions::new().create(true).append(true).open(&path) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to open daily note file {:?}: {}", path, e);
                    return;
                }
            };

            // Append optional text to the daily note
            if let Some(t) = text {
                if !t.is_empty() {
                    debug!("Appending {} characters to daily note", t.len());
                    if let Err(e) = writeln!(file, "{}", t) {
                        error!("Failed to append text to daily note {:?}: {}", path, e);
                    }
                }
            }

            // Open the daily note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian daily note: {}", e);
            }
        }
        ObsidianAction::QuickNote => {
            // Append text to the configured quick note file
            info!("Updating quick Obsidian note");
            let path = vault_path.join(&cfg.quick_note);
            debug!("Quick note path: {:?}", path);

            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    error!(
                        "Failed to create quick note parent directory {:?}: {}",
                        parent, e
                    );
                    return;
                }
            }

            // Append text to quick note if provided
            if let Some(t) = text {
                if !t.is_empty() {
                    debug!("Appending {} characters to quick note", t.len());
                    let mut file =
                        match fs::OpenOptions::new().create(true).append(true).open(&path) {
                            Ok(f) => f,
                            Err(e) => {
                                error!("Failed to open quick note file {:?}: {}", path, e);
                                return;
                            }
                        };
                    if let Err(e) = writeln!(file, "{}", t) {
                        error!("Failed to write to quick note {:?}: {}", path, e);
                    }
                }
            }

            // Open the quick note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open Obsidian quick note: {}", e);
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
    debug!("Opening Obsidian file path: {}", file_path);
    let vault_path = expand_home(&cfg.vault);

    // Validate vault exists
    if !vault_path.exists() {
        error!("Obsidian vault path does not exist: {:?}", vault_path);
        return;
    }

    // Construct and open Obsidian URI
    let uri = format!("obsidian://open?path={}", urlencoding::encode(file_path));
    if let Err(e) = open_uri(&uri) {
        error!("Failed to open Obsidian file: {}", e);
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
    debug!("Opening Obsidian file at line: {}:{}", file_path, line);
    let vault_path = expand_home(&cfg.vault);

    // Validate vault exists
    if !vault_path.exists() {
        error!("Obsidian vault path does not exist: {:?}", vault_path);
        return;
    }

    // Handle both absolute and relative paths
    let path = if file_path.starts_with('/') {
        PathBuf::from(file_path)
    } else {
        vault_path.join(file_path)
    };
    debug!("Resolved path: {:?}", path);

    // Construct Obsidian URI with line parameter
    let uri = format!(
        "obsidian://open?path={}&line={}",
        urlencoding::encode(&path.to_string_lossy()),
        line
    );
    if let Err(e) = open_uri(&uri) {
        error!("Failed to open Obsidian file at line: {}", e);
    }
}

/// Open a URI using xdg-open
///
/// # Arguments
/// * `uri` - The URI to open (obsidian://, http://, etc.)
///
/// Uses the system's default URI handler (xdg-open on Linux) to open the URI.
pub fn open_uri(uri: &str) -> Result<(), std::io::Error> {
    debug!("Opening URI: {}", uri);
    match std::process::Command::new("xdg-open").arg(uri).spawn() {
        Ok(_) => {
            info!("Successfully opened URI: {}", uri);
            Ok(())
        }
        Err(e) => {
            error!("Failed to open URI '{}': {}", uri, e);
            Err(e)
        }
    }
}
