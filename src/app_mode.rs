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
    /// Search provider mode triggered by `:s` prefix
    SearchProvider,
    /// Obsidian note search mode triggered by `:ob` prefix
    Obsidian,
    /// Obsidian grep search mode triggered by `:obg` prefix
    ObsidianGrep,
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
    /// - `:f` prefix → `FileSearch` (file system search)
    /// - `:s` prefix → `SearchProvider` (external search provider)
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
        } else if text.starts_with(":s") {
            Self::SearchProvider
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
    /// - `SearchProvider` → "system-search" (magnifying glass/search icon)
    /// - `Obsidian`/`ObsidianGrep` → Uses the provided `obsidian_icon`
    /// - `Normal` → `None` (no special icon)
    pub fn icon_name<'a>(&self, obsidian_icon: &'a str) -> Option<&'a str> {
        match self {
            Self::FileSearch => Some("text-x-generic"),
            Self::SearchProvider => Some("system-search"),
            Self::Obsidian | Self::ObsidianGrep => Some(obsidian_icon),
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
    pub fn show_obsidian_bar(&self) -> bool {
        matches!(self, Self::Obsidian | Self::ObsidianGrep)
    }
}
