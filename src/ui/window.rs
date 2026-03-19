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
use crate::core::config::Config;
use crate::core::global_state;
use crate::item_activation::activate_item;
use crate::launcher;
use crate::model::items::{AppItem, CommandItem};
use crate::model::list_model::AppListModel;
use crate::ui::obsidian_bar::build_obsidian_bar;
use crate::ui::pinned_strip::{
    build_pinned_strip, launch_pinned_by_index, update_pinned_strip, update_strip_visibility,
};
use crate::ui::power_bar::build_power_bar;
use crate::ui::workspace_bar::build_workspace_bar;
use glib::clone;

use gtk4::gdk;
use gtk4::gdk::Key;
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CssProvider, Entry, EventControllerKey, EventControllerMotion,
    GestureClick, Image, ListView, Orientation, Popover, Revealer, RevealerTransitionType,
    ScrolledWindow,
};
use libadwaita::prelude::AdwApplicationWindowExt;
use libadwaita::{Application, ApplicationWindow, Toast, ToastOverlay};
use log::{debug, error, info, trace};
use std::cell::{Cell, RefCell};
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
/// * `all_apps` - Shared reference to loaded apps for pinned strip
/// * `pinned_strip` - The pinned strip container to update
/// * `pinned_separator` - The pinned separator to update
/// * `pinned_apps` - Current pinned apps list
/// * `window` - Application window for pinned strip context menus
#[allow(clippy::too_many_arguments)]
fn poll_apps(
    rx: std::sync::mpsc::Receiver<Vec<launcher::DesktopApp>>,
    model: AppListModel,
    all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pinned_strip: GtkBox,
    pinned_separator: GtkBox,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    window: ApplicationWindow,
    entry: Entry,
) {
    match rx.try_recv() {
        Ok(apps) => {
            // Apps loaded successfully - update the model
            info!("Loaded {} applications", apps.len());

            // Store loaded apps for pinned strip lookup
            *all_apps.borrow_mut() = apps.clone();

            // Update pinned strip with loaded apps
            let pinned = pinned_apps.borrow();
            update_pinned_strip(
                &pinned_strip,
                &pinned,
                &apps,
                &window,
                &pinned_apps,
                &all_apps,
                &pinned_strip,
                &pinned_separator,
                &entry,
            );
            update_strip_visibility(&pinned_strip, &pinned_separator, &pinned, true);

            model.set_apps(apps);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            // No data yet - reschedule polling on next idle
            trace!("Application loading still in progress");
            glib::idle_add_local_once(move || {
                poll_apps(
                    rx,
                    model,
                    all_apps,
                    pinned_strip,
                    pinned_separator,
                    pinned_apps,
                    window,
                    entry,
                )
            });
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            // Thread finished (shouldn't happen without sending data)
            error!("Application loading thread terminated unexpectedly");
        }
    }
}

// ---------------------------------------------------------------------------
// UI Construction Helpers
// ---------------------------------------------------------------------------

/// Setup CSS provider and apply theme based on configuration
fn setup_css(cfg: &Config, display: &gdk::Display) {
    // Load and apply CSS stylesheet for custom widget styling
    let provider = CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Apply theme based on configuration
    let theme_manager = crate::core::theme::ThemeManager::new();
    theme_manager.apply(cfg.theme, cfg.custom_theme_path.as_deref(), display);

    // Register theme reloader for hot-reload from settings
    let display_for_theme = display.clone();
    global_state::set_theme_reloader(move |config| {
        theme_manager.apply(
            config.theme,
            config.custom_theme_path.as_deref(),
            &display_for_theme,
        );
    });
}

/// Initialize the data model and register config reloader
fn setup_model(cfg: &Config) -> AppListModel {
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

    model
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
    // Un HBox che contiene edge trigger + revealer. L'EventControllerMotion
    // è attaccato a questo wrapper: enter → apre, leave → chiude.
    // In questo modo la sidebar resta aperta mentre il mouse è dentro.
    let sidebar_wrapper = GtkBox::new(Orientation::Horizontal, 0);

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

    sidebar_revealer.set_child(Some(&workspace_bar));
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

    Some(sidebar_wrapper)
}

