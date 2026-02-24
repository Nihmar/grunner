use glib::subclass::prelude::*;
use glib::Object;
use std::cell::RefCell;

use crate::launcher::DesktopApp;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct AppItemInner {
        pub name: String,
        pub description: String,
        pub icon: String,
        pub exec: String,
        pub terminal: bool,
    }

    #[derive(Default)]
    pub struct AppItem {
        pub data: RefCell<AppItemInner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppItem {
        const NAME: &'static str = "GrunnerAppItem";
        type Type = super::AppItem;
    }

    impl ObjectImpl for AppItem {}
}

glib::wrapper! {
    pub struct AppItem(ObjectSubclass<imp::AppItem>);
}

impl AppItem {
    pub fn new(app: &DesktopApp) -> Self {
        let obj: Self = Object::new();
        *obj.imp().data.borrow_mut() = imp::AppItemInner {
            name: app.name.clone(),
            description: app.description.clone(),
            icon: app.icon.clone(),
            exec: app.exec.clone(),
            terminal: app.terminal,
        };
        obj
    }

    pub fn name(&self) -> String {
        self.imp().data.borrow().name.clone()
    }
    pub fn description(&self) -> String {
        self.imp().data.borrow().description.clone()
    }
    pub fn icon(&self) -> String {
        self.imp().data.borrow().icon.clone()
    }
    pub fn exec(&self) -> String {
        self.imp().data.borrow().exec.clone()
    }
    pub fn terminal(&self) -> bool {
        self.imp().data.borrow().terminal
    }
}