//! Context menu builder helpers for Grunner
//!
//! This module provides reusable helpers to build context menus across
//! different application modes (normal, obsidian, file search, shell).
//! It eliminates code duplication by extracting common patterns like
//! menu button creation, clipboard operations, and popover management.

use glib::WeakRef;
use glib::clone;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, GestureClick, Orientation, Popover};
use libadwaita::{ApplicationWindow, Toast, ToastOverlay};
use log::error;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::app_mode::AppMode;
use crate::core::config::Config;
use crate::item_activation::activate_item;
use crate::launcher;
use crate::model::items::{AppItem, CommandItem};
use crate::model::list_model::AppListModel;
use crate::ui::pinned_strip::{
    MAX_PINNED_APPS, add_pinned_app, can_add_pinned_app, refresh_pinned_strip, remove_pinned_app,
    save_pinned_apps,
};

/// Shared state for building a context menu
pub struct MenuContext {
    pub weak_popover: WeakRef<Popover>,
    pub vbox: GtkBox,
}

/// Shared UI state passed to context menu and other UI functions
#[derive(Clone)]
pub struct WindowCtx {
    pub window: ApplicationWindow,
    pub entry: gtk4::Entry,
    pub model: AppListModel,
    pub current_mode: Rc<Cell<AppMode>>,
    pub pinned_apps: Rc<RefCell<Vec<String>>>,
    pub all_apps: Rc<RefCell<Vec<launcher::DesktopApp>>>,
    pub pinned_strip: GtkBox,
    pub toast_overlay: ToastOverlay,
    pub dragging: Rc<Cell<bool>>,
    pub cfg: Config,
}

/// Create a flat menu button with standard CSS classes
#[must_use]
pub fn make_menu_button(label: &str) -> Button {
    let btn = Button::with_label(label);
    btn.add_css_class("flat");
    btn.add_css_class("context-menu-item");
    btn.set_halign(Align::Fill);
    btn.set_hexpand(true);
    btn
}

/// Add a menu button that runs a custom action and closes the popover
pub fn add_menu_button(ctx: &MenuContext, label: &str, action: impl Fn() + 'static) {
    let btn = make_menu_button(label);
    btn.connect_clicked(move |_| action());
    ctx.vbox.append(&btn);
}

/// Add a menu button that only closes the popover
#[allow(dead_code)]
pub fn add_popover_close_button(ctx: &MenuContext, label: &str) {
    let weak = ctx.weak_popover.clone();
    add_menu_button(ctx, label, move || {
        if let Some(p) = weak.upgrade() {
            p.popdown();
        }
    });
}

/// Add a menu button that copies text to clipboard and closes the popover
pub fn add_copy_text_button(ctx: &MenuContext, label: &str, text: &str) {
    let text = text.to_string();
    let weak = ctx.weak_popover.clone();
    add_menu_button(ctx, label, move || {
        copy_text_to_clipboard(&text);
        if let Some(p) = weak.upgrade() {
            p.popdown();
        }
    });
}

/// Add a menu button that reads a file, copies its content to clipboard, and closes the popover
pub fn add_copy_content_button(ctx: &MenuContext, label: &str, path: &str) {
    let path = path.to_string();
    let weak = ctx.weak_popover.clone();
    add_menu_button(ctx, label, move || {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                copy_text_to_clipboard(&content);
            }
            Err(e) => {
                error!("Failed to read file: {e}");
            }
        }
        if let Some(p) = weak.upgrade() {
            p.popdown();
        }
    });
}

/// Add a menu button that copies a file (as `GFile`) to clipboard and closes the popover
pub fn add_copy_file_button(ctx: &MenuContext, label: &str, path: &str) {
    let path = path.to_string();
    let weak = ctx.weak_popover.clone();
    add_menu_button(ctx, label, move || {
        let _ = copy_file_to_clipboard(&path);
        if let Some(p) = weak.upgrade() {
            p.popdown();
        }
    });
}

/// Add a menu button that opens a file in the file manager and closes the popover
pub fn add_open_in_file_manager_button(ctx: &MenuContext, label: &str, path: &str) {
    let path = path.to_string();
    let weak = ctx.weak_popover.clone();
    add_menu_button(ctx, label, move || {
        open_in_file_manager(&path);
        if let Some(p) = weak.upgrade() {
            p.popdown();
        }
    });
}

/// Add a menu button that opens a file with the default app and closes the popover
pub fn add_open_with_default_app_button(ctx: &MenuContext, label: &str, path: &str) {
    let path = path.to_string();
    let weak = ctx.weak_popover.clone();
    add_menu_button(ctx, label, move || {
        open_with_default_app(&path);
        if let Some(p) = weak.upgrade() {
            p.popdown();
        }
    });
}

// ---------------------------------------------------------------------------
// Clipboard operations
// ---------------------------------------------------------------------------

pub use crate::utils::clipboard::copy_file as copy_file_to_clipboard;
pub use crate::utils::clipboard::copy_text as copy_text_to_clipboard;

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

