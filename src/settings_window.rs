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
//! - Commands: Custom colon commands
//! - Advanced: Debug and experimental features

use crate::config::{self, Config, ObsidianConfig};

use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{
    EntryRow, PreferencesDialog, PreferencesGroup, PreferencesRow, SpinRow, Toast, ToastOverlay,
};
use log::{debug, error, info};
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

    // Helper: create a scrolled tab page containing a vertical box of groups.
    // Each tab page is a ScrolledWindow so long content stays accessible.
    let make_tab_page = || -> (gtk4::ScrolledWindow, gtk4::Box) {
        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();
        let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        inner.set_margin_top(16);
        inner.set_margin_bottom(16);
        inner.set_margin_start(16);
        inner.set_margin_end(16);
        scroll.set_child(Some(&inner));
        (scroll, inner)
    };

    // Notebook replaces the flat vertical stack — one tab per settings category.
    let notebook = gtk4::Notebook::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    notebook.add_css_class("pill-tabs");
    notebook.set_show_border(false);
    notebook.set_tab_pos(gtk4::PositionType::Top); // already default, but explicit
    // Center the tab strip within the header
    notebook.set_halign(gtk4::Align::Fill); // notebook fills full width
    // The tabs widget inside is what needs centering:
    if let Some(header) = notebook.first_child() {
        header.set_halign(gtk4::Align::Center);
    }
    content.append(&notebook);

    // ── Tab 1: General ───────────────────────────────────────────────────────
    {
        let (scroll, inner) = make_tab_page();

        let window_group = PreferencesGroup::builder()
            .title("Window")
            .description("Configure the launcher window appearance")
            .build();

        let width_row = SpinRow::builder()
            .title("Window Width")
            .subtitle("Width of the launcher window in pixels")
            .build();
        width_row.set_range(400.0, 1920.0);
        width_row.adjustment().set_step_increment(10.0);
        width_row.adjustment().set_page_increment(50.0);
        width_row.set_value(config_rc.borrow().window_width as f64);
        width_row.connect_notify_local(Some("value"), {
            let config_rc = Rc::clone(&config_rc);
            move |row, _| {
                config_rc.borrow_mut().window_width = row.value() as i32;
            }
        });
        window_group.add(&width_row);

        let height_row = SpinRow::builder()
            .title("Window Height")
            .subtitle("Height of the launcher window in pixels")
            .build();
        height_row.set_range(300.0, 1080.0);
        height_row.adjustment().set_step_increment(10.0);
        height_row.adjustment().set_page_increment(50.0);
        height_row.set_value(config_rc.borrow().window_height as f64);
        height_row.connect_notify_local(Some("value"), {
            let config_rc = Rc::clone(&config_rc);
            move |row, _| {
                config_rc.borrow_mut().window_height = row.value() as i32;
            }
        });
        window_group.add(&height_row);
        inner.append(&window_group);

        notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("General"))));
    }

    // ── Tab 2: Search ────────────────────────────────────────────────────────
    {
        let (scroll, inner) = make_tab_page();

        let behavior_group = PreferencesGroup::builder()
            .title("Behavior")
            .description("Configure how search results are displayed")
            .build();

        let max_results_row = SpinRow::builder()
            .title("Maximum Results")
            .subtitle("Maximum number of search results to display")
            .build();
        max_results_row.set_range(10.0, 200.0);
        max_results_row.adjustment().set_step_increment(1.0);
        max_results_row.adjustment().set_page_increment(10.0);
        max_results_row.set_value(config_rc.borrow().max_results as f64);
        max_results_row.connect_notify_local(Some("value"), {
            let config_rc = Rc::clone(&config_rc);
            move |row, _| {
                config_rc.borrow_mut().max_results = row.value() as usize;
            }
        });
        behavior_group.add(&max_results_row);

        let debounce_row = SpinRow::builder()
            .title("Command Debounce")
            .subtitle("Delay before executing colon commands (milliseconds)")
            .build();
        debounce_row.set_range(100.0, 2000.0);
        debounce_row.adjustment().set_step_increment(50.0);
        debounce_row.adjustment().set_page_increment(100.0);
        debounce_row.set_value(config_rc.borrow().command_debounce_ms as f64);
        debounce_row.connect_notify_local(Some("value"), {
            let config_rc = Rc::clone(&config_rc);
            move |row, _| {
                config_rc.borrow_mut().command_debounce_ms = row.value() as u32;
            }
        });
        behavior_group.add(&debounce_row);
        inner.append(&behavior_group);

        // Application directories (multi-line text area)
        let dirs_group = PreferencesGroup::builder()
            .title("Application Directories")
            .description("Paths scanned for .desktop files (one per line)")
            .build();

        let dirs_text = gtk4::TextView::builder()
            .wrap_mode(gtk4::WrapMode::WordChar)
            .build();
        let dirs_buffer = dirs_text.buffer();
        dirs_buffer.set_text(
            &config_rc
                .borrow()
                .app_dirs
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        dirs_buffer.connect_changed({
            let config_rc = Rc::clone(&config_rc);
            move |buffer| {
                let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                config_rc.borrow_mut().app_dirs = text
                    .split('\n')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .map(|s| crate::utils::expand_home(&s))
                    .collect();
            }
        });
        let dirs_scrolled = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .min_content_height(100)
            .max_content_height(200)
            .build();
        dirs_scrolled.set_child(Some(&dirs_text));
        let dirs_row = PreferencesRow::new();
        dirs_row.set_child(Some(&dirs_scrolled));
        dirs_group.add(&dirs_row);
        inner.append(&dirs_group);

        // Search provider blacklist
        let blacklist_group = PreferencesGroup::builder()
            .title("Search Provider Blacklist")
            .description("List of GNOME Shell search providers to exclude (one per line)")
            .build();

        let blacklist_text = gtk4::TextView::builder()
            .wrap_mode(gtk4::WrapMode::WordChar)
            .build();
        let blacklist_buffer = blacklist_text.buffer();
        blacklist_buffer.set_text(&config_rc.borrow().search_provider_blacklist.join("\n"));
        blacklist_buffer.connect_changed({
            let config_rc = Rc::clone(&config_rc);
            move |buffer| {
                let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                config_rc.borrow_mut().search_provider_blacklist = text
                    .split('\n')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        });
        let blacklist_scrolled = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .min_content_height(60)
            .max_content_height(120)
            .build();
        blacklist_scrolled.set_child(Some(&blacklist_text));
        let blacklist_row = PreferencesRow::new();
        blacklist_row.set_child(Some(&blacklist_scrolled));
        blacklist_group.add(&blacklist_row);
        inner.append(&blacklist_group);

        notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Search"))));
    }

    // ── Tab 3: Commands ──────────────────────────────────────────────────────
    {
        let (scroll, inner) = make_tab_page();

        let commands_group = PreferencesGroup::builder()
            .title("Custom Colon Commands")
            .description("One per line, format: command=shell command. Use $1 for the argument.")
            .build();

        let commands_text = gtk4::TextView::builder()
            .wrap_mode(gtk4::WrapMode::WordChar)
            .build();
        let commands_buffer = commands_text.buffer();
        commands_buffer.set_text(
            &config_rc
                .borrow()
                .commands
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        commands_buffer.connect_changed({
            let config_rc = Rc::clone(&config_rc);
            move |buffer| {
                let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                let mut new_commands = std::collections::HashMap::new();
                for line in text.split('\n') {
                    let trimmed = line.trim();
                    if let Some(eq_pos) = trimmed.find('=') {
                        let key = trimmed[..eq_pos].trim().to_string();
                        let value = trimmed[eq_pos + 1..].trim().to_string();
                        if !key.is_empty() && !value.is_empty() {
                            new_commands.insert(key, value);
                        }
                    }
                }
                config_rc.borrow_mut().commands = new_commands;
            }
        });
        let commands_scrolled = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .min_content_height(150)
            .max_content_height(300)
            .build();
        commands_scrolled.set_child(Some(&commands_text));
        let commands_row = PreferencesRow::new();
        commands_row.set_child(Some(&commands_scrolled));
        commands_group.add(&commands_row);
        inner.append(&commands_group);

        notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Commands"))));
    }

    // ── Tab 4: Obsidian (only when vault is configured) ──────────────────────
    if config_rc.borrow().obsidian.is_some() {
        let (scroll, inner) = make_tab_page();

        let obsidian_group = PreferencesGroup::builder()
            .title("Vault Configuration")
            .description("Configure Obsidian vault integration")
            .build();

        let vault_row = EntryRow::builder().title("Vault Path").build();
        vault_row.set_text(&config_rc.borrow().obsidian.as_ref().unwrap().vault);
        vault_row.connect_changed({
            let config_rc = Rc::clone(&config_rc);
            move |row| {
                if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                    obs.vault = row.text().to_string();
                }
            }
        });
        obsidian_group.add(&vault_row);

        let daily_row = EntryRow::builder().title("Daily Notes Folder").build();
        daily_row.set_text(
            &config_rc
                .borrow()
                .obsidian
                .as_ref()
                .unwrap()
                .daily_notes_folder,
        );
        daily_row.connect_changed({
            let config_rc = Rc::clone(&config_rc);
            move |row| {
                if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                    obs.daily_notes_folder = row.text().to_string();
                }
            }
        });
        obsidian_group.add(&daily_row);

        let new_row = EntryRow::builder().title("New Notes Folder").build();
        new_row.set_text(
            &config_rc
                .borrow()
                .obsidian
                .as_ref()
                .unwrap()
                .new_notes_folder,
        );
        new_row.connect_changed({
            let config_rc = Rc::clone(&config_rc);
            move |row| {
                if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                    obs.new_notes_folder = row.text().to_string();
                }
            }
        });
        obsidian_group.add(&new_row);

        let quick_row = EntryRow::builder().title("Quick Note File").build();
        quick_row.set_text(&config_rc.borrow().obsidian.as_ref().unwrap().quick_note);
        quick_row.connect_changed({
            let config_rc = Rc::clone(&config_rc);
            move |row| {
                if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                    obs.quick_note = row.text().to_string();
                }
            }
        });
        obsidian_group.add(&quick_row);

        inner.append(&obsidian_group);
        notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Obsidian"))));
    }

    // --- Save and Cancel Buttons ---
    let action_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    action_bar.set_margin_top(12);
    action_bar.set_margin_bottom(12);
    action_bar.set_margin_start(12);
    action_bar.set_margin_end(12);
    action_bar.set_halign(gtk4::Align::End);

    // Cancel button
    let cancel_button = gtk4::Button::builder().label("Cancel").build();
    cancel_button.add_css_class("destructive-action");
    cancel_button.connect_clicked({
        let window = window.downgrade();
        move |_| {
            if let Some(window) = window.upgrade() {
                libadwaita::prelude::AdwDialogExt::close(&window);
            }
        }
    });
    action_bar.append(&cancel_button);

    // Save button
    let save_button = gtk4::Button::builder().label("Save").build();
    save_button.add_css_class("suggested-action");
    save_button.connect_clicked({
        let window = window.downgrade();
        let overlay = overlay.downgrade();
        let config_rc = Rc::clone(&config_rc);
        move |_| {
            if let Some(window) = window.upgrade() {
                if let Some(overlay) = overlay.upgrade() {
                    if let Err(e) = save_config(&config_rc.borrow()) {
                        error!("Failed to save configuration: {}", e);
                        let toast = Toast::builder()
                            .title("Failed to save settings")
                            .timeout(3)
                            .build();
                        overlay.add_toast(toast);
                    } else {
                        info!("Configuration saved successfully");
                        let toast = Toast::builder().title("Settings saved").timeout(2).build();
                        overlay.add_toast(toast);

                        // Close window after a short delay
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1000),
                            move || {
                                libadwaita::prelude::AdwDialogExt::close(&window);
                            },
                        );
                    }
                }
            }
        }
    });
    action_bar.append(&save_button);

    // Add action bar to the bottom
    content.append(&action_bar);

    // Present the dialog attached to the parent window
    window.present(Some(parent));
}

