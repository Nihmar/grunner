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
//! - Advanced: Debug and experimental features

use crate::actions::open_uri;
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

    // ── Tab 1: Info ──────────────────────────────────────────────────────────
    {
        let (scroll, inner) = make_tab_page();

        // Explanation section
        let explanation_group = PreferencesGroup::builder()
            .title("How Grunner Works")
            .description("Overview of features and usage")
            .build();

        let explanation_text = r#"Grunner is a GTK4 application launcher with advanced search capabilities and system integration.

## Default Search
Type any text to fuzzy-search all installed applications. Results are ranked by match score. The app's name, description, and icon are displayed in each row.

## Colon Commands
Type ':' followed by a command name and optional argument:

• :f <pattern> — Search your home directory for files
• :fg <pattern> — Search file contents recursively using ripgrep/grep
• :ob [text] — Obsidian actions (requires configuration)
• :obg <pattern> — Search Obsidian vault content

## Obsidian Action Bar
When you type `:ob` (with optional text), an action bar appears with four buttons:

• Open Vault — Opens your configured Obsidian vault
• New Note — Creates a timestamped note in the new notes folder
• Daily Note — Opens or creates today's daily note
• Quick Note — Appends text to the quick note file

Selecting any button performs the corresponding action.

## Power Bar Buttons
At the bottom of the main window:
• Settings (left) — Open this settings dialog
• Suspend — Put system to sleep (with confirmation)
• Restart — Reboot system (with confirmation)
• Power off — Shut down system (with confirmation)
• Log out — End current user session

## Search Providers
Grunner integrates with GNOME Shell search providers (Files, Calendar, Contacts, etc.) for unified searching."#;

        let explanation_view = gtk4::TextView::builder()
            .wrap_mode(gtk4::WrapMode::WordChar)
            .editable(false)
            .cursor_visible(false)
            .build();
        let explanation_buffer = explanation_view.buffer();
        explanation_buffer.set_text(explanation_text);
        let explanation_scrolled = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .min_content_height(120)
            .max_content_height(300)
            .build();
        explanation_scrolled.set_child(Some(&explanation_view));
        let explanation_row = PreferencesRow::new();
        explanation_row.set_child(Some(&explanation_scrolled));
        explanation_group.add(&explanation_row);
        inner.append(&explanation_group);

        let desktop_ids_group = PreferencesGroup::builder()
            .title("Desktop IDs for Blacklist")
            .description("Common GNOME desktop IDs you can copy and paste into the blacklist")
            .build();

        let desktop_ids = vec![
            "org.gnome.Contacts.desktop",
            "org.gnome.Calculator.desktop",
            "org.gnome.Characters.desktop",
            "org.gnome.Epiphany.desktop",
            "org.gnome.Weather.desktop",
            "org.gnome.Software.desktop",
            "org.gnome.Settings.desktop",
            "org.gnome.Calendar.desktop",
            "org.gnome.clocks.desktop",
            "org.gnome.Nautilus.desktop",
        ];
        let ids_text = gtk4::TextView::builder()
            .wrap_mode(gtk4::WrapMode::WordChar)
            .editable(false)
            .cursor_visible(false)
            .build();
        let ids_buffer = ids_text.buffer();
        ids_buffer.set_text(&desktop_ids.join("\n"));
        let ids_scrolled = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .min_content_height(120)
            .max_content_height(200)
            .build();
        ids_scrolled.set_child(Some(&ids_text));
        let ids_row = PreferencesRow::new();
        ids_row.set_child(Some(&ids_scrolled));
        desktop_ids_group.add(&ids_row);
        inner.append(&desktop_ids_group);

        // Configuration file section
        let config_group = PreferencesGroup::builder()
            .title("Configuration File")
            .description("Open the configuration file directly in your default editor")
            .build();

        let config_button = gtk4::Button::builder().label("Open Config File").build();
        config_button.connect_clicked({
            let window = window.downgrade();
            let overlay = overlay.downgrade();
            move |_| {
                let config_path = config::config_path();
                let uri = format!("file://{}", config_path.to_string_lossy());
                if let Err(e) = open_uri(&uri) {
                    error!("Failed to open config file: {}", e);
                    if let Some(overlay) = overlay.upgrade() {
                        let toast = Toast::builder()
                            .title("Failed to open config file")
                            .timeout(3)
                            .build();
                        overlay.add_toast(toast);
                    }
                } else {
                    info!("Opened config file: {}", uri);
                    // Close settings window after opening config
                    if let Some(window) = window.upgrade() {
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(500),
                            move || {
                                libadwaita::prelude::AdwDialogExt::close(&window);
                            },
                        );
                    }
                }
            }
        });
        config_group.add(&config_button);
        inner.append(&config_group);

        let reset_group = PreferencesGroup::builder()
            .title("Reset to Defaults")
            .description("Reset all settings to their default values")
            .build();

        let reset_button = gtk4::Button::builder().label("Reset All Settings").build();
        reset_button.add_css_class("destructive-action");
        reset_button.connect_clicked({
            let window = window.downgrade();
            let overlay = overlay.downgrade();
            let config_rc = Rc::clone(&config_rc);
            move |_| {
                let default_config = Config::default();
                config_rc.borrow_mut().window_width = default_config.window_width;
                config_rc.borrow_mut().window_height = default_config.window_height;
                config_rc.borrow_mut().max_results = default_config.max_results;
                config_rc.borrow_mut().command_debounce_ms = default_config.command_debounce_ms;
                config_rc.borrow_mut().app_dirs = default_config.app_dirs.clone();
                config_rc.borrow_mut().search_provider_blacklist =
                    default_config.search_provider_blacklist.clone();
                if let Some(obs) = default_config.obsidian {
                    config_rc.borrow_mut().obsidian = Some(obs);
                } else {
                    config_rc.borrow_mut().obsidian = None;
                }

                // Save the reset configuration
                if let Some(window) = window.upgrade() {
                    if let Some(overlay) = overlay.upgrade() {
                        if let Err(e) = save_config(&config_rc.borrow()) {
                            error!("Failed to save reset configuration: {}", e);
                            let toast = Toast::builder()
                                .title("Failed to save reset settings")
                                .timeout(3)
                                .build();
                            overlay.add_toast(toast);
                        } else {
                            info!("Configuration reset and saved successfully");
                            let toast = Toast::builder()
                                .title("Settings reset to defaults")
                                .timeout(2)
                                .build();
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
        reset_group.add(&reset_button);
        inner.append(&reset_group);

        notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Info"))));
    }

    // ── Tab 2: General ───────────────────────────────────────────────────────
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

    // ── Tab 3: Search ────────────────────────────────────────────────────────
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
