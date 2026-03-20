//! Grunner library crate
//!
//! This crate provides the core functionality for the Grunner application launcher.
//! It includes configuration management, search providers, and other utilities.

pub mod actions;
pub mod app_mode;
pub mod calculator;
pub mod command_handler;
pub mod core {
    pub mod callbacks;
    pub mod config;
    pub mod global_state;
    pub mod theme;
}
pub mod item_activation;
pub mod launcher;
pub mod model {
    pub mod items;
    pub mod list_model;
}
pub mod providers;
pub mod settings_window;
pub mod ui {
    pub mod context_menu;
    pub mod list_factory;
    pub mod obsidian_bar;
    pub mod pinned_strip;
    pub mod power_bar;
    pub mod window;
    pub mod workspace_bar;
}
pub mod utils;
