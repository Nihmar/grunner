use crate::actions::show_error_notification;
use log::{debug, error, info, warn};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Check if a file at the given path is executable
///
/// On Unix systems, checks the file's execute permission bits.
/// On non-Unix systems, simply returns true if the file exists.
pub fn is_executable(path: &std::path::Path) -> bool {
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
#[must_use]
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

/// Get the cached terminal emulator or find and cache it
pub fn terminal() -> Option<&'static String> {
    TERMINAL.get_or_init(find_terminal_impl).as_ref()
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
pub fn find_terminal() -> Option<String> {
    terminal().cloned()
}

/// Launch an application with optional terminal
///
/// # Arguments
/// * `exec` - Command string to execute
/// * `terminal` - Whether to run the command inside a terminal emulator
/// * `working_dir` - Optional working directory (None = current directory)
///
/// If `terminal` is true, launches the command inside the discovered terminal emulator.
/// Different terminals have different argument syntax for running commands.
#[allow(clippy::needless_pass_by_value)]
pub fn launch_app(exec: &str, terminal: bool, working_dir: Option<String>) {
    debug!("Launching application: {exec} (terminal: {terminal}, working_dir: {working_dir:?})");
    let clean = crate::launcher::clean_exec(exec);
    debug!("Cleaned execution command: {clean}");

    if terminal {
        debug!("Looking for terminal emulator");
        if let Some(term) = find_terminal() {
            info!("Using terminal emulator: {term}");
            let mut cmd = std::process::Command::new(&term);
            if let Some(ref dir) = working_dir {
                cmd.current_dir(dir);
            }
            match term.as_str() {
                // GNOME and XFCE terminals use "--" separator
                "gnome-terminal" | "xfce4-terminal" => {
                    cmd.arg("--").arg("sh").arg("-c").arg(&clean);
                }

                // Kitty uses "--" separator and supports --hold
                "kitty" => {
                    cmd.arg("--hold").arg("--").arg("sh").arg("-c").arg(&clean);
                }
                // Default to "-e" for unknown terminals
                _ => {
                    cmd.arg("-e").arg("sh").arg("-c").arg(&clean);
                }
            }
            debug!("Spawning terminal command: {cmd:?}");
            if let Err(e) = cmd.spawn() {
                error!("Failed to launch terminal {term} with command '{clean}': {e}");
                show_error_notification(&format!("Failed to launch: {clean}"));
            } else {
                info!("Successfully launched application in terminal {term}: {clean}");
            }
        } else {
            warn!("No terminal emulator found for command: {clean}");
            show_error_notification("No terminal emulator found");
        }
    } else {
        // Run directly without terminal
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&clean);
        if let Some(ref dir) = working_dir {
            cmd.current_dir(dir);
        }
        debug!("Spawning command directly: {cmd:?}");
        if let Err(e) = cmd.spawn() {
            error!("Failed to launch command '{clean}': {e}");
            show_error_notification(&format!("Failed to launch: {clean}"));
        } else {
            info!("Successfully launched application: {clean}");
        }
    }
}
