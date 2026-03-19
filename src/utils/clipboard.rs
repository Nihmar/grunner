//! Clipboard operations for Grunner
//!
//! This module provides centralized clipboard utilities for copying
//! text, files, and file contents to the system clipboard.

use glib::value::ToValue;
use gtk4::gdk;
use gtk4::gio;
use gtk4::prelude::DisplayExt;

pub fn copy_text(text: &str) {
    if let Some(display) = gdk::Display::default() {
        let clipboard = display.clipboard();
        clipboard.set_text(text);
    }
}

pub fn copy_file(path: &str) -> Result<(), String> {
    let display = gdk::Display::default().ok_or("No display available")?;
    let file = gio::File::for_path(path);
    let value = file.to_value();
    let content_provider = gdk::ContentProvider::for_value(&value);
    let clipboard = display.clipboard();
    clipboard
        .set_content(Some(&content_provider))
        .map_err(|e| format!("Failed to set clipboard content: {}", e))
}

#[allow(dead_code)]
pub fn copy_content(path: &str) -> Result<(), String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    copy_text(&content);
    Ok(())
}
