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
use crate::core::callbacks::AppCallbacks;
use crate::core::config::Config;
use crate::item_activation::activate_item;
use crate::launcher;
use crate::model::list_model::AppListModel;
use crate::ui::context_menu::{WindowCtx, setup_list_context_menu};
use crate::ui::obsidian_bar::build_obsidian_bar;
use crate::ui::pinned_strip::{
    build_pinned_strip, launch_pinned_by_index, update_pinned_strip, update_strip_visibility,
};
use crate::ui::power_bar::build_power_bar;
use crate::ui::workspace_bar::build_workspace_bar;
use glib::clone;

use gtk4::gdk;
use gtk4::gdk::Key;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, CssProvider, Entry, EventControllerKey, EventControllerMotion,
    GestureClick, Image, ListView, Orientation, Revealer, RevealerTransitionType, ScrolledWindow,
};
use libadwaita::prelude::AdwApplicationWindowExt;
use libadwaita::{Application, ApplicationWindow, ToastOverlay};
use log::{debug, error, info, trace};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Helper functions for background processing
// ---------------------------------------------------------------------------

/// Context for app loading polling — groups all state needed when apps are ready
struct AppLoadingContext {
    rx: Rc<std::sync::mpsc::Receiver<Vec<launcher::DesktopApp>>>,
    model: AppListModel,
    all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pinned_strip: GtkBox,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    window: ApplicationWindow,
    dragging: Rc<Cell<bool>>,
    cfg: Config,
}

impl Clone for AppLoadingContext {
    fn clone(&self) -> Self {
        Self {
            rx: Rc::clone(&self.rx),
            model: self.model.clone(),
            all_apps: Rc::clone(&self.all_apps),
            pinned_strip: self.pinned_strip.clone(),
            pinned_apps: Rc::clone(&self.pinned_apps),
            window: self.window.clone(),
            dragging: Rc::clone(&self.dragging),
            cfg: self.cfg.clone(),
        }
    }
}

