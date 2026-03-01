//! Obsidian action bar module for Grunner
//!
//! This module provides the UI component for Obsidian-specific actions
//! that appear when the user enters Obsidian mode (via `:ob` command).
//! The bar contains buttons for common Obsidian operations like opening
//! the vault, creating new notes, daily notes, and quick notes.

use crate::actions::perform_obsidian_action;
use crate::list_model::AppListModel;
use crate::obsidian_item::ObsidianAction;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Entry, Orientation};
use libadwaita::ApplicationWindow;

/// Extract the argument from an Obsidian search query
///
/// When the user types `:ob something`, this function extracts
/// the "something" part to use as note content for new notes.
///
/// # Arguments
/// * `text` - The full search entry text (should start with `:ob `)
///
/// # Returns
/// The trimmed argument text after `:ob `, or empty string if no argument.
///
/// # Examples
/// - `":ob meeting notes"` → `"meeting notes"`
/// - `":ob"` → `""`
/// - `":ob   todo list  "` → `"todo list"`
pub fn extract_obsidian_arg(text: &str) -> &str {
    // Strip the ":ob " prefix and trim any surrounding whitespace
    text.strip_prefix(":ob ").map(str::trim).unwrap_or("")
}

/// Build the Obsidian action bar with buttons for Obsidian operations
///
/// This creates a horizontal bar containing buttons for common Obsidian
/// actions that appears below the search entry when in Obsidian mode.
/// The bar is hidden by default and shown only when `:ob` is entered.
///
/// # Arguments
/// * `window` - The main application window (for closing after action)
/// * `entry` - The search entry widget (for getting current text)
/// * `model` - The application list model (for Obsidian configuration)
///
/// # Returns
/// A `GtkBox` containing the Obsidian action buttons.
pub fn build_obsidian_bar(
    window: &ApplicationWindow,
    entry: &Entry,
    model: &AppListModel,
) -> GtkBox {
    // Create a horizontal box for the action buttons
    let obsidian_bar = GtkBox::new(Orientation::Horizontal, 8);
    obsidian_bar.set_halign(gtk4::Align::Center);
    obsidian_bar.set_margin_top(6);
    obsidian_bar.set_margin_bottom(6);
    obsidian_bar.set_visible(false); // Hidden by default, shown in Obsidian mode

    // Define the available Obsidian actions and their button labels
    let obsidian_actions = [
        ("Open Vault", ObsidianAction::OpenVault),
        ("New Note", ObsidianAction::NewNote),
        ("Daily Note", ObsidianAction::DailyNote),
        ("Quick Note", ObsidianAction::QuickNote),
    ];

    // Create a button for each Obsidian action
    for (label, action) in obsidian_actions {
        let btn = Button::with_label(label);
        btn.add_css_class("power-button"); // Use same styling as power buttons

        // Connect button click to perform the Obsidian action
        btn.connect_clicked(clone!(
            #[strong]
            model,
            #[weak]
            window,
            #[weak]
            entry,
            move |_| {
                // Get current text from search entry
                let current_text = entry.text();

                // Extract argument from Obsidian search (text after ":ob ")
                let arg = extract_obsidian_arg(&current_text);

                // Convert to Option<&str> if argument is non-empty
                let arg_opt = (!arg.is_empty()).then_some(arg);

                // Perform the Obsidian action if configuration is available
                if let Some(cfg) = &model.obsidian_cfg {
                    perform_obsidian_action(action, arg_opt, cfg);
                }

                // Close the window after performing the action
                window.close();
            }
        ));

        // Add button to the action bar
        obsidian_bar.append(&btn);
    }

    obsidian_bar
}
