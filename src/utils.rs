//! Utility functions for Grunner
//!
//! This module provides general-purpose helper functions used throughout
//! the application. Currently, it contains path manipulation utilities
//! for handling user home directory expansion.

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
/// // With HOME = "/home/alice":
/// // expand_home("~/Documents") → PathBuf::from("/home/alice/Documents")
/// // expand_home("~") → PathBuf::from("/home/alice")
/// // expand_home("/etc/fstab") → PathBuf::from("/etc/fstab") (unchanged)
/// ```
///
/// # Environment
/// Relies on the `HOME` environment variable. If `HOME` is not set,
/// defaults to an empty string, which may result in unexpected paths.
pub fn expand_home(path: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();

    if let Some(rest) = path.strip_prefix("~/") {
        // Path like "~/Documents" - join home directory with rest of path
        PathBuf::from(home).join(rest)
    } else if path == "~" {
        // Just "~" - return home directory itself
        PathBuf::from(home)
    } else {
        // Path doesn't start with "~" - return unchanged
        PathBuf::from(path)
    }
}
