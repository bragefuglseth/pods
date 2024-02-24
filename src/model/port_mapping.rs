use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::subclass::Signal;
use glib::Properties;
use gtk::glib;

use crate::engine;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "PortMappingProtocol")]
pub(crate) enum Protocol {
    #[default]
    Tcp,
    Udp,
    Sctp,
}

impl FromStr for Protocol {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            "sctp" => Ok(Self::Sctp),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Tcp => "tcp",
                Self::Udp => "udp",
                Self::Sctp => "sctp",
            }
        )
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::PortMapping)]
    pub(crate) struct PortMapping {
        #[property(get, set)]
        pub(super) ip_address: RefCell<String>,
        #[property(get, set)]
        pub(super) host_port: Cell<i32>,
        #[property(get, set, minimum = 1, default = 1)]
        pub(super) container_port: Cell<i32>,
        #[property(get, set, builder(Protocol::default()))]
        pub(super) protocol: Cell<Protocol>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PortMapping {
        const NAME: &'static str = "PortMapping";
        type Type = super::PortMapping;
    }

    impl ObjectImpl for PortMapping {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![Signal::builder("remove-request").build()])
        }

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
    pub(crate) struct PortMapping(ObjectSubclass<imp::PortMapping>);
}

impl Default for PortMapping {
    fn default() -> Self {
        glib::Object::builder()
            .property("container-port", 1)
            .build()
    }
}

impl From<engine::PortMapping> for PortMapping {
    fn from(port_mapping: engine::PortMapping) -> Self {
        glib::Object::builder()
            .property("ip-address", port_mapping.host_ip.unwrap_or_default())
            .property(
                "host-port",
                port_mapping.host_port.map(|port| port as i32).unwrap_or(1),
            )
            .property(
                "container-port",
                port_mapping
                    .container_port
                    .map(|port| port as i32)
                    .unwrap_or(1),
            )
            .property(
                "protocol",
                port_mapping
                    .protocol
                    .as_deref()
                    .map(Protocol::from_str)
                    .transpose()
                    .ok()
                    .flatten()
                    .unwrap_or(Protocol::Tcp),
            )
            .build()
    }
}

impl PortMapping {
    pub(crate) fn remove_request(&self) {
        self.emit_by_name::<()>("remove-request", &[]);
    }

    pub(crate) fn connect_remove_request<F: Fn(&Self) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("remove-request", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);

            None
        })
    }
}
