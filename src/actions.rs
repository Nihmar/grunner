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

fn which(prog: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(prog))
        .find(|p| is_executable(p))
}

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
    candidates
        .iter()
        .find(|&&c| which(c).is_some())
        .map(|&c| c.to_string())
}

fn find_terminal() -> Option<String> {
    TERMINAL.clone()
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
        config::load();
    }

    if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
        eprintln!("Failed to open settings with xdg-open: {}", e);
    }
}

static FILE_LINE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+):(\d+):").unwrap());

pub fn open_file_or_line(line: &str) {
    let re = &*FILE_LINE_RE;
    if let Some(caps) = re.captures(line) {
        let file = caps.get(1).unwrap().as_str();
        let line_num = caps.get(2).unwrap().as_str();
        if Path::new(file).exists() {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "xdg-open".to_string());
            let mut cmd = std::process::Command::new(&editor);
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

    if Path::new(line).exists() {
        if let Err(e) = std::process::Command::new("xdg-open").arg(line).spawn() {
            eprintln!("Failed to open file: {}", e);
        }
    } else {
        let display = gtk4::gdk::Display::default().expect("cannot get display");
        let clipboard = display.clipboard();
        clipboard.set_text(line);
    }
}

pub fn perform_obsidian_action(action: ObsidianAction, text: Option<&str>, cfg: &ObsidianConfig) {
    let vault_path = expand_home(&cfg.vault);
    if !vault_path.exists() {
        eprintln!("Vault path does not exist: {}", vault_path.display());
        return;
    }

    match action {
        ObsidianAction::OpenVault => {
            let vault_name = vault_path.file_name().unwrap_or_default().to_string_lossy();
            let uri = format!("obsidian://open?vault={}", urlencoding::encode(&vault_name));
            open_uri(&uri);
        }
        ObsidianAction::NewNote => {
            let folder = vault_path.join(&cfg.new_notes_folder);
            if let Err(e) = fs::create_dir_all(&folder) {
                eprintln!("Cannot create folder {}: {}", folder.display(), e);
                return;
            }
            let now = Local::now();
            let filename = format!("New Note {}.md", now.format("%Y-%m-%d %H-%M-%S"));
            let path = folder.join(filename);

            let mut file = match File::create(&path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Cannot create note {}: {}", path.display(), e);
                    return;
                }
            };
            if let Some(t) = text {
                if !t.is_empty() {
                    if let Err(e) = writeln!(file, "{}", t) {
                        eprintln!("Cannot write to note {}: {}", path.display(), e);
                    }
                }
            }

            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            open_uri(&uri);
        }
        ObsidianAction::DailyNote => {
            let folder = vault_path.join(&cfg.daily_notes_folder);
            if let Err(e) = fs::create_dir_all(&folder) {
                eprintln!("Cannot create folder {}: {}", folder.display(), e);
                return;
            }
            let today = Local::now().format("%Y-%m-%d").to_string();
            let path = folder.join(format!("{}.md", today));
            let mut file = match fs::OpenOptions::new().create(true).append(true).open(&path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Cannot open daily note {}: {}", path.display(), e);
                    return;
                }
            };
            if let Some(t) = text {
                if !t.is_empty() {
                    writeln!(file, "{}", t).ok();
                }
            }
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            open_uri(&uri);
        }
        ObsidianAction::QuickNote => {
            let path = vault_path.join(&cfg.quick_note);
            if let Some(parent) = path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!("Cannot create folder {}: {}", parent.display(), e);
                    return;
                }
            }
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
            let uri = format!(
                "obsidian://open?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            open_uri(&uri);
        }
    }
}

pub fn open_obsidian_file_path(file_path: &str, cfg: &ObsidianConfig) {
    let vault_path = expand_home(&cfg.vault);
    if !vault_path.exists() {
        eprintln!("Vault path does not exist: {}", vault_path.display());
        return;
    }
    let uri = format!("obsidian://open?path={}", urlencoding::encode(file_path));
    open_uri(&uri);
}

pub fn open_obsidian_file_line(file_path: &str, line: &str, cfg: &ObsidianConfig) {
    let vault_path = expand_home(&cfg.vault);
    if !vault_path.exists() {
        eprintln!("Vault path does not exist: {}", vault_path.display());
        return;
    }

    let path = if file_path.starts_with('/') {
        PathBuf::from(file_path)
    } else {
        vault_path.join(file_path)
    };

    let uri = format!(
        "obsidian://open?path={}&line={}",
        urlencoding::encode(&path.to_string_lossy()),
        line
    );
    open_uri(&uri);
}

fn open_uri(uri: &str) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(uri).spawn() {
        eprintln!("Failed to open URI {}: {}", uri, e);
    }
}
