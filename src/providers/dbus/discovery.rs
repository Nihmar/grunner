//! Provider discovery for GNOME Shell search providers

use crate::core::global_state::get_home_dir;
use crate::utils::desktop::resolve_icon_from_desktop;
use log::{debug, info, warn};
use std::path::PathBuf;

use super::types::SearchProvider;

/// Discover all available GNOME Shell search providers
///
/// Scans standard directories for .ini files describing search providers,
/// parses them, and filters out any providers in the blacklist.
#[must_use]
pub fn discover_providers(blacklist: &[String]) -> Vec<SearchProvider> {
    let home = get_home_dir();
    let dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/gnome-shell/search-providers"),
        PathBuf::from(format!("{home}/.local/share/gnome-shell/search-providers")),
        PathBuf::from("/var/lib/flatpak/exports/share/gnome-shell/search-providers"),
        PathBuf::from(format!(
            "{home}/.local/share/flatpak/exports/share/gnome-shell/search-providers"
        )),
    ];

    debug!("Discovering search providers, blacklist: {blacklist:?}");
    let mut providers = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            debug!(
                "Skipping non-directory or missing directory: {}",
                dir.display()
            );
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read directory {}: {e}", dir.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ini") {
                if let Some(p) = parse_ini(&path) {
                    if blacklist.iter().any(|b| b == &p.desktop_id) {
                        debug!("Skipping blacklisted provider: {}", p.desktop_id);
                        continue;
                    }
                    if p.default_disabled {
                        debug!(
                            "Provider {} has DefaultDisabled=true; including anyway",
                            p.desktop_id
                        );
                    }
                    debug!(
                        "Discovered provider: {} from {}",
                        p.desktop_id,
                        path.display()
                    );
                    providers.push(p);
                } else {
                    debug!("Failed to parse provider .ini file: {}", path.display());
                }
            }
        }
    }
    info!("Discovered {} search providers", providers.len());
    providers
}

fn parse_ini(path: &std::path::Path) -> Option<SearchProvider> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            debug!("Failed to read .ini file {}: {e}", path.display());
            return None;
        }
    };
    let mut bus_name = None;
    let mut object_path = None;
    let mut desktop_id = None;
    let mut version: Option<u32> = None;
    let mut default_disabled = false;

    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("BusName=") {
            bus_name = Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("ObjectPath=") {
            object_path = Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("DesktopId=") {
            desktop_id = Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("Version=") {
            version = v.parse().ok();
        }
        if let Some(v) = line.strip_prefix("DefaultDisabled=") {
            default_disabled = v.eq_ignore_ascii_case("true");
        }
    }

    if version != Some(2) {
        if let Some(v) = version {
            debug!(
                "Skipping provider {} with unsupported version {v}",
                path.display()
            );
        } else {
            debug!("Skipping provider {} with missing version", path.display());
        }
        return None;
    }

    let Some(desktop_id) = desktop_id else {
        debug!("Provider {} missing DesktopId field", path.display());
        return None;
    };

    let Some(bus_name) = bus_name else {
        debug!("Provider {} missing BusName field", path.display());
        return None;
    };

    let Some(object_path) = object_path else {
        debug!("Provider {} missing ObjectPath field", path.display());
        return None;
    };

    debug!(
        "Successfully parsed provider: {} from {} (default_disabled: {default_disabled})",
        desktop_id,
        path.display()
    );
    Some(SearchProvider {
        bus_name,
        object_path,
        app_icon: resolve_icon_from_desktop(&desktop_id),
        desktop_id,
        default_disabled,
    })
}
