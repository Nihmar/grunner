//! UI factory and binding logic for list items
//!
//! This module separates UI presentation logic from the data model.
//! It handles the creation of GTK factories and the binding of data
//! to list items based on their type.

use crate::app_mode::ActiveMode;
use crate::model::items::{AppItem, CommandItem, ObsidianActionItem, SearchResultItem};
use crate::utils::{contract_home, get_file_icon, is_calculator_result};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Image, Label, ListItem, Orientation, SignalListItemFactory, Widget,
};

/// Context for binding list items, containing all necessary data
pub struct BindContext<'a> {
    pub image: &'a Image,
    pub name_label: &'a Label,
    pub desc_label: &'a Label,
    pub mode: ActiveMode,
    pub vault_path: Option<&'a str>,
}

impl<'a> BindContext<'a> {
    pub fn new(
        image: &'a Image,
        name_label: &'a Label,
        desc_label: &'a Label,
        mode: ActiveMode,
        vault_path: Option<&'a str>,
    ) -> Self {
        Self {
            image,
            name_label,
            desc_label,
            mode,
            vault_path,
        }
    }
}

/// Trait for binding strategies
///
/// Each strategy handles binding a specific type of item to the UI widgets.
/// This allows for easy extension and testing of individual binding behaviors.
pub trait BindStrategy {
    fn matches(&self, ctx: &BindContext, line: &str) -> bool;
    fn bind(&self, ctx: &BindContext, line: &str);
}

/// Strategy for calculator results
struct CalculatorBinder;

impl BindStrategy for CalculatorBinder {
    fn matches(&self, _ctx: &BindContext, line: &str) -> bool {
        is_calculator_result(line)
    }

    fn bind(&self, ctx: &BindContext, line: &str) {
        ctx.image.set_icon_name(Some("accessories-calculator"));
        if let Some((_expr, result)) = line.split_once('=') {
            ctx.name_label.set_text(result.trim());
            set_desc(ctx.desc_label, &format!("Calc: {line}"));
        } else {
            ctx.name_label.set_text(line);
            set_desc(ctx.desc_label, "Calculator result");
        }
    }
}

/// Strategy for shell command results (CustomScript mode)
struct ShellCommandBinder;

impl BindStrategy for ShellCommandBinder {
    fn matches(&self, ctx: &BindContext, line: &str) -> bool {
        ctx.mode == ActiveMode::CustomScript || line.starts_with("Run: ") || line.contains(" | ")
    }

    fn bind(&self, ctx: &BindContext, line: &str) {
        ctx.image.set_icon_name(Some("utilities-terminal"));
        if let Some((name, command)) = line.split_once(" | ") {
            ctx.name_label.set_text(name.trim());
            set_desc(ctx.desc_label, command.trim());
        } else if let Some(stripped) = line.strip_prefix("Run: ") {
            ctx.name_label.set_text("Run command");
            set_desc(ctx.desc_label, stripped);
        } else {
            ctx.name_label.set_text(line);
            set_desc(ctx.desc_label, "");
        }
    }
}

/// Strategy for grep results (file:line:content format)
struct GrepResultBinder;

impl BindStrategy for GrepResultBinder {
    fn matches(&self, ctx: &BindContext, line: &str) -> bool {
        ctx.mode == ActiveMode::ObsidianGrep
            || (line.contains(':') && !line.starts_with('/'))
            || (line.starts_with('/') && line.matches(':').count() >= 2)
    }

    fn bind(&self, ctx: &BindContext, line: &str) {
        if let Some((file_path, rest)) = line.split_once(':') {
            ctx.image.set_from_gicon(&get_file_icon(file_path));

            let display_path = if ctx.mode == ActiveMode::ObsidianGrep {
                relative_to_vault(file_path, ctx.vault_path)
            } else {
                file_path
            };
            let filename = std::path::Path::new(display_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(display_path);
            ctx.name_label.set_text(filename);
            set_desc(ctx.desc_label, rest);
        } else {
            ctx.image.set_icon_name(Some("text-markdown"));
            ctx.name_label.set_text(line);
            set_desc(ctx.desc_label, "");
        }
    }
}

/// Strategy for Obsidian file paths
struct ObsidianFileBinder;

impl BindStrategy for ObsidianFileBinder {
    fn matches(&self, ctx: &BindContext, line: &str) -> bool {
        ctx.mode == ActiveMode::ObsidianFile && line.starts_with('/')
    }

    fn bind(&self, ctx: &BindContext, line: &str) {
        ctx.image.set_icon_name(Some("text-markdown"));

        let (filename, _parent) = extract_filename_and_parent(line);
        ctx.name_label.set_text(filename);
        let relative = relative_to_vault(line, ctx.vault_path);
        let parent = std::path::Path::new(relative)
            .parent()
            .and_then(|p| p.to_str())
            .filter(|s| !s.is_empty())
            .or_else(|| std::path::Path::new(line).parent().and_then(|p| p.to_str()));
        set_desc(ctx.desc_label, parent.unwrap_or(""));
    }
}

/// Strategy for generic file paths
struct FilePathBinder;

impl BindStrategy for FilePathBinder {
    fn matches(&self, ctx: &BindContext, line: &str) -> bool {
        ctx.mode != ActiveMode::ObsidianFile && line.starts_with('/')
    }

    fn bind(&self, ctx: &BindContext, line: &str) {
        ctx.image.set_from_gicon(&get_file_icon(line));

        let (filename, parent) = extract_filename_and_parent(line);
        ctx.name_label.set_text(filename);

        let display_parent = if parent.is_empty() {
            String::new()
        } else {
            contract_home(std::path::Path::new(parent))
        };
        set_desc(ctx.desc_label, &display_parent);
    }
}

/// Default strategy for generic command output
struct DefaultBinder;

impl BindStrategy for DefaultBinder {
    fn matches(&self, _ctx: &BindContext, _line: &str) -> bool {
        true
    }

    fn bind(&self, ctx: &BindContext, line: &str) {
        ctx.image.set_icon_name(Some("system-search"));
        ctx.name_label.set_text(line);
        set_desc(ctx.desc_label, "");
    }
}

/// List of all binding strategies in order of priority
fn get_binders() -> Vec<Box<dyn BindStrategy>> {
    vec![
        Box::new(CalculatorBinder),
        Box::new(ShellCommandBinder),
        Box::new(GrepResultBinder),
        Box::new(ObsidianFileBinder),
        Box::new(FilePathBinder),
        Box::new(DefaultBinder),
    ]
}

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

/// Bind a command item to the list widget using strategy pattern
fn bind_command_item(
    image: &Image,
    name_label: &Label,
    desc_label: &Label,
    cmd_item: &CommandItem,
    mode: ActiveMode,
    vault_path: Option<&str>,
) {
    let line = cmd_item.line();
    let ctx = BindContext::new(image, name_label, desc_label, mode, vault_path);

    for strategy in get_binders() {
        if strategy.matches(&ctx, &line) {
            strategy.bind(&ctx, &line);
            return;
        }
    }
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
    // Try result-specific icons first, then fall back to provider app icon
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
