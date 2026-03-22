//! Custom composite widget for search result rows
//!
//! `ResultRow` extends `GtkBox` and holds direct references to its
//! child widgets (`Image`, name `Label`, desc `Label`), eliminating
//! the need for tree traversal in every bind/unbind cycle.

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, Orientation};

mod imp {
    use super::*;
    use std::cell::OnceCell;

    /// Internal state for `ResultRow`
    #[derive(Default)]
    pub struct ResultRow {
        pub image: OnceCell<Image>,
        pub name_label: OnceCell<Label>,
        pub desc_label: OnceCell<Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ResultRow {
        const NAME: &'static str = "GrunnerResultRow";
        type Type = super::ResultRow;
        type ParentType = GtkBox;
    }

    impl ObjectImpl for ResultRow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let hbox: &GtkBox = obj.upcast_ref();

            hbox.set_orientation(Orientation::Horizontal);
            hbox.set_spacing(12);
            hbox.set_margin_top(6);
            hbox.set_margin_bottom(6);
            hbox.set_margin_start(12);
            hbox.set_margin_end(12);
            hbox.set_halign(Align::Fill);

            let image = Image::new();
            image.set_pixel_size(32);
            image.set_valign(Align::Center);
            image.add_css_class("app-icon");
            hbox.append(&image);

            let vbox = GtkBox::new(Orientation::Vertical, 2);
            vbox.set_valign(Align::Center);
            vbox.set_hexpand(true);

            let name_label = Label::new(None);
            name_label.set_halign(Align::Start);
            name_label.add_css_class("row-name");
            vbox.append(&name_label);

            let desc_label = Label::new(None);
            desc_label.set_halign(Align::Start);
            desc_label.add_css_class("row-desc");
            desc_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            desc_label.set_max_width_chars(70);
            vbox.append(&desc_label);

            hbox.append(&vbox);

            let _ = self.image.set(image);
            let _ = self.name_label.set(name_label);
            let _ = self.desc_label.set(desc_label);
        }
    }

    impl WidgetImpl for ResultRow {}
    impl BoxImpl for ResultRow {}
}

glib::wrapper! {
    /// Composite row widget with direct child references.
    ///
    /// Use [`image`](ResultRow::image), [`name_label`](ResultRow::name_label),
    /// and [`desc_label`](ResultRow::desc_label) to access children without
    /// tree traversal.
    pub struct ResultRow(ObjectSubclass<imp::ResultRow>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Orientable, gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl Default for ResultRow {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultRow {
    /// Create a new empty result row.
    #[must_use]
    pub fn new() -> Self {
        glib::Object::new()
    }

    /// Get the icon image widget.
    #[must_use]
    pub fn image(&self) -> &Image {
        self.imp()
            .image
            .get()
            .expect("image initialized in constructed")
    }

    /// Get the name label widget.
    #[must_use]
    pub fn name_label(&self) -> &Label {
        self.imp()
            .name_label
            .get()
            .expect("name_label initialized in constructed")
    }

    /// Get the description label widget.
    #[must_use]
    pub fn desc_label(&self) -> &Label {
        self.imp()
            .desc_label
            .get()
            .expect("desc_label initialized in constructed")
    }
}
