//! Main UI construction module for Grunner
//!
//! This module is responsible for building the complete GTK user interface for
//! Grunner, including window setup, search entry, results list, action bars,
//! and all event handling. It serves as the central coordination point between
//! the data model (`AppListModel`) and the GTK widgets.
//!
//! Key responsibilities:
//! - Window creation and styling with CSS
//! - Search entry with real-time query processing
//! - Results list view with custom item rendering
//! - Obsidian and power action bars
//! - Keyboard navigation and selection handling
//! - Application lifecycle and focus management
//! - Background application loading with threading

use crate::app_mode::AppMode;
use crate::config::Config;
use crate::item_activation::activate_item;
use crate::launcher;
use crate::list_model::AppListModel;
use crate::obsidian_bar::build_obsidian_bar;
use crate::power_bar::build_power_bar;
use glib::clone;
use gtk4::gdk::Key;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, CssProvider, Entry, EventControllerKey, Image, ListView, Orientation,
    ScrolledWindow,
};
use libadwaita::prelude::AdwApplicationWindowExt;
use libadwaita::{Application, ApplicationWindow};
use std::cell::Cell;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Helper functions for background processing
// ---------------------------------------------------------------------------

/// Poll for application loading results from background thread
///
/// This function checks a channel for the results of desktop application
/// scanning and updates the list model when apps are ready. It uses
/// GLib's idle callbacks to avoid blocking the UI thread.
///
/// # Arguments
/// * `rx` - Channel receiver for desktop application vector
/// * `model` - The AppListModel to update with loaded applications
fn poll_apps(rx: std::sync::mpsc::Receiver<Vec<launcher::DesktopApp>>, model: AppListModel) {
    match rx.try_recv() {
        Ok(apps) => {
            // Apps loaded successfully - update the model
            model.set_apps(apps);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            // No data yet - reschedule polling on next idle
            glib::idle_add_local_once(move || poll_apps(rx, model));
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            // Thread finished (shouldn't happen without sending data)
        }
    }
}

/// Scroll the list view to ensure a selected item is visible
///
/// This function updates the selection model and triggers GTK's
/// built-in scrolling action to bring the selected item into view.
/// It's used for keyboard navigation (arrow keys, page up/down).
///
/// # Arguments
/// * `model` - The application list model containing selection state
/// * `list_view` - The GTK ListView widget to scroll
/// * `pos` - Position (index) of the item to select and scroll to
fn scroll_selection_to(model: &AppListModel, list_view: &ListView, pos: u32) {
    // Update selection model
    model.selection.set_selected(pos);
    // Trigger GTK's scroll-to-item action
    let _ = list_view.activate_action("list.scroll-to-item", Some(&pos.to_variant()));
}

// ---------------------------------------------------------------------------
// Main UI construction function
// ---------------------------------------------------------------------------

