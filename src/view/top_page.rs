use futures::stream;
use futures::StreamExt;
use futures::TryStreamExt;
use gettextrs::gettext;
use glib::clone;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::unsync::OnceCell as UnsyncOnceCell;

use crate::model;
use crate::podman;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::TopPage)]
    #[template(resource = "/com/github/marhkb/Pods/ui/view/top_page.ui")]
    pub(crate) struct TopPage {
        /// A `Container` or a `Pod`
        pub(super) tree_store: UnsyncOnceCell<gtk::TreeStore>,
        #[property(get, set, construct_only, nullable)]
        pub(super) top_source: glib::WeakRef<glib::Object>,
        #[template_child]
        pub(super) window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(super) tree_view: TemplateChild<gtk::TreeView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TopPage {
        const NAME: &'static str = "PdsTopPage";
        type Type = super::TopPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for TopPage {
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

            if let Some(top_source) = obj.top_source() {
                if let Some(container) = top_source.downcast_ref::<model::Container>() {
                    self.window_title.set_title(&gettext("Container Processes"));
                    container.property_expression_weak("name").bind(
                        &*self.window_title,
                        "subtitle",
                        glib::Object::NONE,
                    );
                } else if let Some(pod) = top_source.downcast_ref::<model::Pod>() {
                    self.window_title.set_title(&gettext("Pod Processes"));
                    self.window_title.set_subtitle(&pod.name());
                }
            }

            obj.connect_top_stream();
        }

        fn dispose(&self) {
            utils::unparent_children(self.obj().upcast_ref());
        }
    }

    impl WidgetImpl for TopPage {}
}

glib::wrapper! {
    pub(crate) struct TopPage(ObjectSubclass<imp::TopPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::Container> for TopPage {
    fn from(container: &model::Container) -> Self {
        glib::Object::builder()
            .property("top-source", container)
            .build()
    }
}

impl From<&model::Pod> for TopPage {
    fn from(pod: &model::Pod) -> Self {
        glib::Object::builder().property("top-source", pod).build()
    }
}

impl TopPage {
    fn connect_top_stream(&self) {
        if let Some(processes_source) = self.top_source().as_ref().and_then(|obj| {
            if let Some(container) = obj.downcast_ref::<model::Container>() {
                container.api().map(|c| Box::new(c) as Box<dyn TopSource>)
            } else if let Some(pod) = obj.downcast_ref::<model::Pod>() {
                pod.api().map(|p| Box::new(p) as Box<dyn TopSource>)
            } else {
                unreachable!("unknown type for top source: {obj:?}")
            }
        }) {
            utils::run_stream(
                processes_source,
                move |container| container.stream(),
                clone!(@weak self as obj => @default-return glib::ControlFlow::Break, move |result: podman::Result<TopStreamElement>| {
                    match result {
                        Ok(top) => {
                            let imp = obj.imp();
                            let tree_store = imp.tree_store.get_or_init(|| {
                                let tree_store = gtk::TreeStore::new(
                                    &top.titles
                                        .iter()
                                        .map(|_| String::static_type())
                                        .collect::<Vec<_>>(),
                                );
                                imp.tree_view.set_model(Some(&tree_store));

                                top.titles.iter().enumerate().for_each(|(i, title)| {
                                    let column = gtk::TreeViewColumn::with_attributes(
                                        title,
                                        &gtk::CellRendererText::new(),
                                        &[("text", i as i32)],
                                    );
                                    column.set_sort_column_id(i as i32);
                                    column.set_sizing(gtk::TreeViewColumnSizing::GrowOnly);
                                    imp.tree_view.append_column(&column);
                                });

                                tree_store
                            });

                            // Remove processes that have disappeared.
                            tree_store.foreach(|_, _, iter| {
                                if !top
                                    .processes
                                    .iter()
                                    .any(|process| process[1] == tree_store.get::<String>(iter, 1))
                                {
                                    tree_store.remove(iter);
                                }
                                true
                            });

                            // Replace and add processes.
                            top.processes.iter().for_each(|process| {
                                let row = process.iter()
                                    .enumerate()
                                    .map(|(i, v)| (i as u32, v as &dyn gtk::prelude::ToValue))
                                    .collect::<Vec<_>>();

                                let mut replaced = false;

                                tree_store.foreach(|_, _, iter| {
                                    if process[1] == tree_store.get::<String>(iter, 1) {
                                        tree_store.set(iter, row.as_slice());
                                        replaced = true;
                                        true
                                    } else {
                                        false
                                    }
                                });

                                if !replaced {
                                    tree_store.set(&tree_store.append(None), row.as_slice());
                                }
                            });

                            glib::ControlFlow::Continue
                        }
                        Err(e) => {
                            log::warn!("Stopping top stream due to error: {e}");
                            glib::ControlFlow::Break
                        }
                    }
                }),
            );
        }
    }
}

trait TopSource: Send {
    fn stream(&self) -> stream::BoxStream<podman::Result<TopStreamElement>>;
}

impl TopSource for podman::api::Container {
    fn stream(&self) -> stream::BoxStream<podman::Result<TopStreamElement>> {
        self.top_stream(&podman::opts::ContainerTopOpts::builder().delay(2).build())
            .map_ok(|x| TopStreamElement {
                processes: x.processes,
                titles: x.titles,
            })
            .boxed()
    }
}

impl TopSource for podman::api::Pod {
    fn stream(&self) -> stream::BoxStream<podman::Result<TopStreamElement>> {
        self.top_stream(&podman::opts::PodTopOpts::builder().delay(2).build())
            .map_ok(|x| TopStreamElement {
                processes: x.processes,
                titles: x.titles,
            })
            .boxed()
    }
}

struct TopStreamElement {
    pub processes: Vec<Vec<String>>,
    pub titles: Vec<String>,
}
