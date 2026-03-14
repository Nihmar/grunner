//! General tab — window dimensions.

use super::make_tab_page;
use crate::config::Config;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, SpinRow};
use std::cell::RefCell;
use std::rc::Rc;

/// Append the "General" tab to `notebook`.
pub fn build_tab(notebook: &gtk4::Notebook, config_rc: &Rc<RefCell<Config>>) {
    let (scroll, inner) = make_tab_page();

    // ── Window ───────────────────────────────────────────────────────────────
    let window_group = PreferencesGroup::builder()
        .title("Window")
        .description("Configure the launcher window appearance")
        .build();

    let width_row = SpinRow::builder()
        .title("Window Width")
        .subtitle("Width of the launcher window in pixels")
        .build();
    width_row.set_range(400.0, 1920.0);
    width_row.adjustment().set_step_increment(10.0);
    width_row.adjustment().set_page_increment(50.0);
    width_row.set_value(f64::from(config_rc.borrow().window_width));
    width_row.connect_notify_local(Some("value"), {
        let config_rc = Rc::clone(config_rc);
        move |row, _| {
            config_rc.borrow_mut().window_width = row.value() as i32;
        }
    });
    window_group.add(&width_row);

    let height_row = SpinRow::builder()
        .title("Window Height")
        .subtitle("Height of the launcher window in pixels")
        .build();
    height_row.set_range(300.0, 1080.0);
    height_row.adjustment().set_step_increment(10.0);
    height_row.adjustment().set_page_increment(50.0);
    height_row.set_value(f64::from(config_rc.borrow().window_height));
    height_row.connect_notify_local(Some("value"), {
        let config_rc = Rc::clone(config_rc);
        move |row, _| {
            config_rc.borrow_mut().window_height = row.value() as i32;
        }
    });
    window_group.add(&height_row);
    inner.append(&window_group);

    notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("General"))));
}
