use adw::subclass::prelude::ActionRowImpl;
use adw::subclass::prelude::PreferencesRowImpl;
use adw::traits::BinExt;
use adw::traits::PreferencesRowExt;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/marhkb/Pods/ui/widget/property_widget_row.ui")]
    pub(crate) struct PropertyWidgetRow {
        #[template_child]
        pub(super) bin: TemplateChild<adw::Bin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PropertyWidgetRow {
        const NAME: &'static str = "PdsPropertyWidgetRow";
        type Type = super::PropertyWidgetRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PropertyWidgetRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::builder("key")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecObject::builder::<gtk::Widget>("widget")
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
                "widget" => obj.set_widget(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = &*self.obj();
            match pspec.name() {
                "key" => obj.key().to_value(),
                "widget" => obj.widget().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for PropertyWidgetRow {}
    impl ListBoxRowImpl for PropertyWidgetRow {}
    impl PreferencesRowImpl for PropertyWidgetRow {}
    impl ActionRowImpl for PropertyWidgetRow {}

    #[gtk::template_callbacks]
    impl PropertyWidgetRow {
        #[template_callback]
        fn on_notify_title(&self) {
            self.obj().notify("key");
        }

        #[template_callback]
        fn on_bin_notify_child(&self) {
            self.obj().notify("widget");
        }
    }
}

glib::wrapper! {
    pub(crate) struct PropertyWidgetRow(ObjectSubclass<imp::PropertyWidgetRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow;
}

impl Default for PropertyWidgetRow {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl PropertyWidgetRow {
    pub(crate) fn key(&self) -> glib::GString {
        self.title()
    }

    pub(crate) fn set_key(&self, key: &str) {
        if key == self.key().as_str() {
            return;
        }
        self.set_title(key);
    }

    pub(crate) fn widget(&self) -> Option<gtk::Widget> {
        self.imp().bin.child()
    }

    pub(crate) fn set_widget(&self, value: Option<&gtk::Widget>) {
        if self.widget().as_ref() == value {
            return;
        }
        self.imp().bin.set_child(value);
    }
}
