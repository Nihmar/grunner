//! GTK Object wrapper for GNOME Shell search provider results
//!
//! This module provides `SearchResultItem`, a GTK object that wraps search results
//! from GNOME Shell search providers for use in GTK list models and UI components.
//! It implements the necessary GTK object subclassing to make search result data
//! accessible to GTK's property system and list views.
//!
//! Search result items contain all metadata needed to display and activate
//! results from external search providers, including icons, descriptions,
//! and D-Bus addressing information for activation.

use glib::subclass::prelude::*;

/// Internal implementation module for GTK object subclassing
///
/// GTK requires object implementations to be separated into an `imp` module
/// for encapsulation and proper object lifecycle management.
mod imp {
    use super::*;
    use std::cell::RefCell;

    /// Internal data structure holding search result metadata
    ///
    /// This struct stores all the properties of a search provider result
    /// that need to be exposed to the GTK UI layer and used for result activation.
    #[derive(Default)]
    pub struct SearchResultItem {
        /// Unique identifier for this result within the provider
        pub id: RefCell<String>,
        /// Display name of the search result
        pub name: RefCell<String>,
        /// Descriptive text for the search result (may be empty)
        pub description: RefCell<String>,

        /// Themed icon name for this specific result
        ///
        /// This is a named icon from the current GTK icon theme (e.g., "text-x-generic").
        /// Takes precedence over `icon_file` if both are present.
        pub icon_themed: RefCell<String>,

        /// File-based icon path for this specific result
        ///
        /// This is an absolute filesystem path to an image file, typically used for
        /// thumbnails or custom icons that aren't available in the icon theme.
        pub icon_file: RefCell<String>,

        /// Application icon name from the provider's .desktop file
        ///
        /// Used as a fallback icon when neither `icon_themed` nor `icon_file` is available.
        /// This represents the provider application itself rather than the specific result.
        pub app_icon_name: RefCell<String>,
        /// D-Bus bus name of the search provider
        ///
        /// Required for activating the result when the user selects it.
        pub bus_name: RefCell<String>,
        /// D-Bus object path of the search provider
        ///
        /// Required for activating the result when the user selects it.
        pub object_path: RefCell<String>,
        /// Original search terms that produced this result
        ///
        /// Passed back to the provider when activating the result for context.
        pub terms: RefCell<Vec<String>>,
    }

    /// GTK object subclass implementation
    ///
    /// This macro-based implementation registers the object type with GTK
    /// and defines its behavior. The `ObjectImpl` trait is implemented
    /// with default behavior since no custom object methods are needed.
    #[glib::object_subclass]
    impl ObjectSubclass for SearchResultItem {
        /// Unique type name for GTK's type system
        const NAME: &'static str = "SearchResultItem";
        /// Associated parent type (the public SearchResultItem struct)
        type Type = super::SearchResultItem;
        /// Parent type in the GTK object hierarchy
        type ParentType = glib::Object;
    }

    /// Default implementation of GTK object methods
    ///
    /// No custom object behavior is needed beyond data storage,
    /// so we use the default implementation.
    impl ObjectImpl for SearchResultItem {}
}

// Public GTK object wrapper for search provider results
//
// This is the public-facing type that UI code interacts with.
// It wraps the internal GTK object implementation and provides
// a clean API for creating and accessing search result data.
//
// The `glib::wrapper!` macro generates the necessary boilerplate
// to expose this as a proper GTK object.
glib::wrapper! {
    pub struct SearchResultItem(ObjectSubclass<imp::SearchResultItem>);
}

impl SearchResultItem {
    /// Create a new SearchResultItem with all metadata fields
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the result within the provider
    /// * `name` - Display name for the result
    /// * `description` - Descriptive text (may be empty)
    /// * `icon_themed` - Themed icon name (empty string if not available)
    /// * `icon_file` - File-based icon path (empty string if not available)
    /// * `app_icon_name` - Provider application icon name
    /// * `bus_name` - D-Bus bus name of the search provider
    /// * `object_path` - D-Bus object path of the search provider
    /// * `terms` - Original search terms that produced this result
    ///
    /// # Returns
    /// A new `SearchResultItem` GTK object populated with the search result data.
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

        // Initialize all fields from the provided parameters
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

    /// Get the unique identifier for this result
    pub fn id(&self) -> String {
        self.imp().id.borrow().clone()
    }

    /// Get the display name for this result
    pub fn name(&self) -> String {
        self.imp().name.borrow().clone()
    }

    /// Get the descriptive text for this result
    pub fn description(&self) -> String {
        self.imp().description.borrow().clone()
    }

    /// Get the themed icon name for this result
    ///
    /// Returns empty string if no themed icon is available.
    pub fn icon_themed(&self) -> String {
        self.imp().icon_themed.borrow().clone()
    }

    /// Get the file-based icon path for this result
    ///
    /// Returns empty string if no file icon is available.
    pub fn icon_file(&self) -> String {
        self.imp().icon_file.borrow().clone()
    }

    /// Get the provider application icon name
    ///
    /// Used as a fallback icon when result-specific icons are not available.
    pub fn app_icon_name(&self) -> String {
        self.imp().app_icon_name.borrow().clone()
    }

    /// Get the D-Bus bus name of the search provider
    ///
    /// Required for activating this result via D-Bus.
    pub fn bus_name(&self) -> String {
        self.imp().bus_name.borrow().clone()
    }

    /// Get the D-Bus object path of the search provider
    ///
    /// Required for activating this result via D-Bus.
    pub fn object_path(&self) -> String {
        self.imp().object_path.borrow().clone()
    }

    /// Get the original search terms that produced this result
    ///
    /// These terms are passed back to the provider when activating the result.
    pub fn terms(&self) -> Vec<String> {
        self.imp().terms.borrow().clone()
    }
}
