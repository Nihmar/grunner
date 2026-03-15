//! Theme management for Grunner
//!
//! This module provides theme loading and application functionality.
//! It supports system themes, built-in themes, and custom user themes.

use crate::config::ThemeMode;
use crate::utils::expand_home;
use gtk4::gdk;

pub mod themes;

pub struct ThemeManager {
    provider: gtk4::CssProvider,
}

impl ThemeManager {
    pub fn new() -> Self {
        let provider = gtk4::CssProvider::new();
        Self { provider }
    }

    pub fn apply(&self, mode: ThemeMode, custom_path: Option<&str>, display: &gdk::Display) {
        let css = match mode {
            ThemeMode::System => themes::DARK,
            ThemeMode::SystemLight => themes::LIGHT,
            ThemeMode::SystemDark => themes::DARK,
            ThemeMode::TokioNight => themes::TOKIO_NIGHT,
            ThemeMode::CatppuccinMocha => themes::CATPPUCCIN_MOCHA,
            ThemeMode::CatppuccinLatte => themes::CATPPUCCIN_LATTE,
            ThemeMode::Nord => themes::NORD,
            ThemeMode::GruvboxDark => themes::GRUVBOX_DARK,
            ThemeMode::GruvboxLight => themes::GRUVBOX_LIGHT,
            ThemeMode::Dracula => themes::DRACULA,
            ThemeMode::Custom => self.load_custom_theme(custom_path),
        };

        self.provider.load_from_data(css);
        log::info!("Loaded CSS provider with {} bytes", css.len());
        gtk4::style_context_add_provider_for_display(
            display,
            &self.provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        log::info!("Applied theme: {:?}", mode);
    }

    fn load_custom_theme(&self, path: Option<&str>) -> &'static str {
        if let Some(path) = path {
            let expanded = expand_home(path);
            match std::fs::read_to_string(&expanded) {
                Ok(css) => Box::leak(css.into_boxed_str()),
                Err(e) => {
                    log::error!(
                        "Failed to load custom theme from {}: {}",
                        expanded.display(),
                        e
                    );
                    themes::DARK
                }
            }
        } else {
            log::warn!("Custom theme selected but no path provided, using dark theme");
            themes::DARK
        }
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}
