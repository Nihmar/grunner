//! Desktop application launcher and caching system for Grunner
//!
//! This module is responsible for scanning, parsing, and caching desktop application
//! entries from `.desktop` files. It provides efficient application discovery with
//! intelligent caching to improve startup performance.
//!
//! Key features:
//! - Parallel scanning of application directories using Rayon
//! - Binary caching of parsed applications for fast subsequent loads
//! - Proper handling of desktop entry specifications
//! - Filtering of non-application and hidden entries

use jwalk::WalkDir;
use rayon::prelude::*;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Represents a parsed desktop application entry
///
/// This struct contains the essential information extracted from a `.desktop` file
/// needed for launching and displaying applications in the Grunner launcher.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesktopApp {
    /// Display name of the application (from the `Name=` field)
    pub name: String,
    /// Command to execute when launching the application (from the `Exec=` field)
    pub exec: String,
    /// Description or comment about the application (from the `Comment=` field)
    pub description: String,
    /// Icon name or path for the application (from the `Icon=` field)
    pub icon: String,
    /// Whether the application should be launched in a terminal (from `Terminal=` field)
    pub terminal: bool,
}

/// Get the path to the application cache file
///
/// The cache is stored in the user's cache directory at:
/// `$HOME/.cache/grunner/apps.bin`
///
/// # Returns
/// `PathBuf` pointing to the cache file location
fn cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".cache")
        .join("grunner")
        .join("apps.bin")
}

/// Get the maximum modification time among a list of directories
///
/// This is used to determine if the cache is stale by comparing the
/// cache file's modification time with the most recently modified
/// application directory.
///
/// # Arguments
/// * `dirs` - Slice of directory paths to check
///
/// # Returns
/// `Some(SystemTime)` with the latest modification time if all directories
/// exist and have readable metadata, `None` otherwise.
fn dirs_max_mtime(dirs: &[PathBuf]) -> Option<SystemTime> {
    dirs.iter()
        .filter_map(|d| fs::metadata(d).ok()?.modified().ok())
        .max()
}

/// Attempt to load applications from cache if it's still valid
///
/// The cache is considered valid if:
/// 1. The cache file exists and is readable
/// 2. The cache file is newer than all application directories
///
/// # Arguments
/// * `dirs` - Application directories that would be scanned if cache is invalid
///
/// # Returns
/// `Some(Vec<DesktopApp>)` if cache is valid and loaded successfully,
/// `None` if cache is stale, missing, or corrupt.
fn try_load_cache(dirs: &[PathBuf]) -> Option<Vec<DesktopApp>> {
    let cache = cache_path();

    // Get cache file modification time
    let cache_mtime = fs::metadata(&cache).ok()?.modified().ok()?;

    // Get latest directory modification time
    let dirs_mtime = dirs_max_mtime(dirs)?;

    // Cache is stale if directories were modified after cache was created
    if dirs_mtime > cache_mtime {
        return None;
    }

    // Read and deserialize cache
    let bytes = fs::read(&cache).ok()?;
    bincode::deserialize(&bytes).ok()
}

/// Save parsed applications to cache for faster future loads
///
/// # Arguments
/// * `apps` - Vector of desktop applications to cache
///
/// The cache is written as a binary serialized format using bincode
/// for fast reading/writing and compact storage.
fn save_cache(apps: &[DesktopApp]) {
    let path = cache_path();

    // Ensure cache directory exists
    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("Failed to create cache dir: {}", e);
            return;
        }
    }

    // Serialize and write cache
    match bincode::serialize(apps) {
        Ok(bytes) => {
            if let Err(e) = fs::write(&path, &bytes) {
                eprintln!("Failed to write app cache: {}", e);
            }
        }
        Err(e) => eprintln!("Failed to serialize app cache: {}", e),
    }
}

