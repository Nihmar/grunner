use rusqlite;
use serde_json::Value;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

/// Main entry point: returns all bookmarks from all supported browsers.
pub fn load_all_bookmarks() -> Vec<Bookmark> {
    let mut all = Vec::new();

    // Firefox (native and flatpak)
    for profile in firefox_profiles() {
        if let Ok(bookmarks) = firefox_bookmarks(&profile) {
            all.extend(bookmarks);
        }
    }

    // Chromium-based browsers: Chrome, Brave, Zen (native and flatpak)
    for browser in &[
        "google-chrome",
        "chromium",
        "BraveSoftware/Brave-Browser",
        "zen",
    ] {
        if let Some(bookmarks) = chromium_based_bookmarks(browser) {
            all.extend(bookmarks);
        }
    }

    all
}

// ---------- Firefox (native + flatpak) ----------
fn firefox_profiles() -> Vec<PathBuf> {
    let mut profiles = Vec::new();

    // Native Firefox
    if let Some(moz_dir) = dirs::home_dir().map(|h| h.join(".mozilla/firefox")) {
        if moz_dir.exists() {
            for entry in std::fs::read_dir(moz_dir).ok().into_iter().flatten() {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() && path.join("places.sqlite").exists() {
                        profiles.push(path);
                    }
                }
            }
        }
    }

    // Flatpak Firefox
    let flatpak_base =
        dirs::home_dir().map(|h| h.join(".var/app/org.mozilla.firefox/.mozilla/firefox"));
    if let Some(moz_dir) = flatpak_base {
        if moz_dir.exists() {
            for entry in std::fs::read_dir(moz_dir).ok().into_iter().flatten() {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() && path.join("places.sqlite").exists() {
                        profiles.push(path);
                    }
                }
            }
        }
    }

    profiles
}

fn firefox_bookmarks(profile_path: &Path) -> rusqlite::Result<Vec<Bookmark>> {
    let conn = rusqlite::Connection::open(profile_path.join("places.sqlite"))?;
    let mut stmt = conn.prepare(
        "SELECT b.title, p.url FROM moz_bookmarks b
         JOIN moz_places p ON b.fk = p.id
         WHERE b.type = 1 AND b.title IS NOT NULL AND b.title != ''",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Bookmark {
            title: row.get(0)?,
            url: row.get(1)?,
        })
    })?;
    let mut bookmarks = Vec::new();
    for row in rows {
        bookmarks.push(row?);
    }
    Ok(bookmarks)
}

// ---------- Chromium-based browsers ----------
fn chromium_based_bookmarks(browser_key: &str) -> Option<Vec<Bookmark>> {
    // Try native config
    let native_path = dirs::config_dir()?
        .join(browser_key)
        .join("Default")
        .join("Bookmarks");
    if native_path.exists() {
        if let Ok(content) = std::fs::read_to_string(native_path) {
            if let Some(b) = parse_chrome_bookmarks(&content) {
                return Some(b);
            }
        }
    }

    // Try flatpak paths for common browsers
    let flatpak_map = [
        ("google-chrome", "com.google.Chrome"),
        ("chromium", "org.chromium.Chromium"),
        ("BraveSoftware/Brave-Browser", "com.brave.Browser"),
        ("zen", "io.github.zen_browser.zen"),
    ];
    for (key, flatpak_id) in flatpak_map {
        if key == browser_key {
            let flatpak_path = dirs::home_dir()?
                .join(format!(".var/app/{}/config/{}", flatpak_id, browser_key))
                .join("Default")
                .join("Bookmarks");
            if flatpak_path.exists() {
                if let Ok(content) = std::fs::read_to_string(flatpak_path) {
                    return parse_chrome_bookmarks(&content);
                }
            }
        }
    }

    None
}

fn parse_chrome_bookmarks(json: &str) -> Option<Vec<Bookmark>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let roots = v.get("roots")?.as_object()?;
    let mut bookmarks = Vec::new();
    for root in roots.values() {
        extract_bookmarks_from_node(root, &mut bookmarks);
    }
    Some(bookmarks)
}

fn extract_bookmarks_from_node(node: &Value, out: &mut Vec<Bookmark>) {
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            extract_bookmarks_from_node(child, out);
        }
    } else if let Some(url) = node.get("url").and_then(|u| u.as_str()) {
        if let Some(name) = node.get("name").and_then(|n| n.as_str()) {
            out.push(Bookmark {
                title: name.to_string(),
                url: url.to_string(),
            });
        }
    }
}