impl AppLoadingContext {
    fn poll(&self) {
        match self.rx.try_recv() {
            Ok(apps) => {
                info!("Loaded {} applications", apps.len());
                (*self.all_apps.borrow_mut()).clone_from(&apps);

                let pinned = self.pinned_apps.borrow();
                update_pinned_strip(
                    &self.pinned_strip,
                    &pinned,
                    &apps,
                    &self.window,
                    &self.pinned_apps,
                    &self.all_apps,
                    &self.dragging,
                    &self.cfg,
                );
                update_strip_visibility(&self.pinned_strip, &pinned, true);

                self.model.set_apps(apps);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                trace!("Application loading still in progress");
                let ctx = self.clone();
                glib::idle_add_local_once(move || ctx.poll());
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                error!("Application loading thread terminated unexpectedly");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UI Construction Helpers
// ---------------------------------------------------------------------------

/// Initialize the data model
fn setup_model(cfg: &Config) -> AppListModel {
    AppListModel::new(
        cfg.max_results,
        cfg.obsidian.clone(),
        cfg.command_debounce_ms,
        cfg.search_provider_blacklist.clone(),
        cfg.commands.clone(),
        cfg.disable_modes,
    )
}

/// Create the main application window
fn create_window(app: &Application, cfg: &Config) -> ApplicationWindow {
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

    window
}

/// Build the sidebar containing workspace bar (optional)
fn build_sidebar(window: &ApplicationWindow, cfg: &Config) -> Option<GtkBox> {
    if !cfg.workspace_bar_enabled || cfg.disable_modes {
        if cfg.disable_modes {
            info!("Workspace bar disabled (simple mode)");
        } else {
            info!("Workspace bar disabled via configuration");
        }
        return None;
    }

    let workspace_bar = build_workspace_bar(window);

    // ── Sidebar hover wrapper ────────────────────────────────────
    // An HBox containing edge trigger + revealer. The EventControllerMotion
    // is attached to this wrapper: enter → opens, leave → closes.
    // This keeps the sidebar open while the mouse is inside.
    let sidebar_wrapper = GtkBox::new(Orientation::Horizontal, 0);

    // ── Edge trigger ────────────────────────────────────────────
    let edge_trigger = GtkBox::new(Orientation::Vertical, 0);
    edge_trigger.add_css_class("edge-trigger");
    edge_trigger.set_can_focus(false);
    sidebar_wrapper.append(&edge_trigger);

    // ── Revealer ────────────────────────────────────────────────
    let sidebar_revealer = Revealer::builder()
        .transition_type(RevealerTransitionType::SlideRight)
        .transition_duration(180)
        .reveal_child(false)
        .build();

    sidebar_revealer.set_child(Some(&workspace_bar));
    sidebar_wrapper.append(&sidebar_revealer);

    // ── Hover: opens/closes on mouse enter/leave ────────────────
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

    Some(sidebar_wrapper)
}

/// Build the right sidebar containing pinned apps (favourites)
fn build_right_sidebar(
    pinned_strip: &GtkBox,
    pinned_apps: &Rc<RefCell<Vec<String>>>,
    dragging: &Rc<Cell<bool>>,
) -> GtkBox {
    // ── Sidebar hover wrapper ────────────────────────────────────
    // HBox that contains revealer + edge trigger. The EventControllerMotion
    // is attached to this wrapper: enter → opens, leave → closes.
    let sidebar_wrapper = GtkBox::new(Orientation::Horizontal, 0);

    // ── Revealer that slides the pinned bar in from the right ────
    let sidebar_revealer = Revealer::builder()
        .transition_type(RevealerTransitionType::SlideLeft)
        .transition_duration(180)
        .reveal_child(false)
        .build();

    sidebar_revealer.set_child(Some(pinned_strip));
    sidebar_wrapper.append(&sidebar_revealer);

    // ── Edge trigger on the right edge ───────────────────────────
    let edge_trigger = GtkBox::new(Orientation::Vertical, 0);
    edge_trigger.add_css_class("edge-trigger");
    edge_trigger.set_can_focus(false);
    sidebar_wrapper.append(&edge_trigger);

    // ── Hover: opens/closes on mouse enter/leave ─────────────────
    let motion = EventControllerMotion::new();
    let p_apps_enter = pinned_apps.clone();
    motion.connect_enter(clone!(
        #[weak]
        sidebar_revealer,
        move |_, _, _| {
            if !p_apps_enter.borrow().is_empty() {
                sidebar_revealer.set_reveal_child(true);
            }
        }
    ));
    let dragging_leave = dragging.clone();
    motion.connect_leave(clone!(
        #[weak]
        sidebar_revealer,
        move |_| {
            if !dragging_leave.get() {
                sidebar_revealer.set_reveal_child(false);
            }
        }
    ));
    sidebar_wrapper.add_controller(motion);

    sidebar_wrapper
}

/// Build the main layout: search entry, pinned strip, results list, and action bars
fn build_main_layout(
    window: &ApplicationWindow,
    entry: &Entry,
    model: &AppListModel,
    cfg: &Config,
    callbacks: &AppCallbacks,
    pinned_apps: &Rc<RefCell<Vec<String>>>,
    dragging: &Rc<Cell<bool>>,
) -> (
    GtkBox,
    ListView,
    Option<GtkBox>,
    Image,
    GtkBox,
    ToastOverlay,
) {
    // Create vertical box as root container for all UI elements
    let root = GtkBox::new(Orientation::Horizontal, 0);
    root.add_css_class("launcher-box");
    root.set_overflow(gtk4::Overflow::Hidden); // Prevent scrolling of entire window

    // Build sidebar if enabled
    if let Some(sidebar) = build_sidebar(window, cfg) {
        root.append(&sidebar);
    }

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_overflow(gtk4::Overflow::Hidden);
    root.append(&content);

    // --- Search Entry Area ---
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

    entry_box.append(entry);
    content.append(&entry_box);

    // --- Pinned Apps Strip (built as right sidebar) ---
    let pinned_strip = build_pinned_strip();

    // --- Action Bars and Results List ---
    // Build Obsidian action bar (shown when in Obsidian mode)
    let obsidian_bar = build_obsidian_bar(window, entry, model);

    // Get current icon theme for button icons
    let display = gtk4::prelude::WidgetExt::display(window);
    let icon_theme = gtk4::IconTheme::for_display(&display);

    // Build power/settings action bar (always visible at bottom)
    // Only show power bar when special modes are enabled
    let power_bar = if cfg.disable_modes {
        None
    } else {
        Some(build_power_bar(window, entry, &icon_theme, callbacks))
    };

    // Create list view factory for rendering result items
    let active_mode = model.active_mode();
    let vault_path = model.config.obsidian_cfg.as_ref().map(|cfg| {
        crate::utils::expand_home(&cfg.vault)
            .to_string_lossy()
            .into_owned()
    });
    let factory = crate::ui::list_factory::create_factory(active_mode, vault_path);
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
    //   search entry → results → obsidian bar → power bar
    content.append(&scrolled);
    content.append(&obsidian_bar);
    if let Some(ref pb) = power_bar {
        entry_box.append(pb);
    }

    // Build right sidebar for pinned apps
    let right_sidebar = build_right_sidebar(&pinned_strip, pinned_apps, dragging);
    root.append(&right_sidebar);

    // Set root container as window content, wrapped in toast overlay
    let toast_overlay = ToastOverlay::new();
    toast_overlay.set_child(Some(&root));
    window.set_content(Some(&toast_overlay));

    (
        root,
        list_view,
        Some(obsidian_bar),
        command_icon,
        pinned_strip,
        toast_overlay,
    )
}

/// Connect window lifecycle signals
fn connect_window_signals(
    window: &ApplicationWindow,
    entry: &Entry,
    obsidian_bar: &GtkBox,
    command_icon: &Image,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
) {
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
}

/// Pinned apps UI state
struct PinnedUiState {
    strip: GtkBox,
    apps: Rc<RefCell<Vec<String>>>,
}

/// Internal context for UI construction phases
///
/// Groups all shared state needed during UI building so that helper
/// functions receive a single struct instead of many individual arguments.
#[derive(Clone)]
struct WindowContext {
    display: gdk::Display,
    cfg: Config,
    model: AppListModel,
    current_mode: Rc<Cell<AppMode>>,
    window: ApplicationWindow,
    callbacks: AppCallbacks,
    entry: Entry,
    list_view: ListView,
    obsidian_bar: Option<GtkBox>,
    command_icon: Image,
    pinned_strip: GtkBox,
    toast_overlay: ToastOverlay,
    all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    dragging: Rc<Cell<bool>>,
    theme_manager: crate::core::theme::ThemeManager,
}

impl WindowContext {
    fn ctx(&self) -> WindowCtx {
        WindowCtx {
            window: self.window.clone(),
            entry: self.entry.clone(),
            model: self.model.clone(),
            current_mode: self.current_mode.clone(),
            pinned_apps: self.pinned_apps.clone(),
            all_apps: self.all_apps.clone(),
            pinned_strip: self.pinned_strip.clone(),
            toast_overlay: self.toast_overlay.clone(),
            dragging: self.dragging.clone(),
            cfg: self.cfg.clone(),
        }
    }

    fn setup_theme(&self) {
        self.theme_manager.apply(
            self.cfg.theme,
            self.cfg.custom_theme_path.as_deref(),
            &self.display,
        );
    }

    fn wire_callbacks(&self) {
        let model = self.model.clone();
        self.callbacks.connect_config_changed(move |_| {
            let config = crate::core::config::load();
            model.apply_config(&config);
        });

        let display = self.display.clone();
        let theme_manager = self.theme_manager.clone();
        self.callbacks.connect_theme_changed(move |_| {
            let config = crate::core::config::load();
            theme_manager.apply(config.theme, config.custom_theme_path.as_deref(), &display);
        });

        let window = self.window.clone();
        self.callbacks.connect_window_resized(move |_| {
            let config = crate::core::config::load();
            window.set_resizable(true);
            window.set_default_size(config.window_width, config.window_height);
            window.set_resizable(false);
        });
    }

    fn setup_dragging(&self, root: &GtkBox) {
        let click = GestureClick::new();
        click.set_button(1);
        click.set_propagation_phase(gtk4::PropagationPhase::Target);
        let window = self.window.clone();
        click.connect_pressed(move |gesture, _n_press, x, y| {
            let Some(surface) = window.surface() else {
                return;
            };
            let Some(toplevel) = surface.downcast_ref::<gdk::Toplevel>() else {
                return;
            };
            let Some(device) = gesture.device() else {
                return;
            };
            let button = gesture.current_button().cast_signed();
            toplevel.begin_move(&device, button, x, y, gdk::CURRENT_TIME);
        });
        root.add_controller(click);
    }

    fn wire_signals(&self) {
        if let Some(ref obsidian_bar) = self.obsidian_bar {
            connect_window_signals(
                &self.window,
                &self.entry,
                obsidian_bar,
                &self.command_icon,
                &self.model,
                &self.current_mode,
            );
        }

        let icon_theme = gtk4::IconTheme::for_display(&self.display);
        let obsidian_icon_name = ["obsidian", "md.obsidian.Obsidian", "text-x-markdown"]
            .iter()
            .find(|&&name| icon_theme.has_icon(name))
            .copied()
            .unwrap_or("text-x-markdown");

        let pinned_ui = PinnedUiState {
            strip: self.pinned_strip.clone(),
            apps: self.pinned_apps.clone(),
        };
        if let Some(ref obsidian_bar) = self.obsidian_bar {
            connect_search_signals(
                &self.entry,
                &self.model,
                &self.current_mode,
                obsidian_bar,
                &self.command_icon,
                obsidian_icon_name,
                &pinned_ui,
            );
        }

        setup_keyboard_controller(
            &self.list_view,
            &self.window,
            &self.model,
            &self.current_mode,
            &self.pinned_apps,
            &self.all_apps,
        );
        connect_list_signals(
            &self.list_view,
            &self.window,
            &self.model,
            &self.current_mode,
        );
        setup_list_context_menu(&self.list_view, &self.ctx());
    }

    fn start_loading(&self) {
        let dirs = self.cfg.expanded_app_dirs();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(launcher::load_apps(&dirs));
        });
        let load_ctx = AppLoadingContext {
            rx: Rc::new(rx),
            model: self.model.clone(),
            all_apps: self.all_apps.clone(),
            pinned_strip: self.pinned_strip.clone(),
            pinned_apps: self.pinned_apps.clone(),
            window: self.window.clone(),
            dragging: self.dragging.clone(),
            cfg: self.cfg.clone(),
        };
        glib::idle_add_local_once(move || load_ctx.poll());
    }
}

/// Connect search entry signals (text changes, icon updates)
fn connect_search_signals(
    entry: &Entry,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
    obsidian_bar: &GtkBox,
    command_icon: &Image,
    obsidian_icon_name: &str,
    pinned: &PinnedUiState,
) {
    // Handle text changes in search entry (main search functionality)
    let pinned_strip = pinned.strip.clone();
    let pinned_apps_clone = pinned.apps.clone();
    let obsidian_icon_name = obsidian_icon_name.to_string();
    entry.connect_changed(clone!(
        #[strong]
        model,
        #[strong]
        current_mode,
        #[weak]
        obsidian_bar,
        #[weak]
        command_icon,
        #[strong]
        obsidian_icon_name,
        #[strong]
        pinned_apps_clone,
        move |e| {
            let text = e.text().to_string().to_lowercase();
            let mode = AppMode::from_text(&text);
            current_mode.set(mode);

            // Update chrome immediately — these are cheap
            obsidian_bar.set_visible(mode.show_obsidian_bar());
            match mode.icon_name(&obsidian_icon_name) {
                Some(name) => {
                    command_icon.set_icon_name(Some(name));
                    command_icon.set_visible(true);
                }
                None => command_icon.set_visible(false),
            }

            // Update pinned strip visibility (hide when typing)
            let pinned = pinned_apps_clone.borrow();
            update_strip_visibility(&pinned_strip, &pinned, text.is_empty());

            // Schedule the expensive store rebuild with debounce for default search
            model.schedule_populate(&text);
        }
    ));
}

/// Connect list view activation signals (mouse double-click)
fn connect_list_signals(
    list_view: &ListView,
    window: &ApplicationWindow,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
) {
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
}

/// Scroll the list view to ensure a selected item is visible
///
/// This function updates the selection model and triggers GTK's
/// built-in scrolling action to bring the selected item into view.
/// It's used for keyboard navigation (arrow keys, page up/down).
///
/// # Arguments
/// * `model` - The application list model containing selection state
/// * `list_view` - The GTK `ListView` widget to scroll
/// * `pos` - Position (index) of the item to select and scroll to
fn scroll_selection_to(model: &AppListModel, list_view: &ListView, pos: u32) {
    // Update selection model
    model.selection.set_selected(pos);
    // Trigger GTK's scroll-to-item action
    let _ = list_view.activate_action("list.scroll-to-item", Some(&pos.to_variant()));
}

/// Set up keyboard event controller for search entry navigation
///
/// This creates an `EventControllerKey` that handles keyboard navigation:
/// - Escape: close window
/// - Enter: activate selected item
/// - Arrow keys: move selection up/down
/// - Page Up/Down: jump 10 items
/// - Alt+1..Alt+9: launch N-th pinned app
fn setup_keyboard_controller(
    list_view: &ListView,
    window: &ApplicationWindow,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
    pinned_apps: &Rc<RefCell<Vec<String>>>,
    all_apps: &Rc<RefCell<Vec<launcher::DesktopApp>>>,
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
        #[strong]
        pinned_apps,
        #[strong]
        all_apps,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, modifier_state| {
            // Alt+1..Alt+9: launch pinned app
            if modifier_state.contains(gdk::ModifierType::ALT_MASK) {
                let index = match key {
                    Key::_1 => Some(0),
                    Key::_2 => Some(1),
                    Key::_3 => Some(2),
                    Key::_4 => Some(3),
                    Key::_5 => Some(4),
                    Key::_6 => Some(5),
                    Key::_7 => Some(6),
                    Key::_8 => Some(7),
                    Key::_9 => Some(8),
                    _ => None,
                };
                if let Some(idx) = index {
                    let pinned = pinned_apps.borrow();
                    let apps = all_apps.borrow();
                    launch_pinned_by_index(idx, &pinned, &apps, &window);
                    return glib::Propagation::Stop;
                }
            }

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
    window.add_controller(key_ctrl);
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
///
/// # Panics
/// Panics if the default GDK display cannot be obtained.
pub fn build_ui(app: &Application, cfg: &Config) {
    debug!("Workspace bar enabled: {}", cfg.workspace_bar_enabled);

    let display = gtk4::gdk::Display::default().expect("Cannot connect to display");
    let model = setup_model(cfg);
    let current_mode = Rc::new(Cell::new(AppMode::Normal));
    let all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>> = Rc::new(RefCell::new(Vec::new()));
    let pinned_apps = Rc::new(RefCell::new(cfg.pinned_apps.clone()));
    let dragging = Rc::new(Cell::new(false));
    let window = create_window(app, cfg);
    let callbacks = AppCallbacks::new();

    let provider = CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let entry = Entry::builder()
        .placeholder_text("Search applications…")
        .hexpand(true)
        .build();
    entry.add_css_class("search-entry");

    let (root, list_view, obsidian_bar, command_icon, pinned_strip, toast_overlay) =
        build_main_layout(
            &window,
            &entry,
            &model,
            cfg,
            &callbacks,
            &pinned_apps,
            &dragging,
        );

    let wctx = WindowContext {
        display: display.clone(),
        cfg: cfg.clone(),
        model: model.clone(),
        current_mode: current_mode.clone(),
        window: window.clone(),
        callbacks: callbacks.clone(),
        entry: entry.clone(),
        list_view: list_view.clone(),
        obsidian_bar: obsidian_bar.clone(),
        command_icon: command_icon.clone(),
        pinned_strip: pinned_strip.clone(),
        toast_overlay: toast_overlay.clone(),
        all_apps: all_apps.clone(),
        pinned_apps: pinned_apps.clone(),
        dragging: dragging.clone(),
        theme_manager: crate::core::theme::ThemeManager::new(),
    };

    wctx.setup_theme();
    wctx.wire_callbacks();
    wctx.setup_dragging(&root);
    window.present();
    wctx.wire_signals();
    wctx.start_loading();
}
