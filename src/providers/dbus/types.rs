//! D-Bus types for GNOME Shell search providers

/// Represents a GNOME Shell search provider
///
/// This struct contains the D-Bus addressing information and metadata
/// needed to communicate with a search provider. Providers are discovered
/// from .ini files in standard search provider directories.
#[derive(Debug, Clone)]
pub struct SearchProvider {
    pub bus_name: String,
    pub object_path: String,
    pub app_icon: String,
    pub desktop_id: String,
    pub default_disabled: bool,
}

/// Icon data carried by a search result
///
/// GNOME Shell search providers can send icons in two formats:
/// 1. Themed icon names that reference the current GTK icon theme
/// 2. File paths to image files (used for thumbnails, custom icons, etc.)
#[derive(Debug, Clone)]
pub enum IconData {
    Themed(String),
    File(String),
}

/// Individual search result from a provider
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<IconData>,
    pub app_icon: String,
    pub bus_name: String,
    pub object_path: String,
}
