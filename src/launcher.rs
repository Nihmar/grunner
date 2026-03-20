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

use crate::core::global_state::get_home_dir;
use jwalk::WalkDir;
use log::{debug, error, info, trace};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Represents a parsed desktop application entry
///
/// This struct contains the essential information extracted from a `.desktop` file
/// needed for launching and displaying applications in the Grunner launcher.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesktopApp {
    /// Desktop entry ID (filename without .desktop extension, slashes replaced with dashes)
    pub desktop_id: String,
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
    let home = get_home_dir();
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
    debug!("Checking application cache at {}", cache.display());

    // Get cache file modification time
    let cache_mtime = match fs::metadata(&cache) {
        Ok(metadata) => match metadata.modified() {
            Ok(mtime) => mtime,
            Err(e) => {
                debug!("Failed to get cache file modification time: {e}");
                return None;
            }
        },
        Err(e) => {
            debug!("Cache file not found or inaccessible: {e}");
            return None;
        }
    };

    // Get latest directory modification time
    let Some(dirs_mtime) = dirs_max_mtime(dirs) else {
        debug!("Failed to get directory modification times");
        return None;
    };

    // Cache is stale if directories were modified after cache was created
    if dirs_mtime > cache_mtime {
        info!("Cache is stale (dirs modified after cache creation)");
        return None;
    }

    // Read cache file
    let bytes = match fs::read(&cache) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read cache file: {e}");
            return None;
        }
    };

    // Deserialize cache
    match bincode::deserialize::<Vec<DesktopApp>>(&bytes) {
        Ok(apps) => {
            info!("Loaded {} applications from cache", apps.len());
            Some(apps)
        }
        Err(e) => {
            error!("Failed to deserialize cache: {e}");
            None
        }
    }
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
    debug!(
        "Saving {} applications to cache at {}",
        apps.len(),
        path.display()
    );

    // Ensure cache directory exists
    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            error!("Failed to create cache directory {}: {e}", dir.display());
            return;
        }
        debug!("Created cache directory: {}", dir.display());
    }

    // Serialize and write cache
    match bincode::serialize(apps) {
        Ok(bytes) => {
            let len = bytes.len();
            debug!("Serialized {len} bytes of cache data");
            if let Err(e) = fs::write(&path, &bytes) {
                error!("Failed to write cache to {}: {e}", path.display());
            } else {
                let len = apps.len();
                info!("Saved {len} applications to cache");
            }
        }
        Err(e) => {
            error!("Failed to serialize cache: {e}");
        }
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
    info!("Scanning {} directories for .desktop files", dirs.len());

    // Only use parallel iteration for larger workloads to avoid thread pool overhead
    // For small directory counts, sequential processing is more efficient
    let use_parallel = dirs.len() > 4;

    // Collect all .desktop file paths
    let paths: Vec<PathBuf> = if use_parallel {
        dirs.par_iter()
            .filter(|d| {
                let exists = d.exists();
                if !exists {
                    debug!("Skipping non-existent directory: {}", d.display());
                }
                exists
            })
            .flat_map(|dir| {
                debug!("Scanning directory: {}", dir.display());
                WalkDir::new(dir)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| {
                        e.path().extension().and_then(|ext| ext.to_str()) == Some("desktop")
                    })
                    .map(|e| e.path())
                    .collect::<Vec<_>>()
            })
            .collect()
    } else {
        dirs.iter()
            .filter(|d| {
                let exists = d.exists();
                if !exists {
                    debug!("Skipping non-existent directory: {}", d.display());
                }
                exists
            })
            .flat_map(|dir| {
                debug!("Scanning directory: {}", dir.display());
                WalkDir::new(dir)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| {
                        e.path().extension().and_then(|ext| ext.to_str()) == Some("desktop")
                    })
                    .map(|e| e.path())
                    .collect::<Vec<_>>()
            })
            .collect()
    };

    debug!("Found {} .desktop files before deduplication", paths.len());

    // Remove duplicate paths using a hash set for deduplication
    let mut seen = HashSet::new();
    let unique_paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect();

    debug!(
        "{} unique .desktop files after deduplication",
        unique_paths.len()
    );

    // Parse desktop files - use parallel iteration only for larger workloads
    let use_parallel_parsing = unique_paths.len() > 50;
    let mut apps: Vec<DesktopApp> = if use_parallel_parsing {
        unique_paths
            .par_iter()
            .filter_map(|p| parse_desktop_file(p))
            .collect()
    } else {
        unique_paths
            .iter()
            .filter_map(|p| parse_desktop_file(p))
            .collect()
    };

    debug!("Successfully parsed {} applications", apps.len());

    // Sort applications alphabetically for consistent UI presentation
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    info!(
        "Scanned {} applications from {} directories",
        apps.len(),
        dirs.len()
    );
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
#[must_use]
pub fn load_apps(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    // First attempt to load from cache
    if let Some(cached) = try_load_cache(dirs) {
        info!("Cache hit: loaded {} applications from cache", cached.len());
        return cached;
    }

    info!("Cache miss or invalid, scanning application directories");
    // Cache miss or invalid - perform fresh scan
    let apps = scan_apps(dirs);
    info!(
        "Scanned {} applications from {} directories",
        apps.len(),
        dirs.len()
    );

    // Save to cache for future use
    save_cache(&apps);
    apps
}

