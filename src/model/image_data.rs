use std::cell::OnceCell;

use glib::ObjectExt;
use glib::Properties;
use gtk::glib;
use gtk::subclass::prelude::*;

use crate::model;
use crate::podman;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::ImageData)]
    pub(crate) struct ImageData {
        #[property(get, set, construct_only, nullable)]
        pub(super) architecture: OnceCell<Option<String>>,
        #[property(get, set, construct_only, nullable)]
        pub(super) author: OnceCell<Option<String>>,
        #[property(get, set, construct_only, nullable)]
        pub(super) comment: OnceCell<Option<String>>,
        #[property(get, set, construct_only)]
        pub(super) config: OnceCell<model::ImageConfig>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ImageData {
        const NAME: &'static str = "ImageData";
        type Type = super::ImageData;
    }

    impl ObjectImpl for ImageData {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }
}

glib::wrapper! {
    pub(crate) struct ImageData(ObjectSubclass<imp::ImageData>);
}

impl From<podman::models::ImageData> for ImageData {
    fn from(data: podman::models::ImageData) -> Self {
        glib::Object::builder()
            .property("architecture", data.architecture)
            .property("author", data.author)
            .property("comment", data.comment)
            .property(
                "config",
                model::ImageConfig::from_libpod(data.config.unwrap()),
            )
            .build()
    }
}
