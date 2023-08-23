use adw::subclass::prelude::ActionRowImpl;
use adw::subclass::prelude::PreferencesRowImpl;
use adw::traits::PreferencesRowExt;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/marhkb/Pods/ui/widget/property_row.ui")]
    pub(crate) struct PropertyRow {
        #[template_child]
        pub(super) value_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PropertyRow {
        const NAME: &'static str = "PdsPropertyRow";
        type Type = super::PropertyRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PropertyRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::builder("key")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecString::builder("value")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecEnum::builder_with_default::<pango::WrapMode>(
                        "value-wrap-mode",
                        pango::WrapMode::Word,
                    )
                    .explicit_notify()
                    .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = &*self.obj();
            match pspec.name() {
                "key" => obj.set_key(value.get().unwrap_or_default()),
                "value" => obj.set_value(value.get().unwrap_or_default()),
                "value-wrap-mode" => obj.set_value_wrap_mode(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = &*self.obj();
            match pspec.name() {
                "key" => obj.key().to_value(),
                "value" => obj.value().to_value(),
                "value-wrap-mode" => obj.value_wrap_mode().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for PropertyRow {}
    impl ListBoxRowImpl for PropertyRow {}
    impl PreferencesRowImpl for PropertyRow {}
    impl ActionRowImpl for PropertyRow {}

    #[gtk::template_callbacks]
    impl PropertyRow {
        #[template_callback]
        fn on_notify_title(&self) {
            self.obj().notify("key");
        }

        #[template_callback]
        fn on_notify_label(&self) {
            self.obj().notify("value");
        }

        #[template_callback]
        fn on_notify_wrap_mode(&self) {
            self.obj().notify("value-wrap-mode");
        }
    }
}

glib::wrapper! {
    pub(crate) struct PropertyRow(ObjectSubclass<imp::PropertyRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Actionable;
}

impl Default for PropertyRow {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl PropertyRow {
    pub(crate) fn new(key: &str, value: &str) -> Self {
        glib::Object::builder()
            .property("key", key)
            .property("value", value)
            .build()
    }

    pub(crate) fn key(&self) -> glib::GString {
        self.title()
    }

    pub(crate) fn set_key(&self, key: &str) {
        if key == self.key().as_str() {
            return;
        }
        self.set_title(key);
    }

    pub(crate) fn value(&self) -> glib::GString {
        self.imp().value_label.label()
    }

    pub(crate) fn set_value(&self, value: &str) {
        self.style_context().color();
        if value == self.value().as_str() {
            return;
        }
        self.imp().value_label.set_label(value);
    }

    pub(crate) fn value_wrap_mode(&self) -> pango::WrapMode {
        self.imp().value_label.wrap_mode()
    }

    pub(crate) fn set_value_wrap_mode(&self, mode: pango::WrapMode) {
        if mode == self.value_wrap_mode() {
            return;
        }
        self.imp().value_label.set_wrap_mode(mode);
    }
}
