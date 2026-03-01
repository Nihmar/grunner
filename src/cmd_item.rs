//! GTK Object wrapper for command line entries
//!
//! This module provides `CommandItem`, a GTK object that wraps command line strings
//! for use in GTK list models and UI components. It implements the necessary
//! GTK object subclassing to make command data accessible to GTK's property
//! system and list views.
//!
//! Command items are used to represent:
//! - Shell commands entered by the user
//! - File paths with line numbers
//! - Search results that can be executed or opened

use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

/// Internal implementation module for GTK object subclassing
///
/// GTK requires object implementations to be separated into an `imp` module
/// for encapsulation and proper object lifecycle management.
mod imp {
    use super::*;

    /// Main GTK object implementation struct for command items
    ///
    /// This struct implements the GTK object subclass. The `RefCell`
    /// allows mutable access to the command line text while maintaining
    /// Rust's borrowing rules within GTK's ownership model.
    #[derive(Default)]
    pub struct CommandItem {
        /// The command line text wrapped in RefCell for interior mutability
        ///
        /// This stores the actual command string that will be displayed
        /// in the UI and potentially executed when selected.
        pub line: RefCell<String>,
    }

    /// GTK object subclass implementation
    ///
    /// This macro-based implementation registers the object type with GTK
    /// and defines its behavior. The `ObjectImpl` trait is implemented
    /// with default behavior since no custom object methods are needed.
    #[glib::object_subclass]
    impl ObjectSubclass for CommandItem {
        /// Unique type name for GTK's type system
        const NAME: &'static str = "GrunnerCommandItem";
        /// Associated parent type (the public CommandItem struct)
        type Type = super::CommandItem;
    }

    /// Default implementation of GTK object methods
    ///
    /// No custom object behavior is needed beyond data storage,
    /// so we use the default implementation.
    impl ObjectImpl for CommandItem {}
}

// Public GTK object wrapper for command line entries
//
// This is the public-facing type that UI code interacts with.
// It wraps the internal GTK object implementation and provides
// a clean API for creating and accessing command data.
//
// The `glib::wrapper!` macro generates the necessary boilerplate
// to expose this as a proper GTK object.
glib::wrapper! {
    pub struct CommandItem(ObjectSubclass<imp::CommandItem>);
}

impl CommandItem {
    /// Create a new CommandItem from a command line string
    ///
    /// # Arguments
    /// * `line` - The command line text to store
    ///
    /// # Returns
    /// A new `CommandItem` GTK object populated with the command text.
    ///
    /// # Examples
    /// ```rust
    /// let cmd = CommandItem::new("ls -la".to_string());
    /// let cmd = CommandItem::new("/path/to/file.rs:42".to_string());
    /// ```
    pub fn new(line: String) -> Self {
        // Create a new GTK object instance
        let obj: Self = Object::new();
        // Initialize the internal data with the command line text
        *obj.imp().line.borrow_mut() = line;
        obj
    }

    /// Get the command line text stored in this item
    ///
    /// # Returns
    /// A clone of the command line string.
    ///
    /// # Usage
    /// This is typically called by UI code to display the command
    /// or by action handlers to execute the command.
    pub fn line(&self) -> String {
        self.imp().line.borrow().clone()
    }
}
