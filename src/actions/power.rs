use crate::actions::launcher::which;
use log::{debug, error, info, warn};

/// Perform a power management action
///
/// # Arguments
/// * `action` - The action to perform: "logout", "suspend", "reboot", or "poweroff"
///
/// Uses systemctl for suspend, reboot, and poweroff actions.
/// `logout_action()` handles logout with various methods.
pub fn power_action(action: &str) {
    debug!("Performing power action: {action}");
    let run_systemctl = |subcmd: &str| {
        debug!("Running systemctl {subcmd}");
        let mut cmd = std::process::Command::new("systemctl");
        // Use -i to ignore inhibitors and force the operation
        cmd.arg("-i").arg(subcmd);
        if let Err(e) = cmd.spawn() {
            error!("Failed to run systemctl {subcmd}: {e}");
        } else {
            info!("Successfully initiated systemctl {subcmd}");
        }
    };

    match action {
        "logout" => {
            info!("Logging out current session");
            logout_action();
        }
        "suspend" => {
            info!("Suspending system");
            run_systemctl("suspend");
        }
        "reboot" => {
            info!("Rebooting system");
            run_systemctl("reboot");
        }
        "poweroff" => {
            info!("Shutting down system");
            run_systemctl("poweroff");
        }
        _ => {
            warn!("Unknown power action: {action}");
        }
    }
}

/// Log out the current user session
///
/// Attempts multiple logout methods in order:
/// 1. Use loginctl with `XDG_SESSION_ID`
/// 2. Use gnome-session-quit for GNOME sessions
/// 3. Use loginctl with current username as fallback
fn logout_action() {
    debug!("Attempting to log out current session");
    // First try: Use XDG_SESSION_ID if available
    if let Ok(session_id) = std::env::var("XDG_SESSION_ID") {
        if session_id.is_empty() {
            debug!("XDG_SESSION_ID is empty");
        } else {
            debug!("Using XDG_SESSION_ID {session_id} for logout");
            let status = std::process::Command::new("loginctl")
                .args(["terminate-session", &session_id])
                .status();
            if let Ok(status) = status {
                if status.success() {
                    info!("Successfully logged out via loginctl with XDG_SESSION_ID");
                    return;
                }
                warn!("loginctl terminate-session failed with status: {status}");
            } else {
                error!("Failed to execute loginctl terminate-session command");
            }
        }
    } else {
        debug!("XDG_SESSION_ID environment variable not set");
    }

    // Second try: Use GNOME session quit command
    if let Some(path) = which("gnome-session-quit") {
        debug!("Using gnome-session-quit at {} for logout", path.display());
        let status = std::process::Command::new(path).arg("--logout").status();
        if let Ok(status) = status {
            if status.success() {
                info!("Successfully logged out via gnome-session-quit");
                return;
            }
            warn!("gnome-session-quit failed with status: {status}");
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
    if user.is_empty() {
        warn!("Cannot determine current user for logout");
    } else {
        info!("Logging out user {user} via loginctl terminate-user");
        if let Err(e) = std::process::Command::new("loginctl")
            .args(["terminate-user", &user])
            .spawn()
        {
            error!("Failed to execute loginctl terminate-user: {e}");
        } else {
            info!("Successfully initiated logout for user {user}");
        }
    }
}