/// Scan application directories for `.desktop` files and parse them
///
/// This function performs the actual filesystem scanning and parsing:
/// 1. Walks each directory recursively to find all `.desktop` files
/// 2. Uses parallel processing (Rayon) for faster scanning
/// 3. Removes duplicate paths (same file accessed via symlinks or multiple dirs)
/// 4. Parses each `.desktop` file in parallel
/// 5. Sorts applications alphabetically by name (case-insensitive)
///
/// # Arguments
/// * `dirs` - Directories to scan for `.desktop` files
///
/// # Returns
/// Vector of parsed `DesktopApp` instances
fn scan_apps(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    // Collect all .desktop file paths using parallel iteration
    let paths: Vec<PathBuf> = dirs
        .par_iter()
        .filter(|d| d.exists()) // Skip non-existent directories
        .flat_map(|dir| {
            WalkDir::new(dir)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("desktop"))
                .map(|e| e.path())
                .collect::<Vec<_>>()
        })
        .collect();

    // Remove duplicate paths using a hash set for deduplication
    let mut seen = FxHashSet::default();
    let unique_paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect();

    // Parse desktop files in parallel and collect valid applications
    let mut apps: Vec<DesktopApp> = unique_paths
        .par_iter()
        .filter_map(|p| parse_desktop_file(p))
        .collect();

    // Sort applications alphabetically for consistent UI presentation
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Main entry point for loading desktop applications
///
/// This function implements the caching strategy:
/// 1. Try to load from cache if it exists and is valid
/// 2. If cache is invalid or missing, scan and parse directories
/// 3. Save fresh scan results to cache for next time
///
/// # Arguments
/// * `dirs` - Directories to scan for `.desktop` files
///
/// # Returns
/// Vector of `DesktopApp` instances ready for display and launching
pub fn load_apps(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    // First attempt to load from cache
    if let Some(cached) = try_load_cache(dirs) {
        return cached;
    }

    // Cache miss or invalid - perform fresh scan
    let apps = scan_apps(dirs);

    // Save to cache for future use
    save_cache(&apps);
    apps
}

/// Parse a single `.desktop` file into a `DesktopApp` struct
///
/// This function implements a subset of the Desktop Entry Specification:
/// https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html
///
/// It extracts only the fields needed by Grunner and filters out:
/// - Non-application entries (Type != "Application")
/// - Hidden entries (Hidden=true or NoDisplay=true)
///
/// # Arguments
/// * `path` - Path to the `.desktop` file to parse
///
/// # Returns
/// `Some(DesktopApp)` if the file is a valid, displayable application,
/// `None` if it's not an application or should be hidden.
fn parse_desktop_file(path: &Path) -> Option<DesktopApp> {
    // Read file content
    let content = fs::read_to_string(path).ok()?;

    // Initialize parser state
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut description = String::new();
    let mut icon = String::new();
    let mut app_type = String::new();
    let mut no_display = false;
    let mut hidden = false;
    let mut terminal = false;
    let mut in_desktop_entry = false;

    // Parse file line by line
    for line in content.lines() {
        let line = line.trim();

        // Section detection
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        // Exit Desktop Entry section if we encounter another section
        if line.starts_with('[') && line != "[Desktop Entry]" {
            if in_desktop_entry {
                break;
            }
            continue;
        }
        // Skip lines outside Desktop Entry section
        if !in_desktop_entry {
            continue;
        }

        // Parse key-value pairs
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

    // Filter out non-applications and hidden entries
    if app_type != "Application" || no_display || hidden {
        return None;
    }

    // Return parsed application (requires at least name and exec)
    Some(DesktopApp {
        name: name?,
        exec: exec?,
        description,
        icon,
        terminal,
    })
}

/// Clean desktop execution command by removing field codes
///
/// Desktop entry `Exec` fields can contain special field codes like `%f`, `%u`, etc.
/// This function removes those codes to get a plain command string that can
/// be executed directly.
///
/// # Arguments
/// * `exec` - Raw Exec string from `.desktop` file
///
/// # Returns
/// Cleaned command string with field codes removed
///
/// # Field Codes Removed
/// - `%f`, `%F` - Single/multiple file arguments
/// - `%u`, `%U` - Single/multiple URL arguments
/// - `%d`, `%D` - Directory arguments
/// - `%n`, `%N` - Translated names
/// - `%i`, `%c`, `%k`, `%v`, `%m` - Various other codes
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
