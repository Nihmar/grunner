//! UI factory and binding logic for list items
//!
//! This module separates UI presentation logic from the data model.
//! It handles the creation of GTK factories and the binding of data
//! to list items based on their type.

use crate::app_mode::ActiveMode;
use crate::model::items::{AppItem, CommandItem, ObsidianActionItem, SearchResultItem};
use crate::utils::{contract_home, is_calculator_result};
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Image, Label, ListItem, Orientation, SignalListItemFactory, Widget,
};

/// Create a factory for the list view
///
/// This function builds a `GTK SignalListItemFactory` that handles
/// the creation and binding of list items based on their type.
#[must_use]
pub fn create_factory(
    active_mode: ActiveMode,
    vault_path: Option<String>,
) -> SignalListItemFactory {
    let factory = SignalListItemFactory::new();

    // Create signal for new list items
    factory.connect_setup(move |_factory, item| {
        let item = item
            .downcast_ref::<ListItem>()
            .expect("Needs to be ListItem");

        // Create horizontal box for icon and text
        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_top(6);
        hbox.set_margin_bottom(6);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_halign(Align::Fill);

        // Create icon
        let image = Image::new();
        image.set_pixel_size(32);
        image.set_valign(Align::Center);
        image.add_css_class("app-icon");
        hbox.append(&image);

        // Create vertical box for text (name + description)
        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_valign(Align::Center);
        vbox.set_hexpand(true);

        // Create name label
        let name_label = Label::new(None);
        name_label.set_halign(Align::Start);
        name_label.add_css_class("row-name");
        vbox.append(&name_label);

        // Create description label
        let desc_label = Label::new(None);
        desc_label.set_halign(Align::Start);
        desc_label.add_css_class("row-desc");
        desc_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        desc_label.set_max_width_chars(70);
        vbox.append(&desc_label);

        hbox.append(&vbox);
        item.set_child(Some(&hbox));
    });

    // Bind signal to populate data
    factory.connect_bind(move |_factory, item| {
        let item = item
            .downcast_ref::<ListItem>()
            .expect("Needs to be ListItem");
        let child = item.item().expect("Needs item");

        // Extract widgets from the list item
        let hbox = item
            .child()
            .and_then(|c| c.downcast::<GtkBox>().ok())
            .expect("missing hbox");
        let image = hbox
            .first_child()
            .and_then(|c| c.downcast::<Image>().ok())
            .expect("missing image");
        let vbox = image
            .next_sibling()
            .and_then(|c| c.downcast::<GtkBox>().ok())
            .expect("missing vbox");
        let name_label = vbox
            .first_child()
            .and_then(|c| c.downcast::<Label>().ok())
            .expect("missing name_label");
        let desc_label = name_label
            .next_sibling()
            .and_then(|c| c.downcast::<Label>().ok())
            .expect("missing desc_label");

        // Downcast to specific types and bind
        if let Some(app_item) = child.downcast_ref::<AppItem>() {
            bind_app_item(&image, &name_label, &desc_label, app_item);
        } else if let Some(cmd_item) = child.downcast_ref::<CommandItem>() {
            bind_command_item(
                &image,
                &name_label,
                &desc_label,
                cmd_item,
                active_mode,
                vault_path.as_deref(),
            );
        } else if let Ok(obs_item) = child.clone().downcast::<ObsidianActionItem>() {
            bind_obsidian_item(&image, &name_label, &desc_label, &obs_item);
        } else if let Ok(sr_item) = child.clone().downcast::<SearchResultItem>() {
            bind_search_result_item(&image, &name_label, &desc_label, &sr_item);
        }
    });

    // Unbind signal to clean up
    factory.connect_unbind(move |_factory, item| {
        let item = item
            .downcast_ref::<ListItem>()
            .expect("Needs to be ListItem");
        item.set_child(None::<&Widget>);
    });

    factory
}

/// Bind an application item to the list widget
fn bind_app_item(image: &Image, name_label: &Label, desc_label: &Label, app_item: &AppItem) {
    // Set icon
    let icon = app_item.icon();
    if icon.is_empty() {
        // Default executable icon for apps without specified icon
        image.set_icon_name(Some("application-x-executable"));
    } else if icon.starts_with('/') {
        // Absolute path to icon file
        image.set_from_file(Some(&icon));
    } else {
        // Themed icon name
        image.set_icon_name(Some(&icon));
    }

    // Set name and description
    name_label.set_text(&app_item.name());
    set_desc(desc_label, &app_item.description());
}

/// Set description label text with visibility handling
///
/// Shows the label only if text is non-empty, hiding it completely
/// when there's no description to avoid empty space in the UI.
fn set_desc(label: &Label, text: &str) {
    let visible = !text.is_empty();
    label.set_visible(visible);
    label.set_text(if visible { text } else { "" });
}

/// Convert absolute file path to vault-relative path for display
///
/// Strips the vault path prefix from absolute paths to show cleaner
/// relative paths in the UI when displaying Obsidian search results.
fn relative_to_vault<'a>(path: &'a str, vault: Option<&str>) -> &'a str {
    vault
        .and_then(|v| path.strip_prefix(v))
        .map_or(path, |s| s.trim_start_matches('/'))
}

/// Extract filename and parent directory from a path
fn extract_filename_and_parent(path: &str) -> (&str, &str) {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    let parent = std::path::Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    (filename, parent)
}

