//! Application mode management for Grunner
//!
//! This module defines the `AppMode` enum which represents the different
//! operational modes of the application. Each mode corresponds to a different
//! type of search or functionality that Grunner can perform.

/// Enum representing the different modes of the Grunner application
///
/// Modes are triggered by prefix commands in the search input and determine:
/// - What type of search to perform
/// - What data sources to query
/// - What UI elements to show
/// - What actions are available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Default mode - searches desktop applications and commands
    Normal,
    /// File search mode triggered by `:f` prefix
    FileSearch,

    /// Obsidian note search mode triggered by `:ob` prefix
    Obsidian,
    /// Obsidian grep search mode triggered by `:obg` prefix
    ObsidianGrep,
    /// Custom script mode triggered by `:sh` prefix
    CustomScript,
}

impl AppMode {
    /// Determine the application mode based on input text
    ///
    /// # Arguments
    /// * `text` - The user's input text (typically from the search entry)
    ///
    /// # Returns
    /// The appropriate `AppMode` based on the prefix in the text.
    ///
    /// # Mode Triggers
    /// - `:obg` prefix → `ObsidianGrep` (grep search within Obsidian notes)
    /// - `:ob` prefix → `Obsidian` (simple Obsidian note search)
    /// - `:f` or `:fg` prefix → `FileSearch` (file system search or content grep)
    /// - `:sh` prefix → `CustomScript` (run custom scripts/commands)
    /// - No prefix or unrecognized prefix → `Normal` (default application search)
    ///
    /// Note: Order matters - `:obg` must be checked before `:ob` since both start with `:ob`
    pub fn from_text(text: &str) -> Self {
        if text.starts_with(":obg") {
            Self::ObsidianGrep
        } else if text.starts_with(":ob") {
            Self::Obsidian
        } else if text.starts_with(":f") {
            Self::FileSearch
        } else if text.starts_with(":sh") {
            Self::CustomScript
        } else {
            Self::Normal
        }
    }

    /// Get the icon name for the current mode
    ///
    /// # Arguments
    /// * `obsidian_icon` - The icon name to use for Obsidian-related modes
    ///
    /// # Returns
    /// `Some(&str)` with the appropriate icon name for the mode,
    /// or `None` for the Normal mode (which uses no special icon).
    ///
    /// # Icon Mappings
    /// - `FileSearch` → "text-x-generic" (generic text file icon)
    /// - `Obsidian`/`ObsidianGrep` → Uses the provided `obsidian_icon`
    /// - `CustomScript` → "utilities-terminal" (terminal icon)
    /// - `Normal` → `None` (no special icon)
    pub fn icon_name(self, obsidian_icon: &str) -> Option<&str> {
        match self {
            Self::FileSearch => Some("text-x-generic"),
            Self::Obsidian | Self::ObsidianGrep => Some(obsidian_icon),
            Self::CustomScript => Some("utilities-terminal"),
            Self::Normal => None,
        }
    }

    /// Check if the Obsidian action bar should be shown in this mode
    ///
    /// # Returns
    /// `true` if the mode is Obsidian-related (either `Obsidian` or `ObsidianGrep`),
    /// `false` otherwise.
    ///
    /// This is used by the UI to determine whether to show the special Obsidian
    /// action bar with buttons for vault actions, new notes, etc.
    pub fn show_obsidian_bar(self) -> bool {
        matches!(self, Self::Obsidian | Self::ObsidianGrep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_mode_from_text() {
        assert_eq!(AppMode::from_text(":sh"), AppMode::CustomScript);
        assert_eq!(AppMode::from_text(":sh "), AppMode::CustomScript);
        assert_eq!(AppMode::from_text(":sh ls"), AppMode::CustomScript);
        assert_eq!(AppMode::from_text(":ob"), AppMode::Obsidian);
        assert_eq!(AppMode::from_text(":obg"), AppMode::ObsidianGrep);
        assert_eq!(AppMode::from_text(":f"), AppMode::FileSearch);
        assert_eq!(AppMode::from_text(":fg"), AppMode::FileSearch);
        assert_eq!(AppMode::from_text(""), AppMode::Normal);
        assert_eq!(AppMode::from_text("hello"), AppMode::Normal);
    }

    #[test]
    fn test_app_mode_icon_name() {
        let obsidian_icon = "obsidian-icon";
        assert_eq!(
            AppMode::CustomScript.icon_name(obsidian_icon),
            Some("utilities-terminal")
        );
        assert_eq!(
            AppMode::FileSearch.icon_name(obsidian_icon),
            Some("text-x-generic")
        );
        assert_eq!(
            AppMode::Obsidian.icon_name(obsidian_icon),
            Some(obsidian_icon)
        );
        assert_eq!(AppMode::Normal.icon_name(obsidian_icon), None);
    }

    #[test]
    fn test_app_mode_show_obsidian_bar() {
        assert!(AppMode::Obsidian.show_obsidian_bar());
        assert!(AppMode::ObsidianGrep.show_obsidian_bar());
        assert!(!AppMode::CustomScript.show_obsidian_bar());
        assert!(!AppMode::FileSearch.show_obsidian_bar());
        assert!(!AppMode::Normal.show_obsidian_bar());
    }
}
