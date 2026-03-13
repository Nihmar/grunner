//! Commands tab — manage custom script commands for :sh mode.

use super::make_tab_page;
use crate::config::{CommandConfig, Config};
use gtk4::prelude::*;
use libadwaita::PreferencesGroup;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Append the "Commands" tab to `notebook`.
pub fn build_tab(notebook: &gtk4::Notebook, config_rc: &Rc<RefCell<Config>>) {
    let (scroll, inner) = make_tab_page();

    // ── Custom Script Commands ─────────────────────────────────────────────────
    let commands_group = PreferencesGroup::builder()
        .title("Custom Script Commands")
        .description("Commands available via :sh mode (name and command to execute)")
        .build();

    // Create a list box to display commands
    let list_box = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .build();
    list_box.add_css_class("boxed-list");

    // Function to refresh the list box with current commands
    let refresh_list = {
        let list_box = list_box.clone();
        let config_rc = Rc::clone(config_rc);
        move || {
            // Remove all existing rows
            while let Some(row) = list_box.first_child() {
                list_box.remove(&row);
            }

            // Add a row for each command
            let commands = config_rc.borrow().commands.clone();
            for (index, cmd) in commands.iter().enumerate() {
                let row = create_command_row(index, cmd, &config_rc);
                list_box.append(&row);
            }
        }
    };

    // Initial population
    refresh_list();

    // Add button to create new command
    let add_button = gtk4::Button::builder()
        .label("Add Command")
        .css_classes(["suggested-action"])
        .build();

    let list_box_clone = list_box.clone();
    let config_rc_clone = Rc::clone(config_rc);
    add_button.connect_clicked(move |_| {
        let new_cmd = CommandConfig {
            name: "New Command".to_string(),
            command: "echo 'Hello World'".to_string(),
        };
        {
            let mut cfg = config_rc_clone.borrow_mut();
            cfg.commands.push(new_cmd);
        }

        let row = create_command_row(
            config_rc_clone.borrow().commands.len() - 1,
            config_rc_clone.borrow().commands.last().unwrap(),
            &config_rc_clone,
        );
        list_box_clone.append(&row);
    });

    commands_group.add(&list_box);
    commands_group.add(&add_button);
    inner.append(&commands_group);

    notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Commands"))));
}

/// Create a row for a single command with edit and delete buttons
fn create_command_row(
    index: usize,
    cmd: &CommandConfig,
    config_rc: &Rc<RefCell<Config>>,
) -> gtk4::Box {
    let row_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    // Name entry
    let name_entry = gtk4::Entry::builder()
        .text(&cmd.name)
        .placeholder_text("Command name")
        .hexpand(true)
        .build();

    let config_rc_name = Rc::clone(config_rc);
    name_entry.connect_changed(move |entry| {
        let text = entry.text().to_string();
        if let Some(cmd) = config_rc_name.borrow_mut().commands.get_mut(index) {
            cmd.name = text;
        }
    });

    // Command entry
    let cmd_entry = gtk4::Entry::builder()
        .text(&cmd.command)
        .placeholder_text("Command to execute")
        .hexpand(true)
        .build();

    let config_rc_cmd = Rc::clone(config_rc);
    cmd_entry.connect_changed(move |entry| {
        let text = entry.text().to_string();
        if let Some(cmd) = config_rc_cmd.borrow_mut().commands.get_mut(index) {
            cmd.command = text;
        }
    });

    // Delete button
    let delete_button = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .css_classes(["destructive-action"])
        .build();

    let config_rc_del = Rc::clone(config_rc);
    let row_box_clone = row_box.clone();
    delete_button.connect_clicked(move |_| {
        {
            let mut cfg = config_rc_del.borrow_mut();
            if index < cfg.commands.len() {
                cfg.commands.remove(index);
            }
        }
        // Remove the row from UI
        if let Some(parent) = row_box_clone.parent()
            && let Some(list_box) = parent.downcast_ref::<gtk4::ListBox>()
        {
            list_box.remove(&row_box_clone);
        }
    });

    row_box.append(&name_entry);
    row_box.append(&cmd_entry);
    row_box.append(&delete_button);

    row_box
}
