//! Utility functions for Grunner
//!
//! This module provides general-purpose helper functions used throughout
//! the application. Currently, it contains path manipulation utilities
//! for handling user home directory expansion, calculator result parsing,
//! and icon selection.

pub mod clipboard;
pub mod desktop;

use crate::calculator::is_valid_calc_char;
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

    if !expr.chars().all(is_valid_calc_char) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── expand_home tests ─────────────────────────────────────────────

    #[test]
    fn test_expand_home_tilde_slash() {
        let home = get_home_dir();
        let result = expand_home("~/Documents");
        assert_eq!(result, PathBuf::from(home).join("Documents"));
    }

    #[test]
    fn test_expand_home_bare_tilde() {
        let home = get_home_dir();
        let result = expand_home("~");
        assert_eq!(result, PathBuf::from(home));
    }

    #[test]
    fn test_expand_home_absolute_path() {
        let result = expand_home("/etc/fstab");
        assert_eq!(result, PathBuf::from("/etc/fstab"));
    }

    #[test]
    fn test_expand_home_relative_path() {
        let result = expand_home("some/relative/path");
        assert_eq!(result, PathBuf::from("some/relative/path"));
    }

    #[test]
    fn test_expand_home_empty_string() {
        let result = expand_home("");
        assert_eq!(result, PathBuf::from(""));
    }

    #[test]
    fn test_expand_home_tilde_slash_empty() {
        let home = get_home_dir();
        let result = expand_home("~/");
        assert_eq!(result, PathBuf::from(home));
    }

    // ── contract_home tests ───────────────────────────────────────────

    #[test]
    fn test_contract_home_under_home() {
        let home = get_home_dir();
        let path = Path::new(home).join("Documents");
        let result = contract_home(&path);
        assert_eq!(result, "~/Documents");
    }

    #[test]
    fn test_contract_home_exact_home() {
        let home = get_home_dir();
        let path = Path::new(home);
        let result = contract_home(path);
        assert_eq!(result, "~");
    }

    #[test]
    fn test_contract_home_outside_home() {
        let result = contract_home(Path::new("/etc/fstab"));
        assert_eq!(result, "/etc/fstab");
    }

    #[test]
    fn test_contract_home_root() {
        let result = contract_home(Path::new("/"));
        assert_eq!(result, "/");
    }

    #[test]
    fn test_expand_contract_roundtrip() {
        let original = "~/Documents/test.txt";
        let expanded = expand_home(original);
        let contracted = contract_home(&expanded);
        assert_eq!(contracted, original);
    }

    #[test]
    fn test_contract_home_nested() {
        let home = get_home_dir();
        let path = Path::new(home).join("a/b/c/d.txt");
        let result = contract_home(&path);
        assert_eq!(result, "~/a/b/c/d.txt");
    }

    // ── is_calculator_result tests ────────────────────────────────────

    #[test]
    fn test_is_calculator_result_valid() {
        assert!(is_calculator_result("2 + 2 = 4"));
    }

    #[test]
    fn test_is_calculator_result_no_equals() {
        assert!(!is_calculator_result("2 + 2"));
    }

    #[test]
    fn test_is_calculator_result_multiple_equals() {
        assert!(!is_calculator_result("a = b = c"));
    }

    #[test]
    fn test_is_calculator_result_empty_expr() {
        assert!(!is_calculator_result(" = 5"));
    }

    #[test]
    fn test_is_calculator_result_no_digits_in_result() {
        assert!(!is_calculator_result("2 + 2 = abc"));
    }

    #[test]
    fn test_is_calculator_result_float() {
        assert!(is_calculator_result("10 / 3 = 3.33"));
    }

    #[test]
    fn test_is_calculator_result_with_functions() {
        assert!(is_calculator_result("sin(0) = 0"));
    }

    #[test]
    fn test_is_calculator_result_negative() {
        assert!(is_calculator_result("-5 + 3 = -2"));
    }
}
