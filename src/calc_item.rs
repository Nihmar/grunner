use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct CalcItem {
        pub result: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CalcItem {
        const NAME: &'static str = "GrunnerCalcItem";
        type Type = super::CalcItem;
    }

    impl ObjectImpl for CalcItem {}
}

glib::wrapper! {
    pub struct CalcItem(ObjectSubclass<imp::CalcItem>);
}

impl CalcItem {
    pub fn new(result: String) -> Self {
        let obj: Self = Object::new();
        *obj.imp().result.borrow_mut() = result;
        obj
    }

    pub fn result(&self) -> String {
        self.imp().result.borrow().clone()
    }
}