use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObsidianAction {
    #[default]
    OpenVault,
    NewNote,
    DailyNote,
    QuickNote,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ObsidianActionItem {
        pub action: RefCell<ObsidianAction>,
        pub arg: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ObsidianActionItem {
        const NAME: &'static str = "GrunnerObsidianActionItem";
        type Type = super::ObsidianActionItem;
    }

    impl ObjectImpl for ObsidianActionItem {}
}

glib::wrapper! {
    pub struct ObsidianActionItem(ObjectSubclass<imp::ObsidianActionItem>);
}

impl ObsidianActionItem {
    pub fn new(action: ObsidianAction, arg: Option<String>) -> Self {
        let obj: Self = Object::new();
        *obj.imp().action.borrow_mut() = action;
        *obj.imp().arg.borrow_mut() = arg;
        obj
    }

    pub fn action(&self) -> ObsidianAction {
        *self.imp().action.borrow()
    }

    pub fn arg(&self) -> Option<String> {
        self.imp().arg.borrow().clone()
    }
}
