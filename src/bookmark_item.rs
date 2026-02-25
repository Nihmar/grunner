use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct BookmarkItem {
        pub title: RefCell<String>,
        pub url: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BookmarkItem {
        const NAME: &'static str = "GrunnerBookmarkItem";
        type Type = super::BookmarkItem;
    }

    impl ObjectImpl for BookmarkItem {}
}

glib::wrapper! {
    pub struct BookmarkItem(ObjectSubclass<imp::BookmarkItem>);
}

impl BookmarkItem {
    pub fn new(title: String, url: String) -> Self {
        let obj: Self = Object::new();
        *obj.imp().title.borrow_mut() = title;
        *obj.imp().url.borrow_mut() = url;
        obj
    }

    pub fn title(&self) -> String {
        self.imp().title.borrow().clone()
    }

    pub fn url(&self) -> String {
        self.imp().url.borrow().clone()
    }
}
