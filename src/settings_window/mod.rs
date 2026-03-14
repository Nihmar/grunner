//! Settings window module for Grunner
//!
//! This module provides a graphical user interface for editing Grunner's
//! configuration settings. It replaces the previous behavior of opening
//! the configuration file in a text editor.
//!
//! The settings window is organized into several categories:
//! - General: Window dimensions and basic behavior
//! - Search: Result limits and search behavior
//! - Obsidian: Integration with Obsidian vault (if configured)

pub(crate) mod save;
pub mod tabs;

use crate::config;
use crate::global_state;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{PreferencesDialog, Toast, ToastOverlay};
use log::{error, info};
use save::save_config;
use std::cell::RefCell;
use std::rc::Rc;

/// Open the settings window as a modal dialog
///
/// This function creates and displays a settings window that allows users
/// to modify Grunner's configuration through a graphical interface rather
/// than editing the TOML file directly.
///
/// # Arguments
/// * `parent` - The parent window to attach the settings dialog to
/// * `entry`  - The search entry to refocus when the dialog is dismissed
pub fn open_settings_window(parent: &libadwaita::ApplicationWindow, entry: &gtk4::Entry) {
    // Load current configuration
    let config = config::load();

    // Create the preferences dialog (replaces deprecated PreferencesWindow since adw 1.6)
    // Note: PreferencesDialog is an AdwDialog, not a GtkWindow — no default_width/height
    let window = PreferencesDialog::builder()
        .title("Grunner Settings")
        .build();

    // Create a toast overlay for notifications
    let overlay = ToastOverlay::new();
    // PreferencesDialog extends AdwDialog (not AdwWindow), so use set_child not set_content
    libadwaita::prelude::AdwDialogExt::set_child(&window, Some(&overlay));

    // Create a box to hold all pages
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.add_css_class("settings-content");
    // Enforce a sensible minimum height — PreferencesDialog has no default_height
    content.set_size_request(580, 560);
    overlay.set_child(Some(&content));

    // Refocus the search entry when the dialog is dismissed (Esc or Cancel/Save)
    window.connect_closed({
        let entry = entry.clone();
        move |_| {
            entry.grab_focus();
        }
    });

    // Store config in Rc for shared access in closures
    let config_rc = Rc::new(RefCell::new(config));

    // Notebook: one tab per settings category
    let notebook = gtk4::Notebook::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    notebook.add_css_class("pill-tabs");
    notebook.set_show_border(false);
    notebook.set_tab_pos(gtk4::PositionType::Top); // already default, but explicit
    notebook.set_halign(gtk4::Align::Fill);
    if let Some(header) = notebook.first_child() {
        header.set_halign(gtk4::Align::Center);
    }
    content.append(&notebook);

    // Build each tab
    tabs::info::build_tab(&notebook, &config_rc, &window, &overlay);
    tabs::general::build_tab(&notebook, &config_rc);
    tabs::search::build_tab(&notebook, &config_rc);
    tabs::commands::build_tab(&notebook, &config_rc);
    if config_rc.borrow().obsidian.is_some() {
        tabs::obsidian::build_tab(&notebook, &config_rc, parent);
    }

    // --- Save and Cancel Buttons ---
    let action_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    action_bar.add_css_class("settings-action-bar");
    action_bar.set_halign(gtk4::Align::End);

    let cancel_button = gtk4::Button::builder().label("Cancel").build();
    cancel_button.add_css_class("destructive-action");
    cancel_button.add_css_class("settings-action-button");
    cancel_button.connect_clicked({
        let window = window.downgrade();
        move |_| {
            if let Some(window) = window.upgrade() {
                libadwaita::prelude::AdwDialogExt::close(&window);
            }
        }
    });
    action_bar.append(&cancel_button);

    let save_button = gtk4::Button::builder().label("Save").build();
    save_button.add_css_class("suggested-action");
    save_button.add_css_class("settings-action-button");
    save_button.connect_clicked({
        let window = window.downgrade();
        let overlay = overlay.downgrade();
        let config_rc = Rc::clone(&config_rc);
        move |_| {
            if let Some(window) = window.upgrade()
                && let Some(overlay) = overlay.upgrade()
            {
                if let Err(e) = save_config(&config_rc.borrow()) {
                    error!("Failed to save configuration: {e}");
                    let toast = Toast::builder()
                        .title("Failed to save settings")
                        .timeout(3)
                        .build();
                    overlay.add_toast(toast);
                } else {
                    info!("Configuration saved successfully");
                    global_state::reload_config(&config_rc.borrow());
                    let toast = Toast::builder().title("Settings saved").timeout(2).build();
                    overlay.add_toast(toast);
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(1000),
                        move || {
                            libadwaita::prelude::AdwDialogExt::close(&window);
                        },
                    );
                }
            }
        }
    });
    action_bar.append(&save_button);

    content.append(&action_bar);

    // Present the dialog attached to the parent window
    window.present(Some(parent));
}
