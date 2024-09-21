use std::sync::OnceLock;

use gio::prelude::*;
use gio::subclass::prelude::*;
use glib::clone;
use gtk::gio;
use gtk::glib;

use crate::model;
use crate::model::prelude::*;

mod imp {
    use super::*;

    #[derive(Copy, Clone, Debug)]
    pub(crate) struct SelectableList(#[allow(dead_code)] glib::gobject_ffi::GTypeInterface);

    #[glib::object_interface]
    unsafe impl ObjectInterface for SelectableList {
        const NAME: &'static str = "SelectableList";
        type Prerequisites = (gio::ListModel,);

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecBoolean::builder("selection-mode").build(),
                    glib::ParamSpecUInt::builder("num-selected")
                        .read_only()
                        .build(),
                ]
            })
        }
    }
}

glib::wrapper! {
    pub(crate) struct SelectableList(ObjectInterface<imp::SelectableList>) @requires gio::ListModel;
}

impl SelectableList {
    pub(super) fn bootstrap(list: &Self) {
        list.connect_items_changed(|self_, position, _, added| {
            self_.notify("num-selected");
            (position..position + added)
                .map(|i| self_.item(i).unwrap())
                .for_each(|item| {
                    item.connect_notify_local(
                        Some("selected"),
                        clone!(@weak self_ as obj => move |_, _| obj.notify("num-selected")),
                    );
                });
        });
    }
}

pub(crate) trait SelectableListExt: IsA<SelectableList> {
    fn is_selection_mode(&self) -> bool;

    #[allow(dead_code)]
    fn set_selection_mode(&self, value: bool);

    #[allow(dead_code)]
    fn select_all(&self) {
        self.select(true);
    }

    fn select_none(&self) {
        self.select(false);
    }

    fn select(&self, value: bool);

    fn num_selected(&self) -> u32;

    fn selected_items(&self) -> Vec<model::Selectable>;
}

impl<T: IsA<SelectableList> + IsA<gio::ListModel>> SelectableListExt for T {
    fn is_selection_mode(&self) -> bool {
        self.property("selection-mode")
    }

    fn set_selection_mode(&self, value: bool) {
        if !value {
            self.select_none();
        }
        self.set_property("selection-mode", value);
    }

    fn select(&self, value: bool) {
        self.to_owned()
            .iter::<model::Selectable>()
            .map(|selectable| selectable.unwrap())
            .for_each(|selectable| selectable.set_selected(value));
    }

    fn num_selected(&self) -> u32 {
        self.to_owned()
            .iter::<model::Selectable>()
            .map(|selectable| selectable.unwrap())
            .filter(|obj| obj.is_selected())
            .count() as u32
    }

    fn selected_items(&self) -> Vec<model::Selectable> {
        self.to_owned()
            .iter::<model::Selectable>()
            .map(|selectable| selectable.unwrap())
            .filter(|obj| obj.is_selected())
            .collect()
    }
}

unsafe impl<T: ObjectSubclass> IsImplementable<T> for SelectableList {}
