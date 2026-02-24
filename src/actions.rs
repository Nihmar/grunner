use std::path::PathBuf;

use once_cell::sync::Lazy;

use crate::config;
use crate::launcher;

pub static TERMINAL: Lazy<Option<String>> = Lazy::new(find_terminal_impl);

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
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::env::split_paths(&path_var).collect::<Vec<_>>();

    for candidate in candidates {
        for dir in &paths {
            let full = dir.join(candidate);
            if full.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = std::fs::metadata(&full) {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return Some(candidate.to_string());
                        }
                    }
                }
                #[cfg(not(unix))]
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn find_terminal() -> Option<String> {
    TERMINAL.clone()
}

/// Helper: find an executable in PATH
fn which(prog: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let paths = std::env::split_paths(&path_var);
    for dir in paths {
        let full = dir.join(prog);
        if full.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata(&full) {
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return Some(full);
                    }
                }
            }
            #[cfg(not(unix))]
            return Some(full);
        }
    }
    None
}

pub fn launch_app(exec: &str, terminal: bool) {
    let clean = launcher::clean_exec(exec);
    if terminal {
        if let Some(term) = find_terminal() {
            let mut cmd = std::process::Command::new(&term);
            match term.as_str() {
                "gnome-terminal" | "xfce4-terminal" => {
                    cmd.arg("--").arg("sh").arg("-c").arg(&clean);
                }
                "konsole" | "alacritty" | "foot" => {
                    cmd.arg("-e").arg("sh").arg("-c").arg(&clean);
                }
                "kitty" => {
                    cmd.arg("--").arg("sh").arg("-c").arg(&clean);
                }
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
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&clean);
        if let Err(e) = cmd.spawn() {
            eprintln!("Failed to launch {}: {}", clean, e);
        }
    }
}

pub fn power_action(action: &str) {
    match action {
        "logout" => logout_action(),
        "suspend" => {
            if let Err(e) = std::process::Command::new("systemctl")
                .arg("suspend")
                .spawn()
            {
                eprintln!("Failed to suspend: {}", e);
            }
        }
        "reboot" => {
            if let Err(e) = std::process::Command::new("systemctl")
                .arg("reboot")
                .spawn()
            {
                eprintln!("Failed to reboot: {}", e);
            }
        }
        "poweroff" => {
            if let Err(e) = std::process::Command::new("systemctl")
                .arg("poweroff")
                .spawn()
            {
                eprintln!("Failed to power off: {}", e);
            }
        }
        _ => {}
    }
}

fn logout_action() {
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

    if let Some(path) = which("gnome-session-quit") {
        let status = std::process::Command::new(path).arg("--logout").status();
        if let Ok(status) = status {
            if status.success() {
                return;
            }
        }
    }

    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default();
    if !user.is_empty() {
        let _ = std::process::Command::new("loginctl")
            .args(["terminate-user", &user])
            .spawn();
    }
}

pub fn open_settings() {
    let path = config::config_path();

    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("Failed to create config dir: {}", e);
        }
    }
    if !path.exists() {
        config::load(); // writes default file
    }

    if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
        eprintln!("Failed to open settings with xdg-open: {}", e);
    }
}