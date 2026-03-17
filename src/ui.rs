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
use crate::global_state;
use crate::item_activation::activate_item;
use crate::launcher;
use crate::list_model::AppListModel;
use crate::obsidian_bar::build_obsidian_bar;
use crate::power_bar::build_power_bar;
use crate::workspace_bar::build_workspace_bar;
use glib::clone;

use gtk4::gdk;
use gtk4::gdk::Key;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, CssProvider, Entry, EventControllerKey, EventControllerMotion,
    Image, ListView, Orientation, Revealer, RevealerTransitionType, ScrolledWindow,
};
use libadwaita::prelude::AdwApplicationWindowExt;
use libadwaita::{Application, ApplicationWindow};
use log::{debug, error, info, trace};
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
            info!("Loaded {} applications", apps.len());
            model.set_apps(apps);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            // No data yet - reschedule polling on next idle
            trace!("Application loading still in progress");
            glib::idle_add_local_once(move || poll_apps(rx, model));
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            // Thread finished (shouldn't happen without sending data)
            error!("Application loading thread terminated unexpectedly");
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

/// Set up keyboard event controller for search entry navigation
///
/// This creates an EventControllerKey that handles keyboard navigation:
/// - Escape: close window
/// - Enter: activate selected item
/// - Arrow keys: move selection up/down
/// - Page Up/Down: jump 10 items
fn setup_keyboard_controller(
    entry: &Entry,
    list_view: &ListView,
    window: &ApplicationWindow,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
) {
    let key_ctrl = EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);

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
                Key::Escape => {
                    window.hide();
                    glib::Propagation::Stop
                }
                Key::Return | Key::KP_Enter => {
                    let timestamp = gdk::CURRENT_TIME;
                    let pos = model.selection.selected();
                    if let Some(obj) = model.store.item(pos) {
                        activate_item(&obj, &model, current_mode.get(), timestamp);
                    }
                    window.hide();
                    glib::Propagation::Stop
                }
                Key::Down | Key::KP_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    if pos + 1 < n {
                        scroll_selection_to(&model, &list_view, pos + 1);
                    }
                    glib::Propagation::Stop
                }
                Key::Up | Key::KP_Up => {
                    let pos = model.selection.selected();
                    if pos > 0 {
                        scroll_selection_to(&model, &list_view, pos - 1);
                    }
                    glib::Propagation::Stop
                }
                Key::Page_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    let next = (pos + 10).min(n.saturating_sub(1));
                    scroll_selection_to(&model, &list_view, next);
                    glib::Propagation::Stop
                }
                Key::Page_Up => {
                    let pos = model.selection.selected();
                    scroll_selection_to(&model, &list_view, pos.saturating_sub(10));
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    ));
    entry.add_controller(key_ctrl);
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
    debug!("Workspace bar enabled: {}", cfg.workspace_bar_enabled);
    info!("Workspace bar enabled: {}", cfg.workspace_bar_enabled);
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

    // Apply theme based on configuration
    let theme_manager = crate::theme::ThemeManager::new();
    theme_manager.apply(cfg.theme, cfg.custom_theme_path.as_deref(), &display);

    // Register theme reloader for hot-reload from settings
    let display_for_theme = display.clone();
    global_state::set_theme_reloader(move |config| {
        theme_manager.apply(
            config.theme,
            config.custom_theme_path.as_deref(),
            &display_for_theme,
        );
    });

    // -----------------------------------------------------------------------
    // 2. Data Model Initialization
    // -----------------------------------------------------------------------

    // Create the main data model that manages search results and state
    let model = AppListModel::new(
        cfg.max_results,
        cfg.obsidian.clone(),
        cfg.command_debounce_ms,
        cfg.search_provider_blacklist.clone(),
        cfg.commands.clone(),
        cfg.disable_modes,
    );

    // Register config reloader for hot-reload from settings
    let model_for_reloader = model.clone();
    global_state::set_config_reloader(move |config| {
        model_for_reloader.apply_config(config);
    });

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

    // Intercept close requests to hide window instead of destroying it
    window.connect_close_request(move |win| {
        win.hide();
        glib::Propagation::Stop
    });

    // -----------------------------------------------------------------------
    // 4. Main Layout Container
    // -----------------------------------------------------------------------

    // Create vertical box as root container for all UI elements
    let root = GtkBox::new(Orientation::Horizontal, 0);
    root.add_css_class("launcher-box");
    root.set_overflow(gtk4::Overflow::Hidden); // Prevent scrolling of entire window

    // Build workspace/window bar (shown between search entry and results when
    // there are open windows on the current workspace; hidden otherwise).
    // Also hidden when simple mode is enabled.
    let workspace_bar = if cfg.workspace_bar_enabled && !cfg.disable_modes {
        Some(build_workspace_bar(&window))
    } else {
        if cfg.disable_modes {
            info!("Workspace bar disabled (simple mode)");
        } else {
            info!("Workspace bar disabled via configuration");
        }
        None
    };
    if let Some(ref workspace_bar) = workspace_bar {
        // ── Sidebar hover wrapper ────────────────────────────────────
        // Un HBox che contiene edge trigger + revealer. L'EventControllerMotion
        // è attaccato a questo wrapper: enter → apre, leave → chiude.
        // In questo modo la sidebar resta aperta mentre il mouse è dentro.
        let sidebar_wrapper = GtkBox::new(Orientation::Horizontal, 0);
        root.append(&sidebar_wrapper);

        // ── Edge trigger ────────────────────────────────────────────
        let edge_trigger = GtkBox::new(Orientation::Vertical, 0);
        edge_trigger.add_css_class("edge-trigger");
        edge_trigger.set_can_focus(false);
        sidebar_wrapper.append(&edge_trigger);

        // ── Revealer con Overlay per il fade del bordo destro ────────
        // L'Overlay sovrappone un Box con gradiente CSS sopra la sidebar,
        // simulando la dissolvenza che mask-image multipla non può fare.
        let sidebar_revealer = Revealer::builder()
            .transition_type(RevealerTransitionType::SlideRight)
            .transition_duration(180)
            .reveal_child(false)
            .build();

        sidebar_revealer.set_child(Some(workspace_bar));
        sidebar_wrapper.append(&sidebar_revealer);

        // ── Hover: apre/chiude al passaggio del mouse ────────────────
        let motion = EventControllerMotion::new();
        motion.connect_enter(clone!(
            #[weak]
            sidebar_revealer,
            move |_, _, _| {
                sidebar_revealer.set_reveal_child(true);
            }
        ));
        motion.connect_leave(clone!(
            #[weak]
            sidebar_revealer,
            move |_| {
                sidebar_revealer.set_reveal_child(false);
            }
        ));
        sidebar_wrapper.add_controller(motion);
    }

    let content = GtkBox::new(Orientation::Vertical, 0);
    // content.add_css_class("launcher-box");
    content.set_overflow(gtk4::Overflow::Hidden);
    root.append(&content);

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

    content.append(&entry_box);

    // -----------------------------------------------------------------------
    // 6. Action Bars and Results List
    // -----------------------------------------------------------------------

    // Build Obsidian action bar (shown when in Obsidian mode)
    let obsidian_bar = build_obsidian_bar(&window, &entry, &model);

    // Get current icon theme for button icons
    let icon_theme = gtk4::IconTheme::for_display(&display);

    // Build power/settings action bar (always visible at bottom)
    // Only show power bar when special modes are enabled
    let power_bar = if cfg.disable_modes {
        None
    } else {
        Some(build_power_bar(&window, &entry, &icon_theme))
    };

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

    // Assemble all UI components in order:
    //   search entry → workspace bar → results → obsidian bar → power bar
    content.append(&scrolled);
    content.append(&obsidian_bar);
    if let Some(ref pb) = power_bar {
        entry_box.append(pb);
    }

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
            // Use idle_add to ensure focus is set after window is fully realized
            let entry_clone = entry.clone();
            glib::idle_add_local_once(move || {
                entry_clone.grab_focus();
            });
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
            let text = e.text().to_string().to_lowercase();
            let mode = AppMode::from_text(&text);
            current_mode.set(mode);

            // Update chrome immediately — these are cheap
            obsidian_bar.set_visible(mode.show_obsidian_bar());
            match mode.icon_name(obsidian_icon_name) {
                Some(name) => {
                    command_icon.set_icon_name(Some(name));
                    command_icon.set_visible(true);
                }
                None => command_icon.set_visible(false),
            }

            // Schedule the expensive store rebuild with debounce for default search
            model.schedule_populate(&text);
        }
    ));

    // -----------------------------------------------------------------------
    // 10. Keyboard Navigation and Shortcuts
    // -----------------------------------------------------------------------

    setup_keyboard_controller(&entry, &list_view, &window, &model, &current_mode);

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
            let timestamp = gdk::CURRENT_TIME;
            if let Some(obj) = model.store.item(pos) {
                activate_item(&obj, &model, current_mode.get(), timestamp);
            }
            window.hide();
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
