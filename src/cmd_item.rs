use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct CommandItem {
        pub line: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CommandItem {
        const NAME: &'static str = "GrunnerCommandItem";
        type Type = super::CommandItem;
    }

    impl ObjectImpl for CommandItem {}
}

glib::wrapper! {
    pub struct CommandItem(ObjectSubclass<imp::CommandItem>);
}

impl CommandItem {
    pub fn new(line: String) -> Self {
        let obj: Self = Object::new();
        *obj.imp().line.borrow_mut() = line;
        obj
    }

    pub fn line(&self) -> String {
        self.imp().line.borrow().clone()
    }
}
