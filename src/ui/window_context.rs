use crate::app_mode::AppMode;
use crate::core::callbacks::AppCallbacks;
use crate::core::config::Config;
use crate::launcher;
use crate::model::list_model::AppListModel;
use crate::ui::context_menu::{WindowCtx, setup_list_context_menu};
use crate::ui::pinned_strip::{update_pinned_strip, update_strip_visibility};

use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Entry, GestureClick, Image, ListView};
use libadwaita::{ApplicationWindow, ToastOverlay};
use log::{error, info, trace};
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
// Pinned apps UI state
// ---------------------------------------------------------------------------

/// Pinned apps UI state
pub struct PinnedUiState {
    pub strip: GtkBox,
    pub apps: Rc<RefCell<Vec<String>>>,
}

// ---------------------------------------------------------------------------
// WindowContext
// ---------------------------------------------------------------------------

/// Internal context for UI construction phases
///
/// Groups all shared state needed during UI building so that helper
/// functions receive a single struct instead of many individual arguments.
#[derive(Clone)]
pub struct WindowContext {
    pub display: gdk::Display,
    pub cfg: Config,
    pub model: AppListModel,
    pub current_mode: Rc<Cell<AppMode>>,
    pub window: ApplicationWindow,
    pub callbacks: AppCallbacks,
    pub entry: Entry,
    pub list_view: ListView,
    pub obsidian_bar: Option<GtkBox>,
    pub command_icon: Image,
    pub pinned_strip: GtkBox,
    pub toast_overlay: ToastOverlay,
    pub all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pub pinned_apps: Rc<RefCell<Vec<String>>>,
    pub dragging: Rc<Cell<bool>>,
    pub theme_manager: crate::core::theme::ThemeManager,
}

impl WindowContext {
    pub fn ctx(&self) -> WindowCtx {
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

    pub fn setup_theme(&self) {
        self.theme_manager.apply(
            self.cfg.theme,
            self.cfg.custom_theme_path.as_deref(),
            &self.display,
        );
    }

    pub fn wire_callbacks(&self) {
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

    pub fn setup_dragging(&self, root: &GtkBox) {
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

    pub fn wire_signals(&self) {
        if let Some(ref obsidian_bar) = self.obsidian_bar {
            super::window::connect_window_signals(
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
            super::window::connect_search_signals(
                &self.entry,
                &self.model,
                &self.current_mode,
                obsidian_bar,
                &self.command_icon,
                obsidian_icon_name,
                &pinned_ui,
            );
        }

        super::window::setup_keyboard_controller(
            &self.list_view,
            &self.window,
            &self.model,
            &self.current_mode,
            &self.pinned_apps,
            &self.all_apps,
        );
        super::window::connect_list_signals(
            &self.list_view,
            &self.window,
            &self.model,
            &self.current_mode,
        );
        setup_list_context_menu(&self.list_view, &self.ctx());
    }

    pub fn start_loading(&self) {
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
