use crate::actions::{open_settings, power_action};
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Entry, Image, Label, Orientation};
use libadwaita::prelude::{AdwDialogExt, AlertDialogExt};
use libadwaita::{AlertDialog, ApplicationWindow, ResponseAppearance};


fn make_icon_button(label: &str, icon_candidates: &[&str], icon_theme: &gtk4::IconTheme) -> Button {
    let btn = Button::new();
    btn.add_css_class("power-button");

    let btn_box = GtkBox::new(Orientation::Horizontal, 6);
    btn_box.set_halign(Align::Center);

    if let Some(&icon_name) = icon_candidates.iter().find(|&&n| icon_theme.has_icon(n)) {
        let image = Image::from_icon_name(icon_name);
        image.set_pixel_size(16);
        btn_box.append(&image);
    }

    btn_box.append(&Label::new(Some(label)));
    btn.set_child(Some(&btn_box));
    btn
}

pub fn build_power_bar(
    window: &ApplicationWindow,
    entry: &Entry,
    icon_theme: &gtk4::IconTheme,
) -> GtkBox {
    let power_bar = GtkBox::new(Orientation::Horizontal, 8);
    power_bar.add_css_class("power-bar");
    power_bar.set_hexpand(true);
    power_bar.set_margin_top(4);
    power_bar.set_margin_bottom(8);
    power_bar.set_margin_start(12);
    power_bar.set_margin_end(12);


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
                open_settings();
                window.close();
            }
        ));
        power_bar.append(&btn);
    }

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    power_bar.append(&spacer);

    for (label, icon_candidates, action) in [
        (
            "Suspend",
            &[
                "system-suspend",
                "system-suspend-hibernate",
                "media-playback-pause",
            ][..],
            "suspend",
        ),
        (
            "Restart",
            &["system-restart", "system-reboot", "view-refresh"][..],
            "reboot",
        ),
        (
            "Power off",
            &["system-shutdown", "system-power-off"][..],
            "poweroff",
        ),
        (
            "Log out",
            &["system-log-out", "application-exit"][..],
            "logout",
        ),
    ] {
        let btn = make_icon_button(label, icon_candidates, icon_theme);

        let action = action.to_string();
        let label_str = label.to_string();
        btn.connect_clicked(clone!(
            #[weak]
            window,
            #[weak]
            entry,
            move |_| {
                let dialog = AlertDialog::builder()
                    .heading(format!("{}?", label_str))
                    .body(format!(
                        "Are you sure you want to {}?",
                        label_str.to_lowercase()
                    ))
                    .default_response("cancel")
                    .close_response("cancel")
                    .build();
                dialog.add_response("cancel", "Cancel");
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
                                window.close();
                                power_action(&action);
                            } else {
                                entry.grab_focus();
                            }
                        }
                    ),
                );
                dialog.present(Some(&window));
            }
        ));
        power_bar.append(&btn);
    }

    power_bar
}
