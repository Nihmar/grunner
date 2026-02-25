use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ClipboardItem {
        pub text: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ClipboardItem {
        const NAME: &'static str = "GrunnerClipboardItem";
        type Type = super::ClipboardItem;
    }

    impl ObjectImpl for ClipboardItem {}
}

glib::wrapper! {
    pub struct ClipboardItem(ObjectSubclass<imp::ClipboardItem>);
}

impl ClipboardItem {
    pub fn new(text: String) -> Self {
        let obj: Self = Object::new();
        *obj.imp().text.borrow_mut() = text;
        obj
    }

    pub fn text(&self) -> String {
        self.imp().text.borrow().clone()
    }
}