/// Bind a command item to the list widget (calculator, file paths, scripts)
fn bind_command_item(
    image: &Image,
    name_label: &Label,
    desc_label: &Label,
    cmd_item: &CommandItem,
    mode: ActiveMode,
    vault_path: Option<&str>,
) {
    let line = cmd_item.line();

    // Check if this is a calculator result
    if is_calculator_result(&line) {
        image.set_icon_name(Some("accessories-calculator"));
        if let Some((_expr, result)) = line.split_once('=') {
            name_label.set_text(result.trim());
            set_desc(desc_label, &format!("Calc: {line}"));
        } else {
            name_label.set_text(&line);
            set_desc(desc_label, "Calculator result");
        }
        return;
    }

    // Check if this is a shell command result
    if mode == ActiveMode::CustomScript || line.starts_with("Run: ") || line.contains(" | ") {
        image.set_icon_name(Some("utilities-terminal"));
        if let Some((name, command)) = line.split_once(" | ") {
            name_label.set_text(name.trim());
            set_desc(desc_label, command.trim());
        } else if let Some(stripped) = line.strip_prefix("Run: ") {
            name_label.set_text("Run command");
            set_desc(desc_label, stripped);
        } else {
            name_label.set_text(&line);
            set_desc(desc_label, "");
        }
        return;
    }

    // Check if this is a grep result (file:line:content format)
    // grep output: /path/file.md:42:content (starts with /, has at least 2 colons)
    // :obg also outputs in this format
    let is_grep_result = mode == ActiveMode::ObsidianGrep
        || (line.contains(":") && !line.starts_with("/"))
        || (line.starts_with('/') && line.matches(':').count() >= 2);
    if is_grep_result {
        if let Some((file_path, rest)) = line.split_once(':') {
            // Use content type to get the appropriate icon for the file
            let (ctype, _) = gio::content_type_guess(Some(file_path), None::<&[u8]>);
            image.set_from_gicon(&gio::content_type_get_icon(&ctype));

            // For ObsidianGrep, use vault-relative path; for FileSearch use full path
            let display_path = if mode == ActiveMode::ObsidianGrep {
                relative_to_vault(file_path, vault_path)
            } else {
                file_path
            };
            let filename = std::path::Path::new(display_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(display_path);
            name_label.set_text(filename);
            set_desc(desc_label, rest);
        } else {
            image.set_icon_name(Some("text-markdown"));
            name_label.set_text(&line);
            set_desc(desc_label, "");
        }
        return;
    }

    // Check if this is a file path (starts with /)
    if line.starts_with('/') {
        // Handle Obsidian file mode
        if mode == ActiveMode::ObsidianFile {
            // Use markdown icon for Obsidian file mode
            image.set_icon_name(Some("text-markdown"));

            let (filename, _parent) = extract_filename_and_parent(&line);
            name_label.set_text(filename);
            let relative = relative_to_vault(&line, vault_path);
            let parent = std::path::Path::new(relative)
                .parent()
                .and_then(|p| p.to_str())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    std::path::Path::new(&line)
                        .parent()
                        .and_then(|p| p.to_str())
                });
            set_desc(desc_label, parent.unwrap_or(""));
        } else {
            // Regular file path (not Obsidian mode)
            // Use generic icon based on file type
            let (ctype, _) = gio::content_type_guess(Some(&line), None::<&[u8]>);
            image.set_from_gicon(&gio::content_type_get_icon(&ctype));

            let (filename, parent) = extract_filename_and_parent(&line);
            name_label.set_text(filename);

            // Contract home directory to tilde
            let display_parent = if parent.is_empty() {
                String::new()
            } else {
                contract_home(std::path::Path::new(parent))
            };
            set_desc(desc_label, &display_parent);
        }
        return;
    }

    // Default: generic command output
    image.set_icon_name(Some("system-search"));
    name_label.set_text(&line);
    set_desc(desc_label, "");
}
/// Bind an Obsidian action item to the list widget
fn bind_obsidian_item(
    image: &Image,
    name_label: &Label,
    desc_label: &Label,
    obs_item: &ObsidianActionItem,
) {
    let icon_name = match obs_item.action() {
        crate::model::items::ObsidianAction::OpenVault => "org.obsidianmd.Obsidian",
        crate::model::items::ObsidianAction::NewNote => "document-new",
        crate::model::items::ObsidianAction::DailyNote => "x-office-calendar",
        crate::model::items::ObsidianAction::QuickNote => "document-new",
    };

    image.set_icon_name(Some(icon_name));

    let label_text = match obs_item.action() {
        crate::model::items::ObsidianAction::OpenVault => "Open Obsidian Vault",
        crate::model::items::ObsidianAction::NewNote => "New Obsidian Note",
        crate::model::items::ObsidianAction::DailyNote => "Daily Obsidian Note",
        crate::model::items::ObsidianAction::QuickNote => "Quick Obsidian Note",
    };

    name_label.set_text(label_text);
    set_desc(desc_label, "");
}

/// Bind a search result item (D-Bus provider) to the list widget
fn bind_search_result_item(
    image: &Image,
    name_label: &Label,
    desc_label: &Label,
    sr_item: &SearchResultItem,
) {
    // Icon (try to use the icon from the result, fallback to default)
    // Logic from original list_model.rs: bind_search_result_item
    let icon_file = sr_item.icon_file();
    let icon_themed = sr_item.icon_themed();
    let app_icon = sr_item.app_icon_name();

    if !icon_file.is_empty() {
        image.set_from_file(Some(&icon_file));
    } else if !icon_themed.is_empty() {
        image.set_icon_name(Some(&icon_themed));
    } else if !app_icon.is_empty() {
        image.set_icon_name(Some(&app_icon));
    } else {
        image.set_icon_name(Some("system-search"));
    }

    name_label.set_text(&sr_item.name());
    set_desc(desc_label, &sr_item.description());
}
