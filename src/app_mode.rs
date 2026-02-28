#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    FileSearch,
    SearchProvider,
    Obsidian,
    ObsidianGrep,
}

impl AppMode {
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

    pub fn icon_name<'a>(&self, obsidian_icon: &'a str) -> Option<&'a str> {
        match self {
            Self::FileSearch => Some("text-x-generic"),
            Self::SearchProvider => Some("system-search"),
            Self::Obsidian | Self::ObsidianGrep => Some(obsidian_icon),
            Self::Normal => None,
        }
    }

    pub fn show_obsidian_bar(&self) -> bool {
        matches!(self, Self::Obsidian | Self::ObsidianGrep)
    }
}