/// Build the main layout: search entry, pinned strip, results list, and action bars
fn build_main_layout(
    window: &ApplicationWindow,
    entry: &Entry,
    model: &AppListModel,
    cfg: &Config,
    display: &gdk::Display,
) -> (
    GtkBox,
    ListView,
    Option<GtkBox>,
    Image,
    GtkBox,
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

    // --- Pinned Apps Strip ---
    let (pinned_strip, pinned_separator) = build_pinned_strip();
    content.append(&pinned_strip);
    content.append(&pinned_separator);

    // --- Action Bars and Results List ---
    // Build Obsidian action bar (shown when in Obsidian mode)
    let obsidian_bar = build_obsidian_bar(window, entry, model);

    // Get current icon theme for button icons
    let icon_theme = gtk4::IconTheme::for_display(display);

    // Build power/settings action bar (always visible at bottom)
    // Only show power bar when special modes are enabled
    let power_bar = if cfg.disable_modes {
        None
    } else {
        Some(build_power_bar(window, entry, &icon_theme))
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
        pinned_separator,
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
    separator: GtkBox,
    apps: Rc<RefCell<Vec<String>>>,
}

/// Connect search entry signals (text changes, icon updates)
fn connect_search_signals(
    entry: &Entry,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
    obsidian_bar: &GtkBox,
    command_icon: &Image,
    obsidian_icon_name: String,
    pinned: &PinnedUiState,
) {
    // Handle text changes in search entry (main search functionality)
    let pinned_strip = pinned.strip.clone();
    let pinned_separator = pinned.separator.clone();
    let pinned_apps_clone = pinned.apps.clone();
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
            update_strip_visibility(&pinned_strip, &pinned_separator, &pinned, text.is_empty());

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

/// Set up right-click context menu on the results list
#[allow(clippy::too_many_arguments)]
fn setup_list_context_menu(
    list_view: &ListView,
    window: &ApplicationWindow,
    entry: &Entry,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pinned_strip: GtkBox,
    pinned_separator: GtkBox,
    toast_overlay: ToastOverlay,
) {
    let right_click = GestureClick::new();
    right_click.set_button(3);
    right_click.connect_pressed(clone!(
        #[weak]
        list_view,
        #[weak]
        window,
        #[strong]
        entry,
        #[strong]
        model,
        #[strong]
        current_mode,
        #[strong]
        pinned_apps,
        #[strong]
        all_apps,
        #[strong]
        pinned_strip,
        #[strong]
        pinned_separator,
        #[strong]
        toast_overlay,
        move |_gesture, _n_press, _click_x, click_y| {
            // Determine which model position was clicked
            let scroll_y = gtk4::prelude::ScrollableExt::vadjustment(&list_view)
                .map(|a| a.value())
                .unwrap_or(0.0);
            let row_height = list_view
                .first_child()
                .map(|c| c.allocation().height() as f64)
                .filter(|h| *h > 0.0)
                .unwrap_or(48.0);
            let clicked_pos = ((scroll_y + click_y) / row_height) as u32;

            let Some(obj) = model.store.item(clicked_pos) else {
                return;
            };

            // Select the clicked row so keyboard/actions target it
            model.selection.set_selected(clicked_pos);

            let mode = current_mode.get();

            // Build a popover with action buttons based on current mode
            let popover = Popover::new();
            popover.set_has_arrow(true);
            let weak_popover = glib::WeakRef::<Popover>::new();
            weak_popover.set(Some(&popover));

            let vbox = GtkBox::new(Orientation::Vertical, 0);
            vbox.add_css_class("context-menu-box");

            match mode {
                AppMode::Obsidian | AppMode::ObsidianGrep => {
                    build_obsidian_context_menu(
                        &obj,
                        &popover,
                        &vbox,
                        &weak_popover,
                        &model,
                        mode,
                        &window,
                    );
                }
                AppMode::FileSearch => {
                    build_file_search_context_menu(
                        &obj,
                        &popover,
                        &vbox,
                        &weak_popover,
                        &model,
                        &window,
                    );
                }
                AppMode::CustomScript => {
                    build_shell_context_menu(&obj, &popover, &vbox, &weak_popover, &model, &window);
                }
                _ => {
                    build_normal_context_menu(
                        &obj,
                        &popover,
                        &vbox,
                        &weak_popover,
                        &model,
                        mode,
                        &window,
                        &entry,
                        &pinned_apps,
                        &all_apps,
                        &pinned_strip,
                        &pinned_separator,
                        &toast_overlay,
                    );
                }
            }

            popover.set_child(Some(&vbox));
            popover.set_parent(&list_view);
            let rect = gdk::Rectangle::new(_click_x as i32, click_y as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));
            popover.popup();
        }
    ));
    list_view.add_controller(right_click);
}

#[allow(clippy::too_many_arguments)]
fn build_normal_context_menu(
    obj: &glib::Object,
    _popover: &Popover,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    model: &AppListModel,
    mode: AppMode,
    window: &ApplicationWindow,
    entry: &Entry,
    pinned_apps: &Rc<RefCell<Vec<String>>>,
    all_apps: &Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pinned_strip: &GtkBox,
    pinned_separator: &GtkBox,
    toast_overlay: &ToastOverlay,
) {
    // Determine if this app is pinned
    let (desktop_id_opt, is_pinned) = if let Some(app_item) = obj.downcast_ref::<AppItem>() {
        let exec = app_item.exec();
        let apps = all_apps.borrow();
        let did = apps
            .iter()
            .find(|a| a.exec == exec)
            .map(|a| a.desktop_id.clone());
        let pinned = did
            .as_ref()
            .map(|id| pinned_apps.borrow().contains(id))
            .unwrap_or(false);
        (did, pinned)
    } else {
        (None, false)
    };

    // Open
    let open_btn = make_menu_button("Open");
    let model_open = model.clone();
    let mode_open = mode;
    let win_open = window.clone();
    let obj_open = obj.clone();
    open_btn.connect_clicked(move |_| {
        activate_item(&obj_open, &model_open, mode_open, gdk::CURRENT_TIME);
        win_open.hide();
    });
    vbox.append(&open_btn);

    // Add / Remove from Favorites
    let entry_for_btns = entry.clone();
    if is_pinned {
        let btn = make_menu_button("Remove from Favorites");
        let did = desktop_id_opt.clone();
        let p_apps = pinned_apps.clone();
        let p_strip = pinned_strip.clone();
        let p_sep = pinned_separator.clone();
        let p_all = all_apps.clone();
        let win_ref = window.clone();
        let p_ref = weak_popover.clone();
        btn.connect_clicked(move |_| {
            if let Some(ref id) = did {
                p_apps.borrow_mut().retain(|d| d != id);
                info!("Removed from Favorites: {id}");
            }
            crate::ui::pinned_strip::save_pinned_apps(&p_apps.borrow());
            crate::ui::pinned_strip::refresh_pinned_strip(
                &p_strip,
                &p_sep,
                &p_apps,
                &p_all,
                &win_ref,
                &entry_for_btns,
            );
            if let Some(p) = p_ref.upgrade() {
                p.popdown();
            }
            entry_for_btns.grab_focus();
        });
        vbox.append(&btn);
    } else {
        let btn = make_menu_button("Add to Favorites");
        let did = desktop_id_opt.clone();
        let p_apps = pinned_apps.clone();
        let p_strip = pinned_strip.clone();
        let p_sep = pinned_separator.clone();
        let p_all = all_apps.clone();
        let win_ref = window.clone();
        let p_ref = weak_popover.clone();
        let toast_ref = toast_overlay.clone();
        let entry_add = entry_for_btns.clone();
        btn.connect_clicked(move |_| {
            let Some(ref id) = did else {
                if let Some(p) = p_ref.upgrade() {
                    p.popdown();
                }
                return;
            };
            if p_apps.borrow().len() >= 9 {
                if let Some(p) = p_ref.upgrade() {
                    p.popdown();
                }
                let toast = Toast::builder()
                    .title("Maximum 9 favourites reached")
                    .timeout(2)
                    .build();
                toast_ref.add_toast(toast);
                return;
            }
            let mut pinned = p_apps.borrow_mut();
            if !pinned.contains(id) {
                pinned.push(id.clone());
                info!("Added to Favorites: {id}");
            }
            drop(pinned);
            crate::ui::pinned_strip::save_pinned_apps(&p_apps.borrow());
            crate::ui::pinned_strip::refresh_pinned_strip(
                &p_strip, &p_sep, &p_apps, &p_all, &win_ref, &entry_add,
            );
            if let Some(p) = p_ref.upgrade() {
                p.popdown();
            }
            entry_add.grab_focus();
        });
        vbox.append(&btn);
    }
}

fn build_obsidian_context_menu(
    obj: &glib::Object,
    _popover: &Popover,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    model: &AppListModel,
    mode: AppMode,
    window: &ApplicationWindow,
) {
    let cmd_item = match obj.downcast_ref::<CommandItem>() {
        Some(item) => item,
        None => return,
    };

    let path = cmd_item.line();
    let p_ref = weak_popover.clone();

    // Open in Obsidian
    let open_btn = make_menu_button("Open in Obsidian");
    let obj_open = obj.clone();
    let model_open = model.clone();
    let mode_open = mode;
    let win_open = window.clone();
    open_btn.connect_clicked(move |_| {
        activate_item(&obj_open, &model_open, mode_open, gdk::CURRENT_TIME);
        win_open.hide();
    });
    vbox.append(&open_btn);

    // Copy note path
    let btn_copy_path = make_menu_button("Copy note path");
    let path_for_copy = path.clone();
    btn_copy_path.connect_clicked(move |_| {
        copy_text_to_clipboard(&path_for_copy);
        if let Some(p) = p_ref.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_copy_path);

    // Copy note content
    let btn_copy_content = make_menu_button("Copy note content");
    let path_for_content = path.clone();
    let p_ref2 = weak_popover.clone();
    btn_copy_content.connect_clicked(move |_| {
        match std::fs::read_to_string(&path_for_content) {
            Ok(content) => {
                copy_text_to_clipboard(&content);
            }
            Err(e) => {
                error!("Failed to read note file: {e}");
            }
        }
        if let Some(p) = p_ref2.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_copy_content);

    // Open in text editor
    let btn_editor = make_menu_button("Open in text editor");
    let path_for_editor = path.clone();
    let p_ref3 = weak_popover.clone();
    btn_editor.connect_clicked(move |_| {
        open_with_default_app(&path_for_editor);
        if let Some(p) = p_ref3.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_editor);

    // Show in file manager
    let btn_manager = make_menu_button("Show in file manager");
    let path_for_manager = path.clone();
    let p_ref4 = weak_popover.clone();
    btn_manager.connect_clicked(move |_| {
        open_in_file_manager(&path_for_manager);
        if let Some(p) = p_ref4.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_manager);
}

fn build_file_search_context_menu(
    obj: &glib::Object,
    _popover: &Popover,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    model: &AppListModel,
    window: &ApplicationWindow,
) {
    let cmd_item = match obj.downcast_ref::<CommandItem>() {
        Some(item) => item,
        None => return,
    };

    let path = cmd_item.line();

    // Open
    let open_btn = make_menu_button("Open");
    let obj_open = obj.clone();
    let model_open = model.clone();
    let win_open = window.clone();
    open_btn.connect_clicked(move |_| {
        activate_item(
            &obj_open,
            &model_open,
            AppMode::FileSearch,
            gdk::CURRENT_TIME,
        );
        win_open.hide();
    });
    vbox.append(&open_btn);

    // Copy path
    let btn_copy_path = make_menu_button("Copy path");
    let path_for_copy = path.clone();
    let p_ref2 = weak_popover.clone();
    btn_copy_path.connect_clicked(move |_| {
        copy_text_to_clipboard(&path_for_copy);
        if let Some(p) = p_ref2.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_copy_path);

    // Copy content (only for text files)
    if is_text_file(&path) {
        let btn_copy_content = make_menu_button("Copy content");
        let path_for_content = path.clone();
        let p_ref3 = weak_popover.clone();
        btn_copy_content.connect_clicked(move |_| {
            match std::fs::read_to_string(&path_for_content) {
                Ok(content) => {
                    copy_text_to_clipboard(&content);
                }
                Err(e) => {
                    error!("Failed to read file: {e}");
                }
            }
            if let Some(p) = p_ref3.upgrade() {
                p.popdown();
            }
        });
        vbox.append(&btn_copy_content);
    }

    // Copy file (as GFile for file manager paste)
    let btn_copy_file = make_menu_button("Copy file");
    let path_for_copy_file = path.clone();
    let p_ref4 = weak_popover.clone();
    btn_copy_file.connect_clicked(move |_| {
        copy_file_to_clipboard(&path_for_copy_file);
        if let Some(p) = p_ref4.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_copy_file);

    // Show in file manager
    let btn_manager = make_menu_button("Show in file manager");
    let path_for_manager = path.clone();
    let p_ref5 = weak_popover.clone();
    btn_manager.connect_clicked(move |_| {
        open_in_file_manager(&path_for_manager);
        if let Some(p) = p_ref5.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_manager);
}

fn build_shell_context_menu(
    obj: &glib::Object,
    _popover: &Popover,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    model: &AppListModel,
    window: &ApplicationWindow,
) {
    let cmd_item = match obj.downcast_ref::<CommandItem>() {
        Some(item) => item,
        None => return,
    };

    let line = cmd_item.line();
    let command = if let Some((_, cmd)) = line.split_once(" | ") {
        cmd.trim().to_string()
    } else if let Some(stripped) = line.strip_prefix("Run: ") {
        stripped.trim().to_string()
    } else {
        line
    };

    // Run
    let run_btn = make_menu_button("Run");
    let obj_run = obj.clone();
    let model_run = model.clone();
    let win_run = window.clone();
    run_btn.connect_clicked(move |_| {
        activate_item(
            &obj_run,
            &model_run,
            AppMode::CustomScript,
            gdk::CURRENT_TIME,
        );
        win_run.hide();
    });
    vbox.append(&run_btn);

    // Copy command
    let btn_copy_cmd = make_menu_button("Copy command");
    let cmd_for_copy = command.clone();
    let p_ref1 = weak_popover.clone();
    btn_copy_cmd.connect_clicked(move |_| {
        copy_text_to_clipboard(&cmd_for_copy);
        if let Some(p) = p_ref1.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&btn_copy_cmd);

    // Copy working directory (only if set)
    if let Some(working_dir) = cmd_item.working_dir() {
        let btn_copy_wd = make_menu_button("Copy working directory");
        let wd_for_copy = working_dir.clone();
        let p_ref2 = weak_popover.clone();
        btn_copy_wd.connect_clicked(move |_| {
            copy_text_to_clipboard(&wd_for_copy);
            if let Some(p) = p_ref2.upgrade() {
                p.popdown();
            }
        });
        vbox.append(&btn_copy_wd);
    }
}

/// Create a flat menu button for context menus
fn make_menu_button(label: &str) -> Button {
    let btn = Button::with_label(label);
    btn.add_css_class("flat");
    btn.add_css_class("context-menu-item");
    btn.set_halign(Align::Fill);
    btn.set_hexpand(true);
    btn
}

fn copy_text_to_clipboard(text: &str) {
    if let Some(display) = gdk::Display::default() {
        let clipboard = display.clipboard();
        clipboard.set_text(text);
    }
}

fn copy_file_to_clipboard(path: &str) {
    if let Some(display) = gdk::Display::default() {
        let file = gio::File::for_path(path);
        let value = file.to_value();
        let content_provider = gdk::ContentProvider::for_value(&value);
        let clipboard = display.clipboard();
        let _ = clipboard.set_content(Some(&content_provider));
    }
}

fn is_text_file(path: &str) -> bool {
    let (mime_str, _) = gio::content_type_guess(Some(path), None);
    mime_str.starts_with("text/")
        || mime_str == "application/x-shellscript"
        || mime_str == "application/json"
        || mime_str == "application/xml"
        || mime_str == "application/javascript"
        || mime_str.ends_with("+xml")
        || mime_str.ends_with("+json")
}

fn open_in_file_manager(path: &str) {
    let parent = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    if let Err(e) = std::process::Command::new("xdg-open").arg(&parent).spawn() {
        error!("Failed to open file manager: {e}");
    }
}

fn open_with_default_app(path: &str) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
        error!("Failed to open file with default app: {e}");
    }
}

/// Start background loading of desktop applications
#[allow(clippy::too_many_arguments)]
fn start_background_loading(
    cfg: &Config,
    model: &AppListModel,
    all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pinned_strip: GtkBox,
    pinned_separator: GtkBox,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    window: &ApplicationWindow,
    entry: Entry,
) {
    let dirs = cfg.expanded_app_dirs();
    let model_poll = model.clone();
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let _ = tx.send(launcher::load_apps(&dirs));
    });

    let win_clone = window.clone();
    glib::idle_add_local_once(move || {
        poll_apps(
            rx,
            model_poll,
            all_apps,
            pinned_strip,
            pinned_separator,
            pinned_apps,
            win_clone,
            entry,
        )
    });
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
/// - Alt+1..Alt+9: launch N-th pinned app
fn setup_keyboard_controller(
    list_view: &ListView,
    window: &ApplicationWindow,
    model: &AppListModel,
    current_mode: &Rc<Cell<AppMode>>,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
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
pub fn build_ui(app: &Application, cfg: &Config) {
    debug!("Workspace bar enabled: {}", cfg.workspace_bar_enabled);
    info!("Workspace bar enabled: {}", cfg.workspace_bar_enabled);

    // 1. Display and CSS Setup
    let display = gtk4::gdk::Display::default().expect("Cannot connect to display");
    setup_css(cfg, &display);

    // 2. Data Model Initialization
    let model = setup_model(cfg);
    let current_mode: Rc<Cell<AppMode>> = Rc::new(Cell::new(AppMode::Normal));

    // Shared state for loaded apps and pinned apps
    let all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>> = Rc::new(RefCell::new(Vec::new()));
    let pinned_apps: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(cfg.pinned_apps.clone()));

    // 3. Window Creation
    let window = create_window(app, cfg);

    // 4. Main Layout Construction
    // Main search entry field
    let entry = Entry::builder()
        .placeholder_text("Search applications…")
        .hexpand(true)
        .build();
    entry.add_css_class("search-entry");

    let (
        _root,
        list_view,
        obsidian_bar,
        command_icon,
        pinned_strip,
        pinned_separator,
        toast_overlay,
    ) = build_main_layout(&window, &entry, &model, cfg, &display);

    // Display the window
    window.present();

    // 5. Connect Signals
    // 5.1 Window lifecycle signals
    if let Some(ref obsidian_bar) = obsidian_bar {
        connect_window_signals(
            &window,
            &entry,
            obsidian_bar,
            &command_icon,
            &model,
            &current_mode,
        );
    }

    // 5.2 Icon Theme Configuration
    let icon_theme = gtk4::IconTheme::for_display(&display);
    let obsidian_icon_name = ["obsidian", "md.obsidian.Obsidian", "text-x-markdown"]
        .iter()
        .find(|&&name| icon_theme.has_icon(name))
        .copied()
        .unwrap_or("text-x-markdown");

    // 5.3 Search entry signals
    let pinned_ui = PinnedUiState {
        strip: pinned_strip.clone(),
        separator: pinned_separator.clone(),
        apps: pinned_apps.clone(),
    };
    if let Some(ref obsidian_bar) = obsidian_bar {
        connect_search_signals(
            &entry,
            &model,
            &current_mode,
            obsidian_bar,
            &command_icon,
            obsidian_icon_name.to_string(),
            &pinned_ui,
        );
    }

    // 5.4 Keyboard navigation
    setup_keyboard_controller(
        &list_view,
        &window,
        &model,
        &current_mode,
        pinned_apps.clone(),
        all_apps.clone(),
    );

    // 5.5 List view activation
    connect_list_signals(&list_view, &window, &model, &current_mode);

    // 5.6 List view context menu
    setup_list_context_menu(
        &list_view,
        &window,
        &entry,
        &model,
        &current_mode,
        pinned_apps.clone(),
        all_apps.clone(),
        pinned_strip.clone(),
        pinned_separator.clone(),
        toast_overlay,
    );

    // 6. Background Application Loading
    start_background_loading(
        cfg,
        &model,
        all_apps,
        pinned_strip,
        pinned_separator,
        pinned_apps,
        &window,
        entry,
    );
}
