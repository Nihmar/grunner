use jwalk::WalkDir;
use rayon::prelude::*;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub description: String,
    pub icon: String,
    pub terminal: bool,
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

fn cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".cache")
        .join("grunner")
        .join("apps.bin")
}

/// Returns the most recent mtime across all directories that actually exist.
fn dirs_max_mtime(dirs: &[PathBuf]) -> Option<SystemTime> {
    dirs.iter()
        .filter_map(|d| fs::metadata(d).ok()?.modified().ok())
        .max()
}

/// Returns the cached app list if it is still fresh (i.e. no app directory has
/// been modified since the cache was written).
fn try_load_cache(dirs: &[PathBuf]) -> Option<Vec<DesktopApp>> {
    let cache = cache_path();
    let cache_mtime = fs::metadata(&cache).ok()?.modified().ok()?;
    let dirs_mtime = dirs_max_mtime(dirs)?;
    if dirs_mtime > cache_mtime {
        return None; // at least one directory is newer — rebuild
    }
    let bytes = fs::read(&cache).ok()?;
    bincode::deserialize(&bytes).ok()
}

fn save_cache(apps: &[DesktopApp]) {
    let path = cache_path();
    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("Failed to create cache dir: {}", e);
            return;
        }
    }
    match bincode::serialize(apps) {
        Ok(bytes) => {
            if let Err(e) = fs::write(&path, &bytes) {
                eprintln!("Failed to write app cache: {}", e);
            }
        }
        Err(e) => eprintln!("Failed to serialize app cache: {}", e),
    }
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Scans all directories in parallel, deduplicating by file path.
fn scan_apps(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    // First pass (parallel): collect all .desktop paths from each directory.
    let paths: Vec<PathBuf> = dirs
        .par_iter()
        .filter(|d| d.exists())
        .flat_map(|dir| {
            WalkDir::new(dir)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("desktop"))
                .map(|e| e.path())
                .collect::<Vec<_>>()
        })
        .collect();

    // Deduplicate sequentially using a fast hash set.
    let mut seen = FxHashSet::default();
    let unique_paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect();

    // Second pass (parallel): parse .desktop files.
    let mut apps: Vec<DesktopApp> = unique_paths
        .par_iter()
        .filter_map(|p| parse_desktop_file(p))
        .collect();

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Loads all apps from the given list of directories.
///
/// On a cache hit the result is returned immediately from disk without any
/// directory scanning. On a cache miss the directories are scanned in parallel
/// and the result is written back to the cache before returning.
pub fn load_apps(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    if let Some(cached) = try_load_cache(dirs) {
        return cached;
    }
    let apps = scan_apps(dirs);
    save_cache(&apps);
    apps
}

// ---------------------------------------------------------------------------
// .desktop file parser (unchanged)
// ---------------------------------------------------------------------------

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
                "%f" | "%F"
                    | "%u"
                    | "%U"
                    | "%d"
                    | "%D"
                    | "%n"
                    | "%N"
                    | "%i"
                    | "%c"
                    | "%k"
                    | "%v"
                    | "%m"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}
