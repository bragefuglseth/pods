use gettextrs::gettext;
use glib::closure;
use glib::subclass::InitializingObject;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::ConnectionRow)]
    #[template(resource = "/com/github/marhkb/Pods/ui/view/connection_row.ui")]
    pub(crate) struct ConnectionRow {
        pub(super) css_provider: gtk::CssProvider,
        #[property(get, set, nullable)]
        pub(super) client: glib::WeakRef<model::Client>,
        #[property(get, set = Self::set_connection, nullable)]
        pub(super) connection: glib::WeakRef<model::Connection>,
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) checkmark: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) url_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) color_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) end_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) delete_button: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ConnectionRow {
        const NAME: &'static str = "PdsConnectionRow";
        type Type = super::ConnectionRow;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("connectionrow");
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ConnectionRow {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            let connection_expr = Self::Type::this_expression("connection");
            let is_remote_expr = connection_expr.chain_property::<model::Connection>("is-remote");
            let is_active_expr = connection_expr.chain_property::<model::Connection>("active");

            is_remote_expr
                .chain_closure::<String>(closure!(|_: Self::Type, is_remote: bool| {
                    if is_remote {
                        "network-server-symbolic"
                    } else {
                        "local-connection-symbolic"
                    }
                }))
                .bind(&*self.image, "icon-name", Some(obj));

            gtk::ClosureExpression::new::<String>(
                [
                    &is_remote_expr,
                    &connection_expr.chain_property::<model::Connection>("url"),
                ],
                closure!(|_: Self::Type, is_remote: bool, url: String| {
                    if is_remote {
                        url
                    } else {
                        gettext("Local connection")
                    }
                }),
            )
            .bind(&*self.url_label, "label", Some(obj));

            let classes = utils::css_classes(self.image.upcast_ref());
            is_active_expr
                .chain_closure::<Vec<String>>(closure!(|_: Self::Type, is_active: bool| {
                    classes
                        .iter()
                        .cloned()
                        .chain(Some(String::from(if is_active {
                            "selected-connection"
                        } else {
                            "unselected-connection"
                        })))
                        .collect::<Vec<_>>()
                }))
                .bind(&*self.image, "css-classes", Some(obj));

            is_active_expr.bind(&*self.checkmark, "visible", Some(obj));

            connection_expr
                .chain_property::<model::Connection>("connecting")
                .chain_closure::<String>(closure!(
                    |_: Self::Type, connecting: bool| if connecting {
                        "connecting"
                    } else {
                        "delete"
                    }
                ))
                .bind(&*self.end_stack, "visible-child-name", Some(obj));

            connection_expr
                .chain_property::<model::Connection>("uuid")
                .chain_closure::<Option<glib::Variant>>(closure!(|_: Self::Type, uuid: &str| {
                    Some(uuid.to_variant())
                }))
                .bind(&*self.delete_button, "action-target", Some(obj));

            self.color_bin
                .style_context()
                .add_provider(&self.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        fn dispose(&self) {
            utils::unparent_children(self.obj().upcast_ref());
        }
    }

    impl WidgetImpl for ConnectionRow {}

    impl ConnectionRow {
        pub(super) fn set_connection(&self, value: Option<&model::Connection>) {
            let obj = &*self.obj();
            if obj.connection().as_ref() == value {
                return;
            }

            self.color_bin
                .set_visible(match value.and_then(model::Connection::rgb) {
                    Some(rgb) => {
                        self.css_provider.load_from_data(&format!(
                            "widget {{ background: shade({rgb}, 1.2); }}"
                        ));
                        true
                    }
                    None => false,
                });

            self.connection.set(value);
        }
    }
}

glib::wrapper! {
    pub(crate) struct ConnectionRow(ObjectSubclass<imp::ConnectionRow>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
