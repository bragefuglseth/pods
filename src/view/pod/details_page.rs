use std::cell::RefCell;

use adw::traits::BinExt;
use gettextrs::gettext;
use glib::clone;
use glib::closure;
use glib::Properties;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::utils;
use crate::view;

const ACTION_START_OR_RESUME: &str = "pod-details-page.start";
const ACTION_STOP: &str = "pod-details-page.stop";
const ACTION_KILL: &str = "pod-details-page.kill";
const ACTION_RESTART: &str = "pod-details-page.restart";
const ACTION_PAUSE: &str = "pod-details-page.pause";
const ACTION_RESUME: &str = "pod-details-page.resume";
const ACTION_DELETE: &str = "pod-details-page.delete";
const ACTION_INSPECT_POD: &str = "pod-details-page.inspect-pod";
const ACTION_GENERATE_KUBE: &str = "pod-details-page.generate-kube";
const ACTION_SHOW_PROCESSES: &str = "pod-details-page.show-processes";

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::DetailsPage)]
    #[template(resource = "/com/github/marhkb/Pods/ui/pod/details-page.ui")]
    pub(crate) struct DetailsPage {
        pub(super) handler_id: RefCell<Option<glib::SignalHandlerId>>,
        #[property(get, set = Self::set_pod, construct, explicit_notify, nullable)]
        pub(super) pod: glib::WeakRef<model::Pod>,
        #[template_child]
        pub(super) back_navigation_controls: TemplateChild<view::BackNavigationControls>,
        #[template_child]
        pub(super) window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(super) inspection_spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub(super) action_row: TemplateChild<adw::PreferencesRow>,
        #[template_child]
        pub(super) start_or_resume_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) stop_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) spinning_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) id_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) created_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) hostname_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) leaflet_overlay: TemplateChild<view::LeafletOverlay>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DetailsPage {
        const NAME: &'static str = "PdsPodDetailsPage";
        type Type = super::DetailsPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action(ACTION_START_OR_RESUME, None, |widget, _, _| {
                if widget.pod().map(|pod| pod.can_start()).unwrap_or(false) {
                    super::super::start(widget.upcast_ref());
                } else {
                    super::super::resume(widget.upcast_ref());
                }
            });
            klass.install_action(ACTION_STOP, None, |widget, _, _| {
                super::super::stop(widget.upcast_ref());
            });
            klass.install_action(ACTION_KILL, None, |widget, _, _| {
                super::super::kill(widget.upcast_ref());
            });
            klass.install_action(ACTION_RESTART, None, |widget, _, _| {
                super::super::restart(widget.upcast_ref());
            });
            klass.install_action(ACTION_PAUSE, None, |widget, _, _| {
                super::super::pause(widget.upcast_ref());
            });
            klass.install_action(ACTION_RESUME, None, |widget, _, _| {
                super::super::resume(widget.upcast_ref());
            });
            klass.install_action(ACTION_DELETE, None, |widget, _, _| {
                super::super::delete(widget.upcast_ref());
            });

            klass.install_action(ACTION_INSPECT_POD, None, |widget, _, _| {
                widget.show_inspection();
            });
            klass.install_action(ACTION_GENERATE_KUBE, None, |widget, _, _| {
                widget.show_kube();
            });
            klass.install_action(ACTION_SHOW_PROCESSES, None, |widget, _, _| {
                widget.show_processes();
            });

            // For displaying a mnemonic.
            klass.add_binding_action(
                gdk::Key::N,
                gdk::ModifierType::CONTROL_MASK,
                view::ContainersGroup::action_create_container(),
                None,
            );
            klass.install_action(
                view::ContainersGroup::action_create_container(),
                None,
                move |widget, _, _| {
                    widget.create_container();
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DetailsPage {
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

            let pod_expr = Self::Type::this_expression("pod");
            let data_expr = pod_expr.chain_property::<model::Pod>("data");
            let status_expr = pod_expr.chain_property::<model::Pod>("status");
            let hostname_expr = data_expr.chain_property::<model::PodData>("hostname");

            data_expr
                .chain_closure::<bool>(closure!(|_: Self::Type, cmd: Option<model::PodData>| {
                    cmd.is_none()
                }))
                .bind(&*self.inspection_spinner, "visible", Some(obj));

            pod_expr
                .chain_property::<model::Pod>("id")
                .chain_closure::<String>(closure!(|_: Self::Type, id: &str| utils::format_id(id)))
                .bind(&*self.id_row, "value", Some(obj));

            gtk::ClosureExpression::new::<String>(
                &[
                    Self::Type::this_expression("root")
                        .chain_property::<gtk::Window>("application")
                        .chain_property::<crate::Application>("ticks"),
                    pod_expr.chain_property::<model::Pod>("created"),
                ],
                closure!(|_: Self::Type, _ticks: u64, created: i64| {
                    utils::format_ago(utils::timespan_now(created))
                }),
            )
            .bind(&*self.created_row, "value", Some(obj));

            status_expr
                .chain_closure::<String>(closure!(|_: Self::Type, status: model::PodStatus| {
                    status.to_string()
                }))
                .bind(&*self.status_label, "label", Some(obj));

            let css_classes = utils::css_classes(self.status_label.upcast_ref());
            status_expr
                .chain_closure::<Vec<String>>(closure!(
                    |_: Self::Type, status: model::PodStatus| {
                        css_classes
                            .iter()
                            .cloned()
                            .chain(Some(String::from(super::super::pod_status_css_class(
                                status,
                            ))))
                            .collect::<Vec<_>>()
                    }
                ))
                .bind(&*self.status_label, "css-classes", Some(obj));

            hostname_expr.bind(&*self.hostname_row, "value", Some(obj));
            hostname_expr
                .chain_closure::<bool>(closure!(
                    |_: Self::Type, hostname: String| !hostname.is_empty()
                ))
                .bind(&*self.hostname_row, "visible", Some(obj));

            status_expr.watch(Some(obj), clone!(@weak obj => move || obj.update_actions()));
            pod_expr
                .chain_property::<model::Pod>("action-ongoing")
                .watch(Some(obj), clone!(@weak obj => move || obj.update_actions()));
        }

        fn dispose(&self) {
            utils::ChildIter::from(self.obj().upcast_ref()).for_each(|child| child.unparent());
        }
    }

    impl WidgetImpl for DetailsPage {}

    impl DetailsPage {
        pub(super) fn set_pod(&self, value: Option<&model::Pod>) {
            let obj = &*self.obj();
            if obj.pod().as_ref() == value {
                return;
            }

            self.window_title.set_subtitle("");
            if let Some(pod) = obj.pod() {
                pod.disconnect(self.handler_id.take().unwrap());
            }

            if let Some(pod) = value {
                self.window_title.set_subtitle(&pod.name());
                pod.inspect(clone!(@weak obj => move |result| if let Err(e) = result {
                    utils::show_error_toast(obj.upcast_ref(), &gettext("Error on loading pod data"), &e.to_string());
                }));

                let handler_id = pod.connect_deleted(clone!(@weak obj => move |pod| {
                    utils::show_toast(obj.upcast_ref(), gettext!("Pod '{}' has been deleted", pod.name()));
                    obj.imp().back_navigation_controls.navigate_back();
                }));
                self.handler_id.replace(Some(handler_id));
            }

            self.pod.set(value);
            obj.notify("pod");
        }
    }
}

glib::wrapper! {
    pub(crate) struct DetailsPage(ObjectSubclass<imp::DetailsPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::Pod> for DetailsPage {
    fn from(pod: &model::Pod) -> Self {
        glib::Object::builder().property("pod", pod).build()
    }
}

impl DetailsPage {
    fn update_actions(&self) {
        if let Some(pod) = self.pod() {
            let imp = self.imp();

            imp.action_row.set_sensitive(!pod.action_ongoing());

            let can_start_or_resume = pod.can_start() || pod.can_resume();
            let can_stop = pod.can_stop();

            imp.start_or_resume_button
                .set_visible(!pod.action_ongoing() && can_start_or_resume);
            imp.stop_button
                .set_visible(!pod.action_ongoing() && can_stop);
            imp.spinning_button.set_visible(
                pod.action_ongoing()
                    || (!imp.start_or_resume_button.is_visible() && !imp.stop_button.is_visible()),
            );

            self.action_set_enabled(ACTION_START_OR_RESUME, can_start_or_resume);
            self.action_set_enabled(ACTION_STOP, can_stop);
            self.action_set_enabled(ACTION_KILL, pod.can_kill());
            self.action_set_enabled(ACTION_RESTART, pod.can_restart());
            self.action_set_enabled(ACTION_PAUSE, pod.can_pause());
            self.action_set_enabled(ACTION_DELETE, pod.can_delete());
        }
    }

    fn show_inspection(&self) {
        self.show_kube_inspection_or_kube(view::ScalableTextViewMode::Inspect);
    }

    fn show_kube(&self) {
        self.show_kube_inspection_or_kube(view::ScalableTextViewMode::Kube);
    }

    fn show_kube_inspection_or_kube(&self, mode: view::ScalableTextViewMode) {
        self.exec_action(|| {
            if let Some(pod) = self.pod() {
                let weak_ref = glib::WeakRef::new();
                weak_ref.set(Some(&pod));

                self.imp().leaflet_overlay.show_details(
                    view::ScalableTextViewPage::from(view::Entity::Pod {
                        pod: weak_ref,
                        mode,
                    })
                    .upcast_ref(),
                );
            }
        });
    }

    fn show_processes(&self) {
        self.exec_action(|| {
            if let Some(pod) = self.pod() {
                self.imp()
                    .leaflet_overlay
                    .show_details(view::TopPage::from(&pod).upcast_ref());
            }
        });
    }

    fn create_container(&self) {
        self.exec_action(|| {
            super::create_container(self.upcast_ref(), self.pod());
        });
    }

    fn exec_action<F: Fn()>(&self, op: F) {
        if self.imp().leaflet_overlay.child().is_none() {
            op();
        }
    }
}
