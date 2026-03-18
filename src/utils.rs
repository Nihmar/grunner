//! Utility functions for Grunner
//!
//! This module provides general-purpose helper functions used throughout
//! the application. Currently, it contains path manipulation utilities
//! for handling user home directory expansion, calculator result parsing,
//! and icon selection.

use crate::core::global_state::get_home_dir;
use gtk4::gio;
use std::path::PathBuf;

/// Expand a path starting with `~` to the user's home directory
///
/// This function replaces the tilde (`~`) prefix in a path string with
/// the current user's home directory path obtained from the `HOME`
/// environment variable. It handles two forms:
/// - `~/something` → `$HOME/something`
/// - `~` → `$HOME`
///
/// If the path doesn't start with `~`, it's returned unchanged as a `PathBuf`.
///
/// # Arguments
/// * `path` - A path string that may optionally start with `~` or `~/`
///
/// # Returns
/// A `PathBuf` with the home directory expanded if applicable.
///
/// # Examples
/// ```
/// # use grunner::utils::expand_home;
/// # // Note: actual HOME value depends on environment
/// # // With HOME = "/home/alice":
/// # // expand_home("~/Documents") → PathBuf::from("/home/alice/Documents")
/// # // expand_home("~") → PathBuf::from("/home/alice")
/// # // expand_home("/etc/fstab") → PathBuf::from("/etc/fstab") (unchanged)
/// ```
///
/// # Environment
/// Relies on the `HOME` environment variable. If `HOME` is not set,
/// defaults to an empty string, which may result in unexpected paths.
#[must_use]
pub fn expand_home(path: &str) -> PathBuf {
    let home = get_home_dir();

    if let Some(rest) = path.strip_prefix("~/") {
        PathBuf::from(home).join(rest)
    } else if path == "~" {
        PathBuf::from(home)
    } else {
        PathBuf::from(path)
    }
}

/// Convert an absolute path to a tilde representation if it's under the home directory
///
/// This function checks if the given path starts with the user's home directory.
/// If it does, the home directory portion is replaced with `~`. Otherwise,
/// the path is returned as a string unchanged.
///
/// # Arguments
/// * `path` - A path to potentially contract
///
/// # Returns
/// A string representation of the path with home directory contracted to `~` if applicable.
///
/// # Examples
/// ```
/// # use grunner::utils::contract_home;
/// # use std::path::Path;
/// # // With HOME = "/home/alice":
/// # // contract_home(Path::new("/home/alice/Documents")) → "~/Documents"
/// # // contract_home(Path::new("/etc/fstab")) → "/etc/fstab"
/// ```
#[must_use]
pub fn contract_home(path: &std::path::Path) -> String {
    let home = get_home_dir();
    let home_path = std::path::Path::new(&home);

    if let Ok(relative) = path.strip_prefix(home_path) {
        if relative.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", relative.display())
        }
    } else {
        path.display().to_string()
    }
}

/// Check if a line is a calculator result
///
/// A calculator result has the format "expression = result" where:
/// - expression contains only valid calculator characters (digits, operators, spaces, parentheses, letters)
/// - there's an equals sign in the middle
#[must_use]
pub fn is_calculator_result(line: &str) -> bool {
    if !line.contains('=') {
        return false;
    }

    let parts: Vec<&str> = line.split('=').collect();
    if parts.len() != 2 {
        return false;
    }

    let expr = parts[0].trim();
    let result = parts[1].trim();

    if expr.is_empty() {
        return false;
    }

    if !expr.chars().all(|c| {
        c.is_ascii_digit()
            || c == '.'
            || c == '+'
            || c == '-'
            || c == '*'
            || c == '/'
            || c == '%'
            || c == '^'
            || c == '('
            || c == ')'
            || c.is_whitespace()
            || c.is_ascii_alphabetic()
    }) {
        return false;
    }

    if !result.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }

    true
}

/// Get the icon for a file based on its content type
///
/// Uses GTK's content type detection to determine the appropriate icon
/// for displaying files in the UI.
///
/// # Arguments
/// * `file_path` - Path to the file
///
/// # Returns
/// A `gio::Icon` suitable for use with GTK image widgets
#[must_use]
pub fn get_file_icon(file_path: &str) -> gio::Icon {
    let (ctype, _) = gio::content_type_guess(Some(file_path), None::<&[u8]>);
    gio::content_type_get_icon(&ctype)
}