/// Check if a file is likely a text file based on its MIME type
#[must_use]
pub fn is_text_file(path: &str) -> bool {
    let (mime_str, _) = gtk4::gio::content_type_guess(Some(path), None);
    mime_str.starts_with("text/")
        || mime_str == "application/x-shellscript"
        || mime_str == "application/json"
        || mime_str == "application/xml"
        || mime_str == "application/javascript"
        || mime_str.ends_with("+xml")
        || mime_str.ends_with("+json")
}

/// Open the parent directory of a file in the default file manager
pub fn open_in_file_manager(path: &str) {
    let parent = std::path::Path::new(path)
        .parent()
        .map_or_else(|| path.to_string(), |p| p.to_string_lossy().to_string());

    if let Err(e) = std::process::Command::new("xdg-open").arg(&parent).spawn() {
        error!("Failed to open file manager: {e}");
    }
}

/// Open a file with the default application
pub fn open_with_default_app(path: &str) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
        error!("Failed to open file with default app: {e}");
    }
}

// ---------------------------------------------------------------------------
// Context menu dispatch
// ---------------------------------------------------------------------------

/// Set up right-click context menu on the results list
#[allow(clippy::cast_possible_truncation)]
pub fn setup_list_context_menu(list_view: &gtk4::ListView, ctx: &WindowCtx) {
    let right_click = GestureClick::new();
    right_click.set_button(3);
    let ctx = ctx.clone();
    right_click.connect_pressed(clone!(
        #[weak]
        list_view,
        move |_gesture, _n_press, click_x, click_y| {
            let clicked_pos = ctx.model.selection.selected();

            let Some(obj) = ctx.model.store.item(clicked_pos) else {
                return;
            };

            ctx.model.selection.set_selected(clicked_pos);
            let mode = ctx.current_mode.get();

            let popover = Popover::new();
            popover.set_has_arrow(true);
            let weak_popover = glib::WeakRef::<Popover>::new();
            weak_popover.set(Some(&popover));

            let vbox = GtkBox::new(Orientation::Vertical, 0);
            vbox.add_css_class("context-menu-box");

            match mode {
                AppMode::Obsidian | AppMode::ObsidianGrep => {
                    build_obsidian_context_menu(&obj, &vbox, &weak_popover, &ctx, mode);
                }
                AppMode::FileSearch => {
                    build_file_search_context_menu(&obj, &vbox, &weak_popover, &ctx);
                }
                AppMode::CustomScript => {
                    build_shell_context_menu(&obj, &vbox, &weak_popover, &ctx);
                }
                AppMode::Normal => {
                    build_normal_context_menu(&obj, &vbox, &weak_popover, &ctx, mode);
                }
            }

            popover.set_child(Some(&vbox));
            popover.set_parent(&list_view);
            let rect = gdk::Rectangle::new(click_x as i32, click_y as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));
            popover.popup();
        }
    ));
    list_view.add_controller(right_click);
}

// ---------------------------------------------------------------------------
// Context menu builders
// ---------------------------------------------------------------------------

fn build_normal_context_menu(
    obj: &glib::Object,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    ctx: &WindowCtx,
    mode: AppMode,
) {
    let ctx_menu = MenuContext {
        weak_popover: weak_popover.clone(),
        vbox: vbox.clone(),
    };

    let (desktop_id_opt, is_pinned) = if let Some(app_item) = obj.downcast_ref::<AppItem>() {
        let exec = app_item.exec();
        let apps = ctx.all_apps.borrow();
        let did = apps
            .iter()
            .find(|a| a.exec == exec)
            .map(|a| a.desktop_id.clone());
        let pinned = did
            .as_ref()
            .is_some_and(|id| ctx.pinned_apps.borrow().contains(id));
        (did, pinned)
    } else {
        (None, false)
    };

    let model_open = ctx.model.clone();
    let action_open = mode;
    let win_open = ctx.window.clone();
    let obj_open = obj.clone();
    add_menu_button(&ctx_menu, "Open", move || {
        activate_item(&obj_open, &model_open, action_open, gdk::CURRENT_TIME);
        win_open.hide();
    });

    let entry_for_btns = ctx.entry.clone();
    if is_pinned {
        let did = desktop_id_opt.clone();
        let p_apps = ctx.pinned_apps.clone();
        let p_strip = ctx.pinned_strip.clone();
        let p_all = ctx.all_apps.clone();
        let win_ref = ctx.window.clone();
        let weak = weak_popover.clone();
        let dragging_ref = ctx.dragging.clone();
        let cfg = ctx.cfg.clone();
        add_menu_button(&ctx_menu, "Remove from Favourites", move || {
            if let Some(ref id) = did {
                remove_pinned_app(&p_apps, id);
                save_pinned_apps(&p_apps.borrow(), &cfg);
            }
            refresh_pinned_strip(
                &p_strip,
                &p_apps,
                &p_all,
                &win_ref,
                entry_for_btns.text().is_empty(),
                &dragging_ref,
                &cfg,
            );
            if let Some(p) = weak.upgrade() {
                p.popdown();
            }
            entry_for_btns.grab_focus();
        });
    } else {
        let did = desktop_id_opt.clone();
        let p_apps = ctx.pinned_apps.clone();
        let p_strip = ctx.pinned_strip.clone();
        let p_all = ctx.all_apps.clone();
        let win_ref = ctx.window.clone();
        let weak = weak_popover.clone();
        let toast_ref = ctx.toast_overlay.clone();
        let entry_add = entry_for_btns.clone();
        let dragging_ref = ctx.dragging.clone();
        let cfg = ctx.cfg.clone();
        add_menu_button(&ctx_menu, "Add to Favourites", move || {
            let Some(ref id) = did else {
                if let Some(p) = weak.upgrade() {
                    p.popdown();
                }
                return;
            };
            if !can_add_pinned_app(&p_apps.borrow()) {
                if let Some(p) = weak.upgrade() {
                    p.popdown();
                }
                let toast = Toast::builder()
                    .title(format!("Maximum {MAX_PINNED_APPS} favourites reached"))
                    .timeout(2)
                    .build();
                toast_ref.add_toast(toast);
                return;
            }
            if add_pinned_app(&p_apps, id).is_ok() {
                save_pinned_apps(&p_apps.borrow(), &cfg);
            }
            refresh_pinned_strip(
                &p_strip,
                &p_apps,
                &p_all,
                &win_ref,
                entry_add.text().is_empty(),
                &dragging_ref,
                &cfg,
            );
            if let Some(p) = weak.upgrade() {
                p.popdown();
            }
            entry_add.grab_focus();
        });
    }
}

