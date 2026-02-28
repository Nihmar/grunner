/// Represents which typing mode the launcher is currently in, derived entirely
/// from the entry text prefix. This is the single source of truth for mode —
/// no scattered `starts_with` checks elsewhere in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    FileSearch,     // :f
    SearchProvider, // :s
    Obsidian,       // :ob  (action mode — shows the obsidian button bar)
    ObsidianGrep,   // :obg (grep mode — searches vault content)
}

impl AppMode {
    /// Derives the current mode from the (already lowercased) entry text.
    /// Order matters: `:obg` must be checked before `:ob`.
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

    /// Returns the icon name to display in the command icon widget, or `None`
    /// when no icon should be shown (Normal mode).
    pub fn icon_name<'a>(&self, obsidian_icon: &'a str) -> Option<&'a str> {
        match self {
            Self::FileSearch => Some("text-x-generic"),
            Self::SearchProvider => Some("system-search"),
            Self::Obsidian | Self::ObsidianGrep => Some(obsidian_icon),
            Self::Normal => None,
        }
    }

    /// Whether the Obsidian quick-action button bar should be visible.
    pub fn show_obsidian_bar(&self) -> bool {
        matches!(self, Self::Obsidian | Self::ObsidianGrep)
    }
}
