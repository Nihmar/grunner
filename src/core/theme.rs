//! Theme management for Grunner
//!
//! This module provides theme loading and application functionality.
//! It supports system themes, built-in themes, and custom user themes.

use crate::core::config::ThemeMode;
use crate::utils::expand_home;
use gtk4::gdk;

pub mod themes;

#[derive(Clone)]
pub struct ThemeManager {
    provider: gtk4::CssProvider,
}

impl ThemeManager {
    #[must_use]
    pub fn new() -> Self {
        let provider = gtk4::CssProvider::new();
        Self { provider }
    }

    pub fn apply(&self, mode: ThemeMode, custom_path: Option<&str>, display: &gdk::Display) {
        let style_manager = libadwaita::StyleManager::default();

        let css_owned;
        let css: &str = match mode {
            ThemeMode::System => {
                log::info!("Using system theme (libadwaita defaults)");
                style_manager.set_color_scheme(libadwaita::ColorScheme::Default);
                return;
            }
            ThemeMode::SystemLight => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceLight);
                themes::LIGHT
            }
            ThemeMode::SystemDark => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceDark);
                themes::DARK
            }
            ThemeMode::TokioNight => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceDark);
                themes::TOKIO_NIGHT
            }
            ThemeMode::CatppuccinMocha => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceDark);
                themes::CATPPUCCIN_MOCHA
            }
            ThemeMode::CatppuccinLatte => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceLight);
                themes::CATPPUCCIN_LATTE
            }
            ThemeMode::Nord => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceDark);
                themes::NORD
            }
            ThemeMode::GruvboxDark => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceDark);
                themes::GRUVBOX_DARK
            }
            ThemeMode::GruvboxLight => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceLight);
                themes::GRUVBOX_LIGHT
            }
            ThemeMode::Dracula => {
                style_manager.set_color_scheme(libadwaita::ColorScheme::ForceDark);
                themes::DRACULA
            }
            ThemeMode::Custom => {
                css_owned = Self::load_custom_theme(custom_path);
                css_owned.as_deref().unwrap_or(themes::DARK)
            }
        };

        self.provider.load_from_data(css);
        log::info!("Loaded CSS provider with {} bytes", css.len());
        gtk4::style_context_remove_provider_for_display(display, &self.provider);
        gtk4::style_context_add_provider_for_display(
            display,
            &self.provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        log::info!("Applied theme: {mode:?}");
    }

    fn load_custom_theme(path: Option<&str>) -> Option<String> {
        if let Some(path) = path {
            let expanded = expand_home(path);
            match std::fs::read_to_string(&expanded) {
                Ok(css) => Some(css),
                Err(e) => {
                    log::error!(
                        "Failed to load custom theme from {}: {}",
                        expanded.display(),
                        e
                    );
                    None
                }
            }
        } else {
            log::warn!("Custom theme selected but no path provided");
            None
        }
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}
