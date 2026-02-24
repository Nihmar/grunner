use std::rc::Rc;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use gio::prelude::*;
use gio::ListStore;
use gtk4::{ListItem, SignalListItemFactory, SingleSelection};

use crate::app_item::AppItem;
use crate::launcher::DesktopApp;

pub struct AppListModel {
    pub store: ListStore,
    pub selection: SingleSelection,
    all_apps: Rc<Vec<DesktopApp>>,
    max_results: usize,
}

impl AppListModel {
    pub fn new(all_apps: Rc<Vec<DesktopApp>>, max_results: usize) -> Self {
        let store = ListStore::new::<AppItem>();
        let selection = SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(true);
        selection.set_can_unselect(false);

        Self {
            store,
            selection,
            all_apps,
            max_results,
        }
    }

    pub fn populate(&self, query: &str) {
        self.store.remove_all();

        if query.is_empty() {
            // Show all apps (already sorted)
            let items: Vec<AppItem> = self
                .all_apps
                .iter()
                .map(|app| AppItem::new(app))
                .collect();
            self.store.extend_from_slice(&items);
            if self.store.n_items() > 0 {
                self.selection.set_selected(0);
            }
            return;
        }

        let matcher = SkimMatcherV2::default();
        let mut results: Vec<(i64, &DesktopApp)> = self
            .all_apps
            .iter()
            .filter_map(|app| {
                let name_score = matcher.fuzzy_match(&app.name, query).unwrap_or(i64::MIN);
                let desc_score = if !app.description.is_empty() {
                    matcher
                        .fuzzy_match(&app.description, query)
                        .unwrap_or(i64::MIN)
                        / 2
                } else {
                    i64::MIN
                };
                let score = name_score.max(desc_score);
                if score == i64::MIN {
                    None
                } else {
                    Some((score, app))
                }
            })
            .collect();

        results.sort_by(|a, b| b.0.cmp(&a.0));
        results.truncate(self.max_results);

        let items: Vec<AppItem> = results
            .iter()
            .map(|(_, app)| AppItem::new(app))
            .collect();
        self.store.extend_from_slice(&items);

        if self.store.n_items() > 0 {
            self.selection.set_selected(0);
        }
    }

    /// Creates the factory that will be used by the ListView.
    pub fn create_factory() -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<ListItem>().unwrap();

            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            hbox.set_margin_top(6);
            hbox.set_margin_bottom(6);
            hbox.set_margin_start(12);
            hbox.set_margin_end(12);

            let image = gtk4::Image::new();
            image.set_pixel_size(32);
            image.set_valign(gtk4::Align::Center);
            image.add_css_class("app-icon");
            hbox.append(&image);

            let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            vbox.set_valign(gtk4::Align::Center);
            vbox.set_hexpand(true);

            let name_label = gtk4::Label::new(None);
            name_label.set_halign(gtk4::Align::Start);
            name_label.add_css_class("row-name");
            vbox.append(&name_label);

            let desc_label = gtk4::Label::new(None);
            desc_label.set_halign(gtk4::Align::Start);
            desc_label.add_css_class("row-desc");
            desc_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            desc_label.set_max_width_chars(70);
            vbox.append(&desc_label);

            hbox.append(&vbox);
            list_item.set_child(Some(&hbox));

            unsafe {
                list_item.set_data("image", image);
                list_item.set_data("name_label", name_label);
                list_item.set_data("desc_label", desc_label);
            }
        });

        factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<ListItem>().unwrap();
            let item = match list_item.item().and_then(|o| o.downcast::<AppItem>().ok()) {
                Some(i) => i,
                None => return,
            };

            let image = unsafe { list_item.get_data::<gtk4::Image>("image") }.unwrap();
            let name_label = unsafe { list_item.get_data::<gtk4::Label>("name_label") }.unwrap();
            let desc_label = unsafe { list_item.get_data::<gtk4::Label>("desc_label") }.unwrap();

            let icon = item.icon();
            if icon.is_empty() {
                image.set_icon_name(Some("application-x-executable"));
            } else if icon.starts_with('/') {
                image.set_from_file(Some(&icon));
            } else {
                image.set_icon_name(Some(&icon));
            }

            name_label.set_text(&item.name());

            let desc = item.description();
            if desc.is_empty() {
                desc_label.set_visible(false);
                desc_label.set_text("");
            } else {
                desc_label.set_visible(true);
                desc_label.set_text(&desc);
            }
        });

        factory
    }
}