/// Parse a single `.desktop` file into a `DesktopApp` struct
///
/// This function implements a subset of the Desktop Entry Specification:
/// <https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html>
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
pub(crate) fn parse_desktop_file(path: &Path) -> Option<DesktopApp> {
    // Read file content
    trace!("Parsing desktop file: {}", path.display());
    let content = fs::read_to_string(path).ok()?;

    // Derive desktop entry ID from filename
    let desktop_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .replace('/', "-");

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
    if app_type != "Application" {
        trace!(
            "Skipping non-application entry (type: {app_type}) in {}",
            path.display()
        );
        return None;
    }
    if no_display {
        trace!("Skipping NoDisplay=true entry in {}", path.display());
        return None;
    }
    if hidden {
        trace!("Skipping Hidden=true entry in {}", path.display());
        return None;
    }

    // Return parsed application (requires at least name and exec)
    let Some(name) = name else {
        debug!("Missing Name field in desktop file {}", path.display());
        return None;
    };
    let Some(exec) = exec else {
        debug!("Missing Exec field in desktop file {}", path.display());
        return None;
    };

    trace!(
        "Successfully parsed desktop application: {name} from {}",
        path.display()
    );
    Some(DesktopApp {
        desktop_id,
        name,
        exec,
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
#[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── clean_exec tests ──────────────────────────────────────────────

    #[test]
    fn test_clean_exec_no_codes() {
        assert_eq!(clean_exec("firefox"), "firefox");
    }

    #[test]
    fn test_clean_exec_with_file_code() {
        assert_eq!(clean_exec("firefox %f"), "firefox");
    }

    #[test]
    fn test_clean_exec_with_url_code() {
        assert_eq!(clean_exec("gedit %U"), "gedit");
    }

    #[test]
    fn test_clean_exec_multiple_codes() {
        assert_eq!(clean_exec("app %f %u %i %c"), "app");
    }

    #[test]
    fn test_clean_exec_all_codes() {
        assert_eq!(
            clean_exec("cmd %f %F %u %U %d %D %n %N %i %c %k %v %m"),
            "cmd"
        );
    }

    #[test]
    fn test_clean_exec_only_codes() {
        assert_eq!(clean_exec("%f"), "");
    }

    #[test]
    fn test_clean_exec_empty() {
        assert_eq!(clean_exec(""), "");
    }

    #[test]
    fn test_clean_exec_no_codes_passthrough() {
        assert_eq!(clean_exec("no-codes-here"), "no-codes-here");
    }

    #[test]
    fn test_clean_exec_trims_extra_spaces() {
        assert_eq!(clean_exec("firefox  %f"), "firefox");
    }

    #[test]
    fn test_clean_exec_command_with_args() {
        assert_eq!(clean_exec("python3 -m myapp %u"), "python3 -m myapp");
    }

    // ── parse_desktop_file tests ──────────────────────────────────────

    fn write_temp_desktop(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_parse_valid_desktop_file() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_valid");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "test-app.desktop",
            "[Desktop Entry]\nType=Application\nName=Test App\nExec=test-app %f\nIcon=test-icon\nComment=A test application\n",
        );

        let app = parse_desktop_file(&path).unwrap();
        assert_eq!(app.name, "Test App");
        assert_eq!(app.exec, "test-app %f");
        assert_eq!(app.icon, "test-icon");
        assert_eq!(app.description, "A test application");
        assert!(!app.terminal);
        assert_eq!(app.desktop_id, "test-app");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_link_type() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_link");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "link.desktop",
            "[Desktop Entry]\nType=Link\nName=Link\nURL=http://example.com\n",
        );

        assert!(parse_desktop_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_no_display() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_nodisplay");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "hidden.desktop",
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nNoDisplay=true\n",
        );

        assert!(parse_desktop_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_hidden() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_hidden");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "hidden2.desktop",
            "[Desktop Entry]\nType=Application\nName=Hidden2\nExec=hidden2\nHidden=true\n",
        );

        assert!(parse_desktop_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_missing_name() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_noname");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "noname.desktop",
            "[Desktop Entry]\nType=Application\nExec=noname\n",
        );

        assert!(parse_desktop_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_missing_exec() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_noexec");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "noexec.desktop",
            "[Desktop Entry]\nType=Application\nName=NoExec\n",
        );

        assert!(parse_desktop_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_terminal_true() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_terminal");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "term.desktop",
            "[Desktop Entry]\nType=Application\nName=Terminal App\nExec=term-app\nTerminal=true\n",
        );

        let app = parse_desktop_file(&path).unwrap();
        assert!(app.terminal);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_multiple_sections() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_multi");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "multi.desktop",
            "[Desktop Entry]\nType=Application\nName=Multi\nExec=multi\n\n[Another Section]\nFoo=bar\n",
        );

        let app = parse_desktop_file(&path).unwrap();
        assert_eq!(app.name, "Multi");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_desktop_file_nonexistent() {
        let path = Path::new("/nonexistent/path/app.desktop");
        assert!(parse_desktop_file(path).is_none());
    }

    #[test]
    fn test_parse_desktop_file_desktop_id_with_slashes() {
        let dir = std::env::temp_dir().join("grunner_test_desktop_slashes");
        let _ = fs::create_dir_all(&dir);
        let path = write_temp_desktop(
            &dir,
            "org.example.App.desktop",
            "[Desktop Entry]\nType=Application\nName=Example\nExec=example\n",
        );

        let app = parse_desktop_file(&path).unwrap();
        assert_eq!(app.desktop_id, "org.example.App");
        let _ = fs::remove_dir_all(&dir);
    }
}
