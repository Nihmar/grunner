//! Power action bar module for Grunner
//!
//! This module provides the UI component for system power and management actions
//! that appear at the bottom of the Grunner window. The bar contains buttons for:
//! - Opening application settings
//! - System power operations (suspend, restart, power off, log out)
//!
//! Power operations are protected by confirmation dialogs to prevent accidental
//! activation, while settings access is immediate.

use crate::actions::{open_settings, power_action};
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Entry, Image, Label, Orientation};
use libadwaita::prelude::{AdwDialogExt, AlertDialogExt};
use libadwaita::{AlertDialog, ApplicationWindow, ResponseAppearance};

/// Create a button with an icon and label using available icon themes
///
/// This function attempts to find the best matching icon from a list of
/// candidates based on what's available in the current icon theme.
/// If no candidate icon is found, the button will display only the label.
///
/// # Arguments
/// * `label` - Text label to display on the button
/// * `icon_candidates` - List of icon names to try in order of preference
/// * `icon_theme` - The current GTK icon theme for icon availability checking
///
/// # Returns
/// A configured `Button` with icon (if available) and label, styled as a power button
fn make_icon_button(label: &str, icon_candidates: &[&str], icon_theme: &gtk4::IconTheme) -> Button {
    // Create button with power button styling
    let btn = Button::new();
    btn.add_css_class("power-button");

    // Create horizontal box to hold icon and label
    let btn_box = GtkBox::new(Orientation::Horizontal, 6);
    btn_box.set_halign(Align::Center);

    // Try each icon candidate in order until we find one available in the theme
    if let Some(&icon_name) = icon_candidates.iter().find(|&&n| icon_theme.has_icon(n)) {
        let image = Image::from_icon_name(icon_name);
        image.set_pixel_size(16); // Consistent icon size for power buttons
        btn_box.append(&image);
    }
    // Note: If no icon is found, the button will display only the label

    // Add the text label to the button
    btn_box.append(&Label::new(Some(label)));
    btn.set_child(Some(&btn_box));
    btn
}

/// Build the power action bar with system management buttons
///
/// Creates a horizontal bar at the bottom of the window containing:
/// - Settings button (left-aligned, no confirmation required)
/// - Spacer to push power buttons to the right
/// - Power operation buttons (suspend, restart, power off, log out) with confirmation dialogs
///
/// # Arguments
/// * `window` - The main application window (for closing after actions and dialog parenting)
/// * `entry` - The search entry widget (for refocusing after dialog cancellation)
/// * `icon_theme` - The current GTK icon theme for button icons
///
/// # Returns
/// A `GtkBox` containing all power action buttons properly arranged and configured
pub fn build_power_bar(
    window: &ApplicationWindow,
    entry: &Entry,
    icon_theme: &gtk4::IconTheme,
) -> GtkBox {
    // Create the main horizontal container for the power bar
    let power_bar = GtkBox::new(Orientation::Horizontal, 8);
    power_bar.add_css_class("power-bar");
    power_bar.set_hexpand(true);
    power_bar.set_margin_top(4);
    power_bar.set_margin_bottom(8);
    power_bar.set_margin_start(12);
    power_bar.set_margin_end(12);

    // --- Settings Button (left side) ---
    // Settings button provides immediate access to configuration without confirmation
    {
        let btn = make_icon_button(
            "Settings",
            &["preferences-system", "emblem-system", "settings-configure"],
            icon_theme,
        );
        btn.connect_clicked(clone!(
            #[weak]
            window,
            move |_| {
                // Open settings file with default editor
                open_settings();
                // Close Grunner window after opening settings
                window.close();
            }
        ));
        power_bar.append(&btn);
    }

    // Spacer to push power buttons to the right side of the bar
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    power_bar.append(&spacer);

    // --- Power Operation Buttons (right side) ---
    // Each power operation requires user confirmation via dialog
    for (label, icon_candidates, action) in [
        // Suspend system to RAM
        (
            "Suspend",
            &[
                "system-suspend",
                "system-suspend-hibernate",
                "media-playback-pause", // Fallback icon
            ][..],
            "suspend",
        ),
        // Restart/reboot the system
        (
            "Restart",
            &["system-restart", "system-reboot", "view-refresh"][..], // Fallback: refresh icon
            "reboot",
        ),
        // Power off/shutdown the system
        (
            "Power off",
            &["system-shutdown", "system-power-off"][..],
            "poweroff",
        ),
        // Log out of current user session
        (
            "Log out",
            &["system-log-out", "application-exit"][..], // Fallback: exit icon
            "logout",
        ),
    ] {
        let btn = make_icon_button(label, icon_candidates, icon_theme);

        // Clone variables for use in closure
        let action = action.to_string();
        let label_str = label.to_string();

        btn.connect_clicked(clone!(
            #[weak]
            window,
            #[weak]
            entry,
            move |_| {
                // Create confirmation dialog for destructive power operation
                let dialog = AlertDialog::builder()
                    .heading(format!("{}?", label_str))
                    .body(format!(
                        "Are you sure you want to {}?",
                        label_str.to_lowercase()
                    ))
                    .default_response("cancel")
                    .close_response("cancel")
                    .build();

                // Add Cancel button (safe, default action)
                dialog.add_response("cancel", "Cancel");

                // Add confirmation button with destructive appearance (warning color)
                dialog.add_response("confirm", &label_str);
                dialog.set_response_appearance("confirm", ResponseAppearance::Destructive);

                let action = action.clone();
                dialog.connect_response(
                    None,
                    clone!(
                        #[weak]
                        window,
                        #[weak]
                        entry,
                        move |_, response| {
                            if response == "confirm" {
                                // User confirmed - close window and perform action
                                window.close();
                                power_action(&action);
                            } else {
                                // User cancelled - refocus search entry for continued use
                                entry.grab_focus();
                            }
                        }
                    ),
                );

                // Show dialog centered on the main window
                dialog.present(Some(&window));
            }
        ));

        // Add button to the power bar
        power_bar.append(&btn);
    }

    power_bar
}
