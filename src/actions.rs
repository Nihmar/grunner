//! Action execution module for Grunner
//!
//! This module handles all external actions performed by the application:
//! - Launching applications (with or without terminal)
//! - Power management actions (logout, suspend, reboot, shutdown)
//! - File and line opening operations
//! - Obsidian vault and note management
//! - Settings management

use crate::config;
use crate::config::ObsidianConfig;
use crate::launcher;
use crate::obsidian_item::ObsidianAction;
use crate::utils::expand_home;
use chrono::Local;
use gtk4::prelude::DisplayExt;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

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
fn which(prog: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(prog))
        .find(|p| is_executable(p))
}

/// Global lazy-loaded terminal emulator preference
///
/// This is computed once and reused throughout the application lifetime.
/// It searches for available terminal emulators in a specific order of preference.
pub static TERMINAL: Lazy<Option<String>> = Lazy::new(find_terminal_impl);

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
    TERMINAL.clone()
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
    let clean = launcher::clean_exec(exec);
    if terminal {
        if let Some(term) = find_terminal() {
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
            if let Err(e) = cmd.spawn() {
                eprintln!("Failed to launch terminal {}: {}", term, e);
            }
        } else {
            eprintln!("No terminal emulator found");
        }
    } else {
        // Run directly without terminal
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&clean);
        if let Err(e) = cmd.spawn() {
            eprintln!("Failed to launch {}: {}", clean, e);
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
    let run_systemctl = |subcmd: &str| {
        if let Err(e) = std::process::Command::new("systemctl").arg(subcmd).spawn() {
            eprintln!("Failed to run systemctl {}: {}", subcmd, e);
        }
    };

    match action {
        "logout" => logout_action(),
        "suspend" => run_systemctl("suspend"),
        "reboot" => run_systemctl("reboot"),
        "poweroff" => run_systemctl("poweroff"),
        _ => {}
    }
}

/// Log out the current user session
///
/// Attempts multiple logout methods in order:
/// 1. Use loginctl with XDG_SESSION_ID
/// 2. Use gnome-session-quit for GNOME sessions
/// 3. Use loginctl with current username as fallback
fn logout_action() {
    // First try: Use XDG_SESSION_ID if available
    if let Ok(session_id) = std::env::var("XDG_SESSION_ID") {
        if !session_id.is_empty() {
            let status = std::process::Command::new("loginctl")
                .args(["terminate-session", &session_id])
                .status();
            if let Ok(status) = status {
                if status.success() {
                    return;
                }
            }
        }
    }

    // Second try: Use GNOME session quit command
    if let Some(path) = which("gnome-session-quit") {
        let status = std::process::Command::new(path).arg("--logout").status();
        if let Ok(status) = status {
            if status.success() {
                return;
            }
        }
    }

    // Final fallback: Terminate user session via loginctl
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default();
    if !user.is_empty() {
        let _ = std::process::Command::new("loginctl")
            .args(["terminate-user", &user])
            .spawn();
    }
}

/// Open the application settings file
///
/// Creates the config directory if it doesn't exist, ensures the config file exists,
/// and opens it with the system's default text editor via xdg-open.
pub fn open_settings() {
    let path = config::config_path();

    // Ensure config directory exists
    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("Failed to create config dir: {}", e);
        }
    }

    // Ensure config file exists by loading it
    if !path.exists() {
        config::load();
    }

    // Open with system default editor
    if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
        eprintln!("Failed to open settings with xdg-open: {}", e);
    }
}

/// Regular expression for parsing file:line format
///
/// Matches patterns like "file/path:123:" or "file/path:456"
static FILE_LINE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+):(\d+):").unwrap());

