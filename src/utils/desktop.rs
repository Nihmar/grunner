//! Desktop file utilities for Grunner
//!
//! This module provides utilities for reading and parsing .desktop files
//! to extract application metadata like names and icons.

use crate::core::global_state::get_home_dir;

const DESKTOP_SEARCH_DIRS: &[&str] = &[
    "/usr/share/applications",
    "/usr/local/share/applications",
    "/var/lib/flatpak/exports/share/applications",
];

const USER_DESKTOP_DIRS: &[&str] = &[
    ".local/share/applications",
    ".local/share/flatpak/exports/share/applications",
];

pub struct DesktopInfo {
    pub name: String,
    pub icon: Option<String>,
}

#[must_use]
pub fn resolve_desktop_info(desktop_id: &str) -> Option<DesktopInfo> {
    let home = get_home_dir();

    let filename = if desktop_id.ends_with(".desktop") {
        desktop_id.to_string()
    } else {
        format!("{desktop_id}.desktop")
    };

    for dir in DESKTOP_SEARCH_DIRS {
        let path = format!("{dir}/{filename}");
        if let Some(info) = parse_desktop_file(&path) {
            return Some(info);
        }
    }

    for dir in USER_DESKTOP_DIRS {
        let path = format!("{home}/{dir}/{filename}");
        if let Some(info) = parse_desktop_file(&path) {
            return Some(info);
        }
    }

    None
}

#[must_use]
pub fn resolve_icon_from_desktop(desktop_id: &str) -> String {
    resolve_desktop_info(desktop_id)
        .map(|info| info.icon.unwrap_or_default())
        .unwrap_or_default()
}

fn parse_desktop_file(path: &str) -> Option<DesktopInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    let mut name: Option<String> = None;
    let mut icon: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
        } else if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = false;
        }
        if !in_desktop_entry {
            continue;
        }
        if name.is_none() && line.starts_with("Name=") {
            name = Some(line.strip_prefix("Name=")?.trim().to_string());
        }
        if line.starts_with("Icon=") {
            icon = Some(line.strip_prefix("Icon=")?.trim().to_string());
        }
    }

    Some(DesktopInfo { name: name?, icon })
}
