use gettextrs::gettext;
use gettextrs::ngettext;
use gtk::gio;
use gtk::glib;
use gtk::glib::clone;
use gtk::glib::closure;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;

use crate::model;
use crate::utils;
use crate::view;

const ACTION_RUN_HEALTH_COMMAND: &str = "container-health-check-page.run-health-check";

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/marhkb/Pods/ui/container/health-check-page.ui")]
    pub(crate) struct HealthCheckPage {
        pub(super) container: glib::WeakRef<model::Container>,
        #[template_child]
        pub(super) status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) command_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) interval_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) retries_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) timeout_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) failing_streak_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) log_list_box: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HealthCheckPage {
        const NAME: &'static str = "PdsContainerHealthCheckPage";
        type Type = super::HealthCheckPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action(ACTION_RUN_HEALTH_COMMAND, None, |widget, _, _| {
                widget.run_health_check()
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HealthCheckPage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::Container>("container")
                        .construct()
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "container" => self.obj().set_container(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "container" => self.obj().container().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            let container_expr = Self::Type::this_expression("container");

            let health_status_expr =
                container_expr.chain_property::<model::Container>("health_status");
            let data_expr = container_expr.chain_property::<model::Container>("data");

            gtk::ClosureExpression::new::<bool>(
                [&container_expr.chain_property::<model::Container>("status"), &health_status_expr],
                closure!(|_: Self::Type,
                          _: model::ContainerStatus,
                          _: model::ContainerHealthStatus| false),
            )
            .watch(Some(obj), clone!(@weak obj => move || {
                obj.action_set_enabled(
                    ACTION_RUN_HEALTH_COMMAND,
                    obj.container()
                        .map(|container| {
                            container.health_status() != model::ContainerHealthStatus::Unconfigured
                                && container.status() == model::ContainerStatus::Running
                        })
                        .unwrap_or(false),
                );
            }));

            health_status_expr
                .chain_closure::<String>(closure!(
                    |_: Self::Type, status: model::ContainerHealthStatus| status.to_string()
                ))
                .bind(&*self.status_label, "label", Some(obj));

            let css_classes = self.status_label.css_classes();
            health_status_expr
                .chain_closure::<Vec<String>>(closure!(
                    |_: Self::Type, status: model::ContainerHealthStatus| {
                        css_classes
                            .iter()
                            .cloned()
                            .chain(Some(glib::GString::from(
                                super::super::container_health_status_css_class(status),
                            )))
                            .collect::<Vec<_>>()
                    }
                ))
                .bind(&*self.status_label, "css-classes", Some(obj));

            data_expr.watch(Some(obj), clone!(@weak obj => move || {
                let model = obj.container().as_ref().and_then(model::Container::data).map(model::ContainerData::health_check_log_list);

                if let Some(ref model) = model {
                    obj.set_list_box_visibility(model.upcast_ref());
                    model.connect_items_changed(clone!(@weak obj => move |model, _, _, _| {
                        obj.set_list_box_visibility(model.upcast_ref());
                    }));
                }

                let sort_model = gtk::SortListModel::new(model.as_ref(), Some(&gtk::CustomSorter::new(|item1, item2| {
                    let log1 = item1.downcast_ref::<model::HealthCheckLog>().unwrap();
                    let log2 = item2.downcast_ref::<model::HealthCheckLog>().unwrap();
                    log2.start().cmp(log1.start()).into()
                })));

                obj.imp().log_list_box.bind_model(Some(&sort_model), move |log| {
                    view::HealthCheckLogRow::from(log.downcast_ref().unwrap()).upcast()
                })
            }));
        }

        fn dispose(&self) {
            utils::ChildIter::from(self.obj().upcast_ref()).for_each(|child| child.unparent());
        }
    }

    impl WidgetImpl for HealthCheckPage {}
}

glib::wrapper! {
    pub(crate) struct HealthCheckPage(ObjectSubclass<imp::HealthCheckPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::Container> for HealthCheckPage {
    fn from(container: &model::Container) -> Self {
        glib::Object::builder()
            .property("container", container)
            .build()
    }
}

impl HealthCheckPage {
    fn container(&self) -> Option<model::Container> {
        self.imp().container.upgrade()
    }

    fn set_container(&self, value: Option<&model::Container>) {
        if self.container().as_ref() == value {
            return;
        }

        let imp = self.imp();

        if let Some(config) = value
            .and_then(model::Container::data)
            .and_then(model::ContainerData::health_config)
        {
            imp.command_row.set_value(
                &config
                    .test
                    .as_ref()
                    .map(|s| s.join(" "))
                    .unwrap_or_default(),
            );
            imp.interval_row.set_value(
                &config
                    .interval
                    .map(|nanos| {
                        let secs = nanos / 1000000000;
                        ngettext!("{} second", "{} seconds", secs as u32, secs)
                    })
                    .unwrap_or_default(),
            );
            imp.retries_row.set_value(
                &config
                    .retries
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            );
            imp.timeout_row.set_value(
                &config
                    .timeout
                    .map(|nanos| {
                        let secs = nanos / 1000000000;
                        ngettext!("{} second", "{} seconds", secs as u32, secs)
                    })
                    .unwrap_or_default(),
            );
        }

        imp.container.set(value);
        self.notify("container");
    }

    fn set_list_box_visibility(&self, model: &gio::ListModel) {
        self.imp().log_list_box.set_visible(model.n_items() > 0);
    }

    fn run_health_check(&self) {
        if let Some(container) = self.container().as_ref().and_then(model::Container::api) {
            utils::do_async(
                async move { container.healthcheck().await },
                clone!(@weak self as obj => move |result| if let Err(e) = result {
                    utils::show_error_toast(
                        obj.upcast_ref(),
                        &gettext("Error on running health check"),
                        &e.to_string()
                    );
                }),
            );
        }
    }
}
