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
                let row = create_command_row(index, cmd, &config_rc, &list_box);
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
        .margin_top(6)
        .build();

    let list_box_clone = list_box.clone();
    let config_rc_clone = Rc::clone(config_rc);
    add_button.connect_clicked(move |_| {
        let new_cmd = CommandConfig {
            name: "New Command".to_string(),
            command: "echo 'Hello World'".to_string(),
            working_dir: None,
            keep_open: true,
        };
        {
            let mut cfg = config_rc_clone.borrow_mut();
            cfg.commands.push(new_cmd);
        }

        let row = create_command_row(
            config_rc_clone.borrow().commands.len() - 1,
            config_rc_clone.borrow().commands.last().unwrap(),
            &config_rc_clone,
            &list_box_clone,
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
    list_box: &gtk4::ListBox,
) -> gtk4::Box {
    let row_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();

    // First row: Name, Command, Delete button
    let top_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
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
    let list_box_for_refresh = list_box.clone();
    delete_button.connect_clicked(move |_| {
        {
            let mut cfg = config_rc_del.borrow_mut();
            if index < cfg.commands.len() {
                cfg.commands.remove(index);
            }
        }
        // Refresh the entire list to recalculate indices
        while let Some(row) = list_box_for_refresh.first_child() {
            list_box_for_refresh.remove(&row);
        }
        for (i, cmd) in config_rc_del.borrow().commands.iter().enumerate() {
            let row = create_command_row(i, cmd, &config_rc_del, &list_box_for_refresh);
            list_box_for_refresh.append(&row);
        }
    });

    top_row.append(&name_entry);
    top_row.append(&cmd_entry);
    top_row.append(&delete_button);

    // Second row: Working directory, Keep terminal open switch
    let bottom_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();

    // Working directory entry
    let workdir_entry = gtk4::Entry::builder()
        .text(cmd.working_dir.as_deref().unwrap_or(""))
        .placeholder_text("Working directory (empty = home)")
        .hexpand(true)
        .build();

    let config_rc_workdir = Rc::clone(config_rc);
    workdir_entry.connect_changed(move |entry| {
        let text = entry.text().to_string();
        if let Some(cmd) = config_rc_workdir.borrow_mut().commands.get_mut(index) {
            cmd.working_dir = if text.is_empty() { None } else { Some(text) };
        }
    });

    // Keep terminal open switch
    let keep_open_switch = gtk4::Switch::builder()
        .active(cmd.keep_open)
        .valign(gtk4::Align::Center)
        .build();

    let keep_open_label = gtk4::Label::new(Some("Keep terminal open"));
    keep_open_label.set_margin_end(8);
    keep_open_label.set_valign(gtk4::Align::Center);

    let config_rc_keep = Rc::clone(config_rc);
    keep_open_switch.connect_state_set(move |_switch, state| {
        if let Some(cmd) = config_rc_keep.borrow_mut().commands.get_mut(index) {
            cmd.keep_open = state;
        }
        glib::Propagation::Proceed
    });

    bottom_row.append(&workdir_entry);
    bottom_row.append(&keep_open_label);
    bottom_row.append(&keep_open_switch);

    row_box.append(&top_row);
    row_box.append(&bottom_row);

    row_box
}
