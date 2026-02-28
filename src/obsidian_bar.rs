use crate::actions::perform_obsidian_action;
use crate::list_model::AppListModel;
use crate::obsidian_item::ObsidianAction;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Entry, Orientation};
use libadwaita::ApplicationWindow;

/// Extracts the argument following ":ob " from the search entry text.
/// Returns an empty string if none is present.
pub fn extract_obsidian_arg(text: &str) -> &str {
    text.strip_prefix(":ob ").map(str::trim).unwrap_or("")
}

pub fn build_obsidian_bar(
    window: &ApplicationWindow,
    entry: &Entry,
    model: &AppListModel,
) -> GtkBox {
    let obsidian_bar = GtkBox::new(Orientation::Horizontal, 8);
    obsidian_bar.set_halign(gtk4::Align::Center);
    obsidian_bar.set_margin_top(6);
    obsidian_bar.set_margin_bottom(6);
    obsidian_bar.set_visible(false);

    let obsidian_actions = [
        ("Open Vault", ObsidianAction::OpenVault),
        ("New Note", ObsidianAction::NewNote),
        ("Daily Note", ObsidianAction::DailyNote),
        ("Quick Note", ObsidianAction::QuickNote),
    ];

    for (label, action) in obsidian_actions {
        let btn = Button::with_label(label);
        btn.add_css_class("power-button");

        btn.connect_clicked(clone!(
            #[strong]
            model,
            #[weak]
            window,
            #[weak]
            entry,
            move |_| {
                let current_text = entry.text();
                let arg = extract_obsidian_arg(&current_text);
                let arg_opt = (!arg.is_empty()).then_some(arg);

                if let Some(cfg) = &model.obsidian_cfg {
                    perform_obsidian_action(action, arg_opt, cfg);
                }
                window.close();
            }
        ));
        obsidian_bar.append(&btn);
    }

    obsidian_bar
}
