//! Context menu builder helpers for Grunner
//!
//! This module provides reusable helpers to build context menus across
//! different application modes (normal, obsidian, file search, shell).
//! It eliminates code duplication by extracting common patterns like
//! menu button creation, clipboard operations, and popover management.

use glib::WeakRef;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Popover};
use log::error;

/// Shared state for building a context menu
pub struct MenuContext {
    pub weak_popover: WeakRef<Popover>,
    pub vbox: GtkBox,
}

/// Create a flat menu button with standard CSS classes
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

/// Add a menu button that copies a file (as GFile) to clipboard and closes the popover
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
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

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