/// Save configuration to file
///
/// # Arguments
/// * `config` - The configuration to save
///
/// # Returns
/// `Result<(), std::io::Error>` indicating success or failure
fn save_config(config: &Config) -> Result<(), std::io::Error> {
    use serde::Serialize;
    use std::fs;

    // Create TOML representation
    #[derive(Serialize)]
    struct TomlConfig {
        window: WindowConfig,
        search: SearchConfig,
        commands: Option<std::collections::HashMap<String, String>>,
        obsidian: Option<ObsidianConfig>,
    }

    #[derive(Serialize)]
    struct WindowConfig {
        width: i32,
        height: i32,
    }

    #[derive(Serialize)]
    struct SearchConfig {
        max_results: usize,
        app_dirs: Vec<String>,
        command_debounce_ms: u32,
        provider_blacklist: Vec<String>,
    }

    // Convert app_dirs back to strings (without home expansion)
    let app_dirs: Vec<String> = config
        .app_dirs
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let toml_config = TomlConfig {
        window: WindowConfig {
            width: config.window_width,
            height: config.window_height,
        },
        search: SearchConfig {
            max_results: config.max_results,
            app_dirs,
            command_debounce_ms: config.command_debounce_ms,
            provider_blacklist: config.search_provider_blacklist.clone(),
        },
        commands: if config.commands.is_empty() {
            None
        } else {
            Some(config.commands.clone())
        },
        obsidian: config.obsidian.clone(),
    };

    let toml_string = toml::to_string_pretty(&toml_config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let path = config::config_path();
    debug!("Saving configuration to {:?}", path);

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, toml_string)?;
    Ok(())
}
