
use glib::subclass::prelude::*;

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct SearchResultItem {
        pub id: RefCell<String>,
        pub name: RefCell<String>,
        pub description: RefCell<String>,


        pub icon_themed: RefCell<String>,


        pub icon_file: RefCell<String>,


        pub app_icon_name: RefCell<String>,
        pub bus_name: RefCell<String>,
        pub object_path: RefCell<String>,
        pub terms: RefCell<Vec<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SearchResultItem {
        const NAME: &'static str = "SearchResultItem";
        type Type = super::SearchResultItem;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for SearchResultItem {}
}

glib::wrapper! {
    pub struct SearchResultItem(ObjectSubclass<imp::SearchResultItem>);
}

impl SearchResultItem {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        icon_themed: impl Into<String>,
        icon_file: impl Into<String>,
        app_icon_name: impl Into<String>,
        bus_name: impl Into<String>,
        object_path: impl Into<String>,
        terms: Vec<String>,
    ) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        *imp.id.borrow_mut() = id.into();
        *imp.name.borrow_mut() = name.into();
        *imp.description.borrow_mut() = description.into();
        *imp.icon_themed.borrow_mut() = icon_themed.into();
        *imp.icon_file.borrow_mut() = icon_file.into();
        *imp.app_icon_name.borrow_mut() = app_icon_name.into();
        *imp.bus_name.borrow_mut() = bus_name.into();
        *imp.object_path.borrow_mut() = object_path.into();
        *imp.terms.borrow_mut() = terms;
        obj
    }

    pub fn id(&self) -> String {
        self.imp().id.borrow().clone()
    }
    pub fn name(&self) -> String {
        self.imp().name.borrow().clone()
    }
    pub fn description(&self) -> String {
        self.imp().description.borrow().clone()
    }
    pub fn icon_themed(&self) -> String {
        self.imp().icon_themed.borrow().clone()
    }
    pub fn icon_file(&self) -> String {
        self.imp().icon_file.borrow().clone()
    }
    pub fn app_icon_name(&self) -> String {
        self.imp().app_icon_name.borrow().clone()
    }
    pub fn bus_name(&self) -> String {
        self.imp().bus_name.borrow().clone()
    }
    pub fn object_path(&self) -> String {
        self.imp().object_path.borrow().clone()
    }
    pub fn terms(&self) -> Vec<String> {
        self.imp().terms.borrow().clone()
    }
}