/// Build and display the complete Grunner user interface
///
/// This is the main entry point for UI construction. It creates all
/// GTK widgets, sets up event handlers, loads CSS styling, and
/// initializes the application state. The function is called when
/// the GTK application is activated.
///
/// # Arguments
/// * `app` - The GTK Application instance
/// * `cfg` - Application configuration loaded from file or defaults
pub fn build_ui(app: &Application, cfg: &Config) {
    // -----------------------------------------------------------------------
    // 1. Display and CSS Setup
    // -----------------------------------------------------------------------

    // Get default display connection (required for CSS theming)
    let display = gtk4::gdk::Display::default().expect("Cannot connect to display");

    // Load and apply CSS stylesheet for custom widget styling
    let provider = CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // -----------------------------------------------------------------------
    // 2. Data Model Initialization
    // -----------------------------------------------------------------------

    // Create the main data model that manages search results and state
    let model = AppListModel::new(
        cfg.max_results,
        cfg.commands.clone(),
        cfg.obsidian.clone(),
        cfg.command_debounce_ms,
        cfg.search_provider_blacklist.clone(),
    );

    // Track current application mode for UI rendering and action handling
    let current_mode: Rc<Cell<AppMode>> = Rc::new(Cell::new(AppMode::Normal));

    // -----------------------------------------------------------------------
    // 3. Window Creation and Configuration
    // -----------------------------------------------------------------------

    // Create the main application window with minimal chrome
    let window = ApplicationWindow::builder()
        .application(app)
        .title("grunner")
        .default_width(cfg.window_width)
        .default_height(cfg.window_height)
        .decorated(false) // No window decorations (title bar, borders)
        .resizable(false) // Fixed size launcher window
        .build();

    // Apply custom CSS class for window styling
    window.set_css_classes(&["launcher-window"]);
    // Remove default background class on realize for clean appearance
    window.connect_realize(|w| {
        w.remove_css_class("background");
    });

    // -----------------------------------------------------------------------
    // 4. Main Layout Container
    // -----------------------------------------------------------------------

    // Create vertical box as root container for all UI elements
    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("launcher-box");
    root.set_overflow(gtk4::Overflow::Hidden); // Prevent scrolling of entire window

    // -----------------------------------------------------------------------
    // 5. Search Entry Area
    // -----------------------------------------------------------------------

    // Horizontal container for search icon and entry field
    let entry_box = GtkBox::new(Orientation::Horizontal, 6);
    entry_box.set_hexpand(true);
    entry_box.set_margin_start(12);
    entry_box.set_margin_end(12);
    entry_box.set_margin_top(12);
    entry_box.set_margin_bottom(0);

    // Mode indicator icon (shows for special modes like :ob, :f, etc.)
    let command_icon = Image::new();
    command_icon.set_pixel_size(24);
    command_icon.set_valign(Align::Center);
    command_icon.set_visible(false); // Hidden by default, shown for special modes
    entry_box.append(&command_icon);

    // Main search entry field
    let entry = Entry::builder()
        .placeholder_text("Search applications…")
        .hexpand(true)
        .build();
    entry.add_css_class("search-entry");
    entry_box.append(&entry);

    root.append(&entry_box);

    // -----------------------------------------------------------------------
    // 6. Action Bars and Results List
    // -----------------------------------------------------------------------

    // Build Obsidian action bar (shown when in Obsidian mode)
    let obsidian_bar = build_obsidian_bar(&window, &entry, &model);

    // Get current icon theme for button icons
    let icon_theme = gtk4::IconTheme::for_display(&display);

    // Build power/settings action bar (always visible at bottom)
    let power_bar = build_power_bar(&window, &entry, &icon_theme);

    // Create list view factory for rendering result items
    let factory = model.create_factory();
    // Create list view with selection model and custom factory
    let list_view = ListView::new(Some(model.selection.clone()), Some(factory));
    list_view.set_single_click_activate(false); // Require double-click/Enter to activate
    list_view.add_css_class("app-list");
    list_view.set_can_focus(false); // Keep focus on search entry

    // Wrap list view in scrolled window for vertical scrolling
    let scrolled = ScrolledWindow::builder()
        .vexpand(true)
        .child(&list_view)
        .build();

    // Assemble all UI components in order
    root.append(&scrolled);
    root.append(&obsidian_bar);
    root.append(&power_bar);

    // Set root container as window content
    window.set_content(Some(&root));

    // Display the window
    window.present();

    // -----------------------------------------------------------------------
    // 7. Window Lifecycle and Initial State
    // -----------------------------------------------------------------------

    // Reset UI state each time window is shown
    window.connect_show(clone!(
        #[weak]
        entry,
        #[weak]
        obsidian_bar,
        #[weak]
        command_icon,
        #[strong]
        model,
        #[strong]
        current_mode,
        move |_| {
            // Clear search text and results
            entry.set_text("");
            model.populate("");
            current_mode.set(AppMode::Normal);

            // Hide special UI elements
            obsidian_bar.set_visible(false);
            command_icon.set_visible(false);

            // Focus search entry for immediate typing
            entry.grab_focus();
        }
    ));

    // -----------------------------------------------------------------------
    // 8. Icon Theme Configuration
    // -----------------------------------------------------------------------

    // Determine Obsidian icon name based on available icons in theme
    let obsidian_icon_name = ["obsidian", "md.obsidian.Obsidian", "text-x-markdown"]
        .iter()
        .find(|&&name| icon_theme.has_icon(name))
        .copied()
        .unwrap_or("text-x-markdown");

    // -----------------------------------------------------------------------
    // 9. Search Entry Event Handlers
    // -----------------------------------------------------------------------

    // Handle text changes in search entry (main search functionality)
    entry.connect_changed(clone!(
        #[strong]
        model,
        #[strong]
        current_mode,
        #[weak]
        obsidian_bar,
        #[weak]
        command_icon,
        move |e| {
            let text = e.text().to_lowercase();
            let mode = AppMode::from_text(&text);
            current_mode.set(mode);

            // Update search results based on current text
            model.populate(&text);

            // Show/hide Obsidian action bar based on mode
            obsidian_bar.set_visible(mode.show_obsidian_bar());

            // Update mode indicator icon
            match mode.icon_name(obsidian_icon_name) {
                Some(name) => {
                    command_icon.set_icon_name(Some(name));
                    command_icon.set_visible(true);
                }
                None => command_icon.set_visible(false),
            }

            // Force UI redraw to reflect changes
            e.queue_draw();
            e.queue_resize();
        }
    ));

    // -----------------------------------------------------------------------
    // 10. Keyboard Navigation and Shortcuts
    // -----------------------------------------------------------------------

    // Set up keyboard event controller for search entry
    let key_ctrl = EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture); // Intercept before default handlers

    key_ctrl.connect_key_pressed(clone!(
        #[weak]
        list_view,
        #[weak]
        window,
        #[strong]
        model,
        #[strong]
        current_mode,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            match key {
                // Escape: close window
                Key::Escape => {
                    window.close();
                    glib::Propagation::Stop
                }
                // Enter: activate selected item
                Key::Return | Key::KP_Enter => {
                    let pos = model.selection.selected();
                    if let Some(obj) = model.store.item(pos) {
                        activate_item(&obj, &model, current_mode.get());
                    }
                    window.close();
                    glib::Propagation::Stop
                }
                // Down arrow: move selection down
                Key::Down | Key::KP_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    if pos + 1 < n {
                        scroll_selection_to(&model, &list_view, pos + 1);
                    }
                    glib::Propagation::Stop
                }
                // Up arrow: move selection up
                Key::Up | Key::KP_Up => {
                    let pos = model.selection.selected();
                    if pos > 0 {
                        scroll_selection_to(&model, &list_view, pos - 1);
                    }
                    glib::Propagation::Stop
                }
                // Page down: jump 10 items down
                Key::Page_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    let next = (pos + 10).min(n.saturating_sub(1));
                    scroll_selection_to(&model, &list_view, next);
                    glib::Propagation::Stop
                }
                // Page up: jump 10 items up
                Key::Page_Up => {
                    let pos = model.selection.selected();
                    scroll_selection_to(&model, &list_view, pos.saturating_sub(10));
                    glib::Propagation::Stop
                }
                // Other keys: allow default processing
                _ => glib::Propagation::Proceed,
            }
        }
    ));
    entry.add_controller(key_ctrl);

    // -----------------------------------------------------------------------
    // 11. List View Activation (Mouse Double-Click)
    // -----------------------------------------------------------------------

    // Handle item activation via mouse double-click
    list_view.connect_activate(clone!(
        #[weak]
        window,
        #[strong]
        model,
        #[strong]
        current_mode,
        move |_, pos| {
            if let Some(obj) = model.store.item(pos) {
                activate_item(&obj, &model, current_mode.get());
            }
            window.close();
        }
    ));

    // -----------------------------------------------------------------------
    // 12. Background Application Loading
    // -----------------------------------------------------------------------

    // Load desktop applications in background thread to avoid UI freeze
    let dirs = cfg.app_dirs.clone();
    let model_poll = model.clone();
    let (tx, rx) = std::sync::mpsc::channel();

    // Spawn background thread for application scanning
    std::thread::spawn(move || {
        let _ = tx.send(launcher::load_apps(&dirs));
    });

    // Start polling for application loading results
    glib::idle_add_local_once(move || poll_apps(rx, model_poll));
}
