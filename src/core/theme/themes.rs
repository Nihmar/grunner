//! Built-in theme definitions for Grunner
//!
//! Each theme is defined as a static string loaded from separate CSS files.

pub const LIGHT: &str = include_str!("light.css");
pub const DARK: &str = include_str!("dark.css");
pub const TOKIO_NIGHT: &str = include_str!("tokio_night.css");
pub const CATPPUCCIN_MOCHA: &str = include_str!("catppuccin_mocha.css");
pub const CATPPUCCIN_LATTE: &str = include_str!("catppuccin_latte.css");
pub const NORD: &str = include_str!("nord.css");
pub const GRUVBOX_DARK: &str = include_str!("gruvbox_dark.css");
pub const GRUVBOX_LIGHT: &str = include_str!("gruvbox_light.css");
pub const DRACULA: &str = include_str!("dracula.css");