fn build_obsidian_context_menu(
    obj: &glib::Object,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    ctx: &WindowCtx,
    mode: AppMode,
) {
    let Some(cmd_item) = obj.downcast_ref::<CommandItem>() else {
        return;
    };

    let ctx_menu = MenuContext {
        weak_popover: weak_popover.clone(),
        vbox: vbox.clone(),
    };

    let path = cmd_item.line();

    let obj_open = obj.clone();
    let model_open = ctx.model.clone();
    let action_open = mode;
    let win_open = ctx.window.clone();
    add_menu_button(&ctx_menu, "Open in Obsidian", move || {
        activate_item(&obj_open, &model_open, action_open, gdk::CURRENT_TIME);
        win_open.hide();
    });

    add_copy_text_button(&ctx_menu, "Copy note path", &path);
    add_copy_content_button(&ctx_menu, "Copy note content", &path);
    add_open_with_default_app_button(&ctx_menu, "Open in text editor", &path);
    add_open_in_file_manager_button(&ctx_menu, "Show in file manager", &path);
}

fn build_file_search_context_menu(
    obj: &glib::Object,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    ctx: &WindowCtx,
) {
    let Some(cmd_item) = obj.downcast_ref::<CommandItem>() else {
        return;
    };

    let ctx_menu = MenuContext {
        weak_popover: weak_popover.clone(),
        vbox: vbox.clone(),
    };

    let path = cmd_item.line();

    let obj_open = obj.clone();
    let model_open = ctx.model.clone();
    let win_open = ctx.window.clone();
    add_menu_button(&ctx_menu, "Open", move || {
        activate_item(
            &obj_open,
            &model_open,
            AppMode::FileSearch,
            gdk::CURRENT_TIME,
        );
        win_open.hide();
    });

    add_copy_text_button(&ctx_menu, "Copy path", &path);

    if is_text_file(&path) {
        add_copy_content_button(&ctx_menu, "Copy content", &path);
    }

    add_copy_file_button(&ctx_menu, "Copy file", &path);
    add_open_in_file_manager_button(&ctx_menu, "Show in file manager", &path);
}

fn build_shell_context_menu(
    obj: &glib::Object,
    vbox: &GtkBox,
    weak_popover: &glib::WeakRef<Popover>,
    ctx: &WindowCtx,
) {
    let Some(cmd_item) = obj.downcast_ref::<CommandItem>() else {
        return;
    };

    let ctx_menu = MenuContext {
        weak_popover: weak_popover.clone(),
        vbox: vbox.clone(),
    };

    let line = cmd_item.line();
    let command = if let Some((_, cmd)) = line.split_once(" | ") {
        cmd.trim().to_string()
    } else if let Some(stripped) = line.strip_prefix("Run: ") {
        stripped.trim().to_string()
    } else {
        line
    };

    let obj_run = obj.clone();
    let model_run = ctx.model.clone();
    let win_run = ctx.window.clone();
    add_menu_button(&ctx_menu, "Run", move || {
        activate_item(
            &obj_run,
            &model_run,
            AppMode::CustomScript,
            gdk::CURRENT_TIME,
        );
        win_run.hide();
    });

    add_copy_text_button(&ctx_menu, "Copy command", &command);

    if let Some(working_dir) = cmd_item.working_dir() {
        add_copy_text_button(&ctx_menu, "Copy working directory", &working_dir);
    }
}