/// Open a file or file:line combination
///
/// # Arguments
/// * `line` - Either a file path or "file:line" format
///
/// If the input matches "file:line" format, opens the file at the specified line
/// using the system EDITOR or xdg-open. If it's just a file path, opens the file.
/// If the path doesn't exist, copies the text to clipboard as a fallback.
pub fn open_file_or_line(line: &str) {
    let re = &*FILE_LINE_RE;

    // Check if input matches "file:line" pattern
    if let Some(caps) = re.captures(line) {
        let file = caps.get(1).unwrap().as_str();
        let line_num = caps.get(2).unwrap().as_str();

        // Verify file exists before attempting to open
        if Path::new(file).exists() {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "xdg-open".to_string());
            let mut cmd = std::process::Command::new(&editor);

            // Add line number argument for text editors (not for xdg-open)
            if editor != "xdg-open" {
                cmd.arg(format!("+{}", line_num));
            }
            cmd.arg(file);

            if let Err(e) = cmd.spawn() {
                eprintln!("Failed to open file at line: {}", e);
            }
            return;
        }
    }

    // If not a file:line pattern or file doesn't exist, try opening as plain file
    if Path::new(line).exists() {
        if let Err(e) = std::process::Command::new("xdg-open").arg(line).spawn() {
            eprintln!("Failed to open file: {}", e);
        }
    } else {
        // Path doesn't exist - copy text to clipboard as fallback
        let display = gtk4::gdk::Display::default().expect("cannot get display");
        let clipboard = display.clipboard();
        clipboard.set_text(line);
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
    let vault_path = expand_home(&cfg.vault);

    // Validate vault path exists
    if !vault_path.exists() {
        eprintln!("Vault path does not exist: {}", vault_path.display());
        return;
    }

    match action {
        ObsidianAction::OpenVault => {
            // Open entire vault in Obsidian
            let vault_name = vault_path.file_name().unwrap_or_default().to_string_lossy();
            let uri = format!("obsidian://open?vault={}", urlencoding::encode(&vault_name));
            open_uri(&uri);
        }
        ObsidianAction::NewNote => {
            // Create a new note with timestamp in the configured folder
            let folder = vault_path.join(&cfg.new_notes_folder);
            if let Err(e) = fs::create_dir_all(&folder) {
                eprintln!("Cannot create folder {}: {}", folder.display(), e);
                return;
            }

            // Generate filename with current timestamp
            let now = Local::now();
            let filename = format!("New Note {}.md", now.format("%Y-%m-%d %H-%M-%S"));
            let path = folder.join(filename);

            // Create the note file
            let mut file = match File::create(&path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Cannot create note {}: {}", path.display(), e);
                    return;
                }
            };

            // Write optional text content to the note
            if let Some(t) = text {
                if !t.is_empty() {
                    if let Err(e) = writeln!(file, "{}", t) {
                        eprintln!("Cannot write to note {}: {}", path.display(), e);
                    }
                }
            }

            // Open the new note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            open_uri(&uri);
        }
        ObsidianAction::DailyNote => {
            // Open or create today's daily note
            let folder = vault_path.join(&cfg.daily_notes_folder);
            if let Err(e) = fs::create_dir_all(&folder) {
                eprintln!("Cannot create folder {}: {}", folder.display(), e);
                return;
            }

            // Use today's date for filename
            let today = Local::now().format("%Y-%m-%d").to_string();
            let path = folder.join(format!("{}.md", today));

            // Open in append mode to preserve existing content
            let mut file = match fs::OpenOptions::new().create(true).append(true).open(&path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Cannot open daily note {}: {}", path.display(), e);
                    return;
                }
            };

            // Append optional text to the daily note
            if let Some(t) = text {
                if !t.is_empty() {
                    writeln!(file, "{}", t).ok();
                }
            }

            // Open the daily note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            open_uri(&uri);
        }
        ObsidianAction::QuickNote => {
            // Append text to the configured quick note file
            let path = vault_path.join(&cfg.quick_note);

            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!("Cannot create folder {}: {}", parent.display(), e);
                    return;
                }
            }

            // Append text to quick note if provided
            if let Some(t) = text {
                if !t.is_empty() {
                    let mut file = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)
                        .expect("cannot open quick note");
                    writeln!(file, "{}", t).ok();
                }
            }

            // Open the quick note in Obsidian
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            open_uri(&uri);
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
    let vault_path = expand_home(&cfg.vault);

    // Validate vault exists
    if !vault_path.exists() {
        eprintln!("Vault path does not exist: {}", vault_path.display());
        return;
    }

    // Construct and open Obsidian URI
    let uri = format!("obsidian://open?path={}", urlencoding::encode(file_path));
    open_uri(&uri);
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
    let vault_path = expand_home(&cfg.vault);

    // Validate vault exists
    if !vault_path.exists() {
        eprintln!("Vault path does not exist: {}", vault_path.display());
        return;
    }

    // Handle both absolute and relative paths
    let path = if file_path.starts_with('/') {
        PathBuf::from(file_path)
    } else {
        vault_path.join(file_path)
    };

    // Construct Obsidian URI with line parameter
    let uri = format!(
        "obsidian://open?path={}&line={}",
        urlencoding::encode(&path.to_string_lossy()),
        line
    );
    open_uri(&uri);
}

/// Open a URI using xdg-open
///
/// # Arguments
/// * `uri` - The URI to open (obsidian://, http://, etc.)
///
/// Uses the system's default URI handler (xdg-open on Linux) to open the URI.
fn open_uri(uri: &str) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(uri).spawn() {
        eprintln!("Failed to open URI {}: {}", uri, e);
    }
}
