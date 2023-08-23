use std::cell::Cell;
use std::cell::OnceCell;
use std::sync::OnceLock;

use glib::Properties;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::ObjectExt;
use gtk::prelude::ParamSpecBuilderExt;
use gtk::subclass::prelude::*;
use serde::Deserialize;
use serde::Serialize;

use crate::model;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct ConnectionInfo {
    pub(super) uuid: String,
    pub(super) name: String,
    pub(super) url: String,
    pub(super) rgb: Option<(f32, f32, f32)>,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Connection)]
    pub(crate) struct Connection {
        #[property(get, set, construct_only, nullable)]
        pub(super) manager: glib::WeakRef<model::ConnectionManager>,
        #[property(get, set)]
        pub(super) connecting: Cell<bool>,
        #[property(get, set)]
        pub(super) active: Cell<bool>,
        #[property(get, set, construct_only)]
        pub(super) uuid: OnceCell<String>,
        #[property(get, set, construct_only)]
        pub(super) name: OnceCell<String>,
        #[property(get, set, construct_only)]
        pub(super) url: OnceCell<String>,
        #[property(get, set, construct_only, nullable)]
        pub(super) rgb: Cell<Option<gdk::RGBA>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Connection {
        const NAME: &'static str = "Connection";
        type Type = super::Connection;
    }

    impl ObjectImpl for Connection {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                Self::derived_properties()
                    .iter()
                    .cloned()
                    .chain(Some(
                        glib::ParamSpecBoolean::builder("is-remote")
                            .read_only()
                            .build(),
                    ))
                    .collect::<Vec<_>>()
            })
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "is-remote" => self.obj().is_remote().to_value(),
                _ => self.derived_property(id, pspec),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().connect_connecting_notify(|obj| {
                if let Some(manager) = obj.manager() {
                    manager.notify("connecting");
                }
            });
        }
    }
}

glib::wrapper! {
    pub(crate) struct Connection(ObjectSubclass<imp::Connection>);
}

impl From<&Connection> for ConnectionInfo {
    fn from(connection: &Connection) -> Self {
        Self {
            uuid: connection.uuid(),
            name: connection.name(),
            url: connection.url(),
            rgb: connection
                .rgb()
                .map(|rgb| (rgb.red(), rgb.green(), rgb.blue())),
        }
    }
}

impl Connection {
    pub(crate) fn from_connection_info(
        connection_info: &ConnectionInfo,
        manager: &model::ConnectionManager,
    ) -> Self {
        Self::new(
            &connection_info.uuid,
            &connection_info.name,
            &connection_info.url,
            connection_info
                .rgb
                .map(|(r, g, b)| gdk::RGBA::new(r, g, b, 1.0)),
            manager,
        )
    }

    pub(crate) fn new(
        uuid: &str,
        name: &str,
        url: &str,
        rgb: Option<gdk::RGBA>,
        manager: &model::ConnectionManager,
    ) -> Self {
        glib::Object::builder()
            .property("manager", manager)
            .property("uuid", uuid)
            .property("name", name)
            .property("url", url)
            .property("rgb", rgb)
            .build()
    }

    pub(crate) fn is_local(&self) -> bool {
        self.url().starts_with("unix")
    }

    pub(crate) fn is_remote(&self) -> bool {
        !self.is_local()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.manager()
            .as_ref()
            .and_then(model::ConnectionManager::client)
            .map(|client| &client.connection() == self)
            .unwrap_or(false)
    }

    pub(crate) fn position(&self) -> u32 {
        self.manager()
            .map(|manager| manager.position_by_uuid(&self.uuid()))
            .unwrap_or(gtk::INVALID_LIST_POSITION)
    }
}
