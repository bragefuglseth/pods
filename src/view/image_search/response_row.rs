use glib::closure;
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
    #[properties(wrapper_type = super::ResponseRow)]
    #[template(resource = "/com/github/marhkb/Pods/ui/image-search/response-row.ui")]
    pub(crate) struct ResponseRow {
        #[property(get, set, nullable)]
        pub(super) image_search_response: glib::WeakRef<model::ImageSearchResponse>,
        #[template_child]
        pub(super) description_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ResponseRow {
        const NAME: &'static str = "PdsImageSearchResponseRow";
        type Type = super::ResponseRow;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ResponseRow {
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

            Self::Type::this_expression("image-search-response")
                .chain_property::<model::ImageSearchResponse>("description")
                .chain_closure::<bool>(closure!(|_: Self::Type, description: Option<&str>| {
                    !description.map(str::is_empty).unwrap_or(true)
                }))
                .bind(&*self.description_label, "visible", Some(obj));
        }

        fn dispose(&self) {
            utils::unparent_children(self.obj().upcast_ref());
        }
    }

    impl WidgetImpl for ResponseRow {}
}

glib::wrapper! {
    pub(crate) struct ResponseRow(ObjectSubclass<imp::ResponseRow>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
