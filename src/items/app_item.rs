//! GTK Object wrapper for desktop application entries
//!
//! This module provides `AppItem`, a GTK object that wraps `DesktopApp` data
//! for use in GTK list models and UI components. It implements the necessary
//! GTK object subclassing to make application data accessible to GTK's property
//! system and list views.

use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

use crate::launcher::DesktopApp;

/// Internal implementation module for GTK object subclassing
///
/// GTK requires object implementations to be separated into an `imp` module
/// for encapsulation and proper object lifecycle management.
mod imp {
    use super::*;

    /// Internal data structure holding application metadata
    ///
    /// This struct stores all the properties of a desktop application
    /// that need to be exposed to the GTK UI layer.
    #[derive(Default)]
    pub struct AppItemInner {
        /// Display name of the application
        pub name: String,
        /// Description/comment from the .desktop file
        pub description: String,
        /// Icon name or path for the application
        pub icon: String,
        /// Command to execute when launching the application
        pub exec: String,
        /// Whether the application should run in a terminal
        pub terminal: bool,
    }

    /// Main GTK object implementation struct
    ///
    /// This struct implements the GTK object subclass. The `RefCell`
    /// allows mutable access to the internal data while maintaining
    /// Rust's borrowing rules within GTK's ownership model.
    #[derive(Default)]
    pub struct AppItem {
        /// Mutable application data wrapped in RefCell for interior mutability
        pub data: RefCell<AppItemInner>,
    }

    /// GTK object subclass implementation
    ///
    /// This macro-based implementation registers the object type with GTK
    /// and defines its behavior. The `ObjectImpl` trait is implemented
    /// with default behavior since no custom object methods are needed.
    #[glib::object_subclass]
    impl ObjectSubclass for AppItem {
        /// Unique type name for GTK's type system
        const NAME: &'static str = "GrunnerAppItem";
        /// Associated parent type (the public AppItem struct)
        type Type = super::AppItem;
    }

    /// Default implementation of GTK object methods
    ///
    /// No custom object behavior is needed beyond data storage,
    /// so we use the default implementation.
    impl ObjectImpl for AppItem {}
}

// Public GTK object wrapper for desktop applications
//
// This is the public-facing type that UI code interacts with.
// It wraps the internal GTK object implementation and provides
// a clean API for creating and accessing application data.
//
// The `glib::wrapper!` macro generates the necessary boilerplate
// to expose this as a proper GTK object.
glib::wrapper! {
    pub struct AppItem(ObjectSubclass<imp::AppItem>);
}

impl AppItem {
    /// Create a new AppItem from a DesktopApp
    ///
    /// # Arguments
    /// * `app` - The `DesktopApp` struct containing desktop entry data
    ///
    /// # Returns
    /// A new `AppItem` GTK object populated with the application data.
    pub fn new(app: &DesktopApp) -> Self {
        // Create a new GTK object instance
        let obj: Self = Object::new();

        // Initialize the internal data with values from the DesktopApp
        *obj.imp().data.borrow_mut() = imp::AppItemInner {
            name: app.name.clone(),
            description: app.description.clone(),
            icon: app.icon.clone(),
            exec: app.exec.clone(),
            terminal: app.terminal,
        };

        obj
    }

    /// Get the application's display name
    pub fn name(&self) -> String {
        self.imp().data.borrow().name.clone()
    }

    /// Get the application's description/comment
    pub fn description(&self) -> String {
        self.imp().data.borrow().description.clone()
    }

    /// Get the application's icon name or path
    pub fn icon(&self) -> String {
        self.imp().data.borrow().icon.clone()
    }

    /// Get the command to execute when launching the application
    pub fn exec(&self) -> String {
        self.imp().data.borrow().exec.clone()
    }

    /// Check if the application should run in a terminal
    pub fn terminal(&self) -> bool {
        self.imp().data.borrow().terminal
    }
}
