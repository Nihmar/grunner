//! Tab sub-modules for the settings dialog.
//!
//! Each public module exposes a single `build_tab` function that appends
//! one page to the shared `gtk4::Notebook`.

pub mod general;
pub mod info;
pub mod obsidian;
pub mod search;

use gtk4::prelude::*;

/// Create a scrolled tab page containing a vertical box of preference groups.
///
/// Returns `(ScrolledWindow, inner_Box)`.  Callers append
/// `PreferencesGroup` widgets to the inner box, then pass the
/// `ScrolledWindow` to `notebook.append_page`.
pub(super) fn make_tab_page() -> (gtk4::ScrolledWindow, gtk4::Box) {
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .css_classes(["tab-scroll"])
        .build();

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    inner.add_css_class("tab-inner");
    scroll.set_child(Some(&inner));

    if let Some(viewport) = scroll.child() {
        viewport.add_css_class("tab-viewport");
    }

    (scroll, inner)
}
