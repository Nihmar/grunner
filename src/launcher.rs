use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub description: String,
    pub icon: String,
    pub terminal: bool,
}

/// Loads all apps from the given list of directories.
/// Directories that do not exist are silently skipped.
/// Duplicate .desktop files (by full path) are avoided.
pub fn load_apps(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    let mut apps = Vec::new();
    let mut seen = HashSet::new();   // deduplica per percorso file

    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if seen.insert(path.clone()) {
                if let Some(app) = parse_desktop_file(&path) {
                    apps.push(app);
                }
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

fn parse_desktop_file(path: &Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;

    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut description = String::new();
    let mut icon = String::new();
    let mut app_type = String::new();
    let mut no_display = false;
    let mut hidden = false;
    let mut terminal = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') && line != "[Desktop Entry]" {
            if in_desktop_entry {
                break;
            }
            continue;
        }
        if !in_desktop_entry {
            continue;
        }

        if let Some(val) = line.strip_prefix("Type=") {
            app_type = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Name=") {
            if name.is_none() {
                name = Some(val.trim().to_string());
            }
        } else if let Some(val) = line.strip_prefix("Exec=") {
            exec = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Comment=") {
            if description.is_empty() {
                description = val.trim().to_string();
            }
        } else if let Some(val) = line.strip_prefix("Icon=") {
            if icon.is_empty() {
                icon = val.trim().to_string();
            }
        } else if let Some(val) = line.strip_prefix("NoDisplay=") {
            no_display = val.trim().eq_ignore_ascii_case("true");
        } else if let Some(val) = line.strip_prefix("Hidden=") {
            hidden = val.trim().eq_ignore_ascii_case("true");
        } else if let Some(val) = line.strip_prefix("Terminal=") {
            terminal = val.trim().eq_ignore_ascii_case("true");
        }
    }

    if app_type != "Application" || no_display || hidden {
        return None;
    }

    Some(DesktopApp {
        name: name?,
        exec: exec?,
        description,
        icon,
        terminal,
    })
}

/// Cleans up an Exec= value by removing field codes like %f %F %u %U …
pub fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|token| {
            !matches!(
                *token,
                "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%i" | "%c" | "%k" | "%v" | "%m"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}