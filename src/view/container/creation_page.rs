use std::cell::RefCell;

use adw::subclass::prelude::*;
use adw::traits::ActionRowExt;
use adw::traits::BinExt;
use adw::traits::ComboRowExt;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;
use gtk::glib::clone;
use gtk::glib::closure;
use gtk::glib::closure_local;
use gtk::pango;
use gtk::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;

use crate::model;
use crate::podman;
use crate::utils;
use crate::view;

const ACTION_SEARCH_IMAGE: &str = "container-creation-page.search-image";
const ACTION_REMOVE_REMOTE_IMAGE: &str = "container-creation-page.remove-remote-image";
const ACTION_ADD_CMD_ARG: &str = "container-creation-page.add-cmd-arg";
const ACTION_ADD_PORT_MAPPING: &str = "container-creation-page.add-port-mapping";
const ACTION_ADD_VOLUME: &str = "container-creation-page.add-volume";
const ACTION_ADD_ENV_VAR: &str = "container-creation-page.add-env-var";
const ACTION_ADD_LABEL: &str = "container-creation-page.add-label";
const ACTION_CREATE_AND_RUN: &str = "container-creation-page.create-and-run";
const ACTION_CREATE: &str = "container-creation-page.create";

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/github/marhkb/Pods/ui/container/creation-page.ui")]
    pub(crate) struct CreationPage {
        pub(super) client: glib::WeakRef<model::Client>,
        pub(super) image: glib::WeakRef<model::Image>,
        pub(super) pod: glib::WeakRef<model::Pod>,
        pub(super) cmd_args: gio::ListStore,
        pub(super) port_mappings: gio::ListStore,
        pub(super) volumes: gio::ListStore,
        pub(super) env_vars: gio::ListStore,
        pub(super) labels: gio::ListStore,
        pub(super) command_row_handler:
            RefCell<Option<(glib::SignalHandlerId, glib::WeakRef<model::Image>)>>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) leaflet_overlay: TemplateChild<view::LeafletOverlay>,
        #[template_child]
        pub(super) create_button: TemplateChild<adw::SplitButton>,
        #[template_child]
        pub(super) name_entry_row: TemplateChild<view::RandomNameEntryRow>,
        #[template_child]
        pub(super) local_image_property_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) local_image_combo_row: TemplateChild<view::ImageLocalComboRow>,
        #[template_child]
        pub(super) remote_image_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) pod_property_row: TemplateChild<view::PropertyRow>,
        #[template_child]
        pub(super) pod_combo_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) pull_latest_image_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) pull_latest_image_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) command_entry_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) command_arg_list_box: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) terminal_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) memory_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) mem_value: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub(super) mem_drop_down: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub(super) port_mapping_list_box: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) volume_list_box: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) env_var_list_box: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) labels_list_box: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) health_check_command_entry_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) health_check_interval_value: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub(super) health_check_timeout_value: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub(super) health_check_start_period_value: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub(super) health_check_retries_value: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub(super) action_page_bin: TemplateChild<adw::Bin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CreationPage {
        const NAME: &'static str = "PdsContainerCreationPage";
        type Type = super::CreationPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action(ACTION_SEARCH_IMAGE, None, |widget, _, _| {
                widget.search_image();
            });
            klass.install_action(ACTION_REMOVE_REMOTE_IMAGE, None, |widget, _, _| {
                widget.remove_remote();
            });
            klass.install_action(ACTION_ADD_CMD_ARG, None, |widget, _, _| {
                widget.add_cmd_arg();
            });

            klass.install_action(ACTION_ADD_PORT_MAPPING, None, |widget, _, _| {
                widget.add_port_mapping();
            });
            klass.install_action(ACTION_ADD_VOLUME, None, |widget, _, _| {
                widget.add_volume();
            });
            klass.install_action(ACTION_ADD_ENV_VAR, None, |widget, _, _| {
                widget.add_env_var();
            });

            klass.install_action(ACTION_ADD_LABEL, None, |widget, _, _| {
                widget.add_label();
            });
            klass.install_action(ACTION_CREATE_AND_RUN, None, |widget, _, _| {
                widget.finish(true);
            });
            klass.install_action(ACTION_CREATE, None, |widget, _, _| {
                widget.finish(false);
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CreationPage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::Client>("client")
                        .construct_only()
                        .build(),
                    glib::ParamSpecObject::builder::<model::Image>("image")
                        .construct_only()
                        .build(),
                    glib::ParamSpecObject::builder::<model::Pod>("pod")
                        .construct()
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "client" => self.client.set(value.get().unwrap()),
                "image" => self.image.set(value.get().unwrap()),
                "pod" => self.obj().set_pod(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = &*self.obj();
            match pspec.name() {
                "client" => obj.client().to_value(),
                "image" => obj.image().to_value(),
                "pod" => obj.pod().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            self.name_entry_row
                .connect_text_notify(clone!(@weak obj => move |_| obj.on_name_changed()));

            let image_tag_expr = model::Image::this_expression("repo-tags")
                .chain_closure::<String>(closure!(
                    |image: model::Image, repo_tags: model::RepoTagList| {
                        repo_tags
                            .get(0)
                            .as_ref()
                            .map(model::RepoTag::full)
                            .map(str::to_owned)
                            .unwrap_or_else(|| utils::format_id(image.id()))
                    }
                ));
            let pod_name_expr = model::Pod::this_expression("name");

            if let Some(image) = obj.image() {
                image.connect_deleted(
                    clone!(@weak obj => move |_| obj.activate_action("action.cancel", None).unwrap())
                );

                self.local_image_combo_row.set_visible(false);

                image_tag_expr.bind(&*self.local_image_property_row, "value", Some(&image));

                match image.data().map(model::ImageData::config) {
                    Some(config) => {
                        self.command_entry_row.set_text(config.cmd().unwrap_or(""));
                        obj.set_exposed_ports(config);
                    }
                    None => {
                        image.connect_notify_local(
                            Some("details"),
                            clone!(@weak obj => move |image, _| {
                                let config = image.data().unwrap().config();
                                obj.imp().command_entry_row.set_text(config.cmd().unwrap_or(""));
                                obj.set_exposed_ports(config);
                            }),
                        );
                    }
                }
            } else {
                self.local_image_property_row.set_visible(false);

                self.local_image_combo_row.set_client(obj.client().as_ref());
                self.local_image_combo_row.connect_selected_item_notify(
                    clone!(@weak obj => move |_| obj.update_command_row()),
                );
                obj.update_command_row();
            }

            if let Some(pod) = obj.pod() {
                pod.connect_deleted(
                    clone!(@weak obj => move |_| obj.activate_action("action.cancel", None).unwrap())
                );
                self.pod_combo_row.set_visible(false);
                pod_name_expr.bind(&*self.pod_property_row, "value", Some(&pod));
            } else {
                self.pod_property_row.set_visible(false);

                self.pod_combo_row.connect_selected_item_notify(
                    clone!(@weak obj => move |combo_row| {
                        obj.set_pod(
                            combo_row.selected_item().and_then(|o| o.downcast().ok()).as_ref(),
                        );
                    }),
                );

                let pod_name_expr = model::Pod::this_expression("name");

                self.pod_combo_row.set_expression(Some(&pod_name_expr));

                let list_factory = gtk::SignalListItemFactory::new();
                list_factory.connect_bind(clone!(@weak obj => move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();

                    if let Some(item) = list_item.item() {
                        let box_ = gtk::Box::builder().spacing(3).build();
                        let label = gtk::Label::builder().xalign(0.0).build();

                        if let Some(pod) = item.downcast_ref::<model::Pod>() {
                            pod_name_expr.bind(&label, "label", Some(pod));
                            label.set_max_width_chars(48);
                            label.set_wrap(true);
                            label.set_wrap_mode(pango::WrapMode::WordChar);
                        } else {
                            label.set_label(&format!("<i>{}</i>", gettext("disabled")));
                            label.set_use_markup(true);
                            label.add_css_class("dim-label");
                        };

                        let selected_icon = gtk::Image::builder()
                            .icon_name("object-select-symbolic")
                            .build();

                        adw::ComboRow::this_expression("selected-item")
                            .chain_closure::<bool>(closure_local!(
                                |_: adw::ComboRow, selected: Option<&glib::Object>| {
                                    selected == Some(&item)
                                }
                            ))
                            .bind(&selected_icon, "visible", Some(&*obj.imp().pod_combo_row));

                        box_.append(&label);
                        box_.append(&selected_icon);

                        list_item.set_child(Some(&box_));
                    }
                }));
                list_factory.connect_unbind(|_, list_item| {
                    list_item
                        .downcast_ref::<gtk::ListItem>()
                        .unwrap()
                        .set_child(gtk::Widget::NONE);
                });
                self.pod_combo_row.set_list_factory(Some(&list_factory));

                let pod_list_mode = gio::ListStore::new(gio::ListModel::static_type());
                pod_list_mode.append(&gtk::StringList::new(&[""]));
                pod_list_mode.append(&gtk::SortListModel::new(
                    Some(obj.client().unwrap().pod_list()),
                    Some(&gtk::StringSorter::new(Some(model::Pod::this_expression(
                        "name",
                    )))),
                ));

                self.pod_combo_row
                    .set_model(Some(&gtk::FlattenListModel::new(Some(&pod_list_mode))));
            }

            bind_model(
                &self.command_arg_list_box,
                &self.cmd_args,
                |item| {
                    view::ValueRow::new(item.downcast_ref().unwrap(), &gettext("Argument")).upcast()
                },
                ACTION_ADD_CMD_ARG,
            );

            bind_model(
                &self.port_mapping_list_box,
                &self.port_mappings,
                |item| {
                    view::PortMappingRow::from(item.downcast_ref::<model::PortMapping>().unwrap())
                        .upcast()
                },
                ACTION_ADD_PORT_MAPPING,
            );

            bind_model(
                &self.volume_list_box,
                &self.volumes,
                |item| {
                    view::VolumeRow::from(item.downcast_ref::<model::Volume>().unwrap()).upcast()
                },
                ACTION_ADD_VOLUME,
            );

            bind_model(
                &self.env_var_list_box,
                &self.env_vars,
                |item| {
                    view::KeyValRow::from(item.downcast_ref::<model::KeyVal>().unwrap()).upcast()
                },
                ACTION_ADD_ENV_VAR,
            );

            bind_model(
                &self.labels_list_box,
                &self.labels,
                |item| {
                    view::KeyValRow::from(item.downcast_ref::<model::KeyVal>().unwrap()).upcast()
                },
                ACTION_ADD_LABEL,
            );
        }

        fn dispose(&self) {
            utils::ChildIter::from(self.obj().upcast_ref()).for_each(|child| child.unparent());
        }
    }

    impl WidgetImpl for CreationPage {
        fn root(&self) {
            self.parent_root();

            let widget = &*self.obj();

            glib::idle_add_local(
                clone!(@weak widget => @default-return glib::Continue(false), move || {
                    widget.imp().name_entry_row.grab_focus();
                    glib::Continue(false)
                }),
            );
            utils::root(widget.upcast_ref()).set_default_widget(Some(&*self.create_button));
        }

        fn unroot(&self) {
            utils::root(self.obj().upcast_ref()).set_default_widget(gtk::Widget::NONE);
            self.parent_unroot()
        }
    }
}

glib::wrapper! {
    pub(crate) struct CreationPage(ObjectSubclass<imp::CreationPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::Image> for CreationPage {
    fn from(image: &model::Image) -> Self {
        glib::Object::builder().property("image", image).build()
    }
}

impl From<&model::Pod> for CreationPage {
    fn from(pod: &model::Pod) -> Self {
        glib::Object::builder().property("pod", pod).build()
    }
}

impl From<&model::Client> for CreationPage {
    fn from(client: &model::Client) -> Self {
        glib::Object::builder().property("client", client).build()
    }
}

impl CreationPage {
    fn client(&self) -> Option<model::Client> {
        self.imp()
            .client
            .upgrade()
            .or_else(|| {
                self.image()
                    .as_ref()
                    .and_then(model::Image::image_list)
                    .as_ref()
                    .and_then(model::ImageList::client)
            })
            .or_else(|| {
                self.pod()
                    .as_ref()
                    .and_then(model::Pod::pod_list)
                    .as_ref()
                    .and_then(model::PodList::client)
            })
    }

    fn image(&self) -> Option<model::Image> {
        self.imp().image.upgrade()
    }

    fn pod(&self) -> Option<model::Pod> {
        self.imp().pod.upgrade()
    }

    fn set_pod(&self, value: Option<&model::Pod>) {
        if self.pod().as_ref() == value {
            return;
        }
        self.imp().pod.set(value);
        self.notify("pod");
    }

    fn on_name_changed(&self) {
        let enabled = self.imp().name_entry_row.text().len() > 0;
        self.action_set_enabled(ACTION_CREATE_AND_RUN, enabled);
        self.action_set_enabled(ACTION_CREATE, enabled);
    }

    fn set_exposed_ports(&self, config: &model::ImageConfig) {
        let imp = self.imp();

        let exposed_ports = config.exposed_ports();
        for i in 0..exposed_ports.n_items() {
            let exposed = exposed_ports.string(i).unwrap();

            let port_mapping = add_port_mapping(&imp.port_mappings);
            imp.port_mapping_list_box.set_visible(true);

            let mut split = exposed.split_terminator('/');
            if let Some(port) = split.next() {
                match port.parse() {
                    Ok(port) => {
                        port_mapping.set_host_port(port);
                        port_mapping.set_container_port(port);
                    }
                    Err(e) => log::warn!("Error on parsing port: {e}"),
                }

                if let Some(protocol) = split.next() {
                    match protocol {
                        "tcp" => port_mapping.set_protocol(model::PortMappingProtocol::Tcp),
                        "udp" => port_mapping.set_protocol(model::PortMappingProtocol::Udp),
                        _ => log::warn!("Unknown protocol: {protocol}"),
                    }
                }
            }
        }
    }

    fn remove_remote(&self) {
        let imp = self.imp();
        imp.remote_image_row.set_subtitle("");
        imp.remote_image_row.set_visible(false);
        imp.local_image_combo_row.set_visible(true);
        imp.pull_latest_image_row.set_visible(true);

        self.update_command_row();
    }

    fn update_command_row(&self) {
        let imp = self.imp();

        match imp
            .local_image_combo_row
            .selected_item()
            .as_ref()
            .map(|item| item.downcast_ref::<model::Image>().unwrap())
        {
            Some(image) => match image.data() {
                Some(details) => imp
                    .command_entry_row
                    .set_text(details.config().cmd().unwrap_or("")),
                None => {
                    if let Some((handler, image)) = imp.command_row_handler.take() {
                        if let Some(image) = image.upgrade() {
                            image.disconnect(handler);
                        }
                    }
                    let handler = image.connect_notify_local(
                        Some("details"),
                        clone!(@weak self as obj => move |image, _| {
                            obj.imp().command_entry_row.set_text(
                                image.data().unwrap().config().cmd().unwrap_or("")
                            );
                        }),
                    );
                    let image_weak = glib::WeakRef::new();
                    image_weak.set(Some(image));
                    imp.command_row_handler.replace(Some((handler, image_weak)));

                    image.inspect(|_| {});
                }
            },
            None => imp.command_entry_row.set_text(""),
        }
    }

    fn search_image(&self) {
        if let Some(client) = self.client() {
            let image_selection_page = view::ImageSelectionPage::from(&client);
            image_selection_page.connect_image_selected(
                clone!(@weak self as obj => move |_, image| {
                    let imp = obj.imp();

                    imp.local_image_combo_row.set_visible(false);
                    imp.remote_image_row.set_visible(true);
                    imp.remote_image_row.set_subtitle(&image);
                    imp.pull_latest_image_row.set_visible(false);

                    imp.command_entry_row.set_text("");
                }),
            );
            self.imp()
                .leaflet_overlay
                .show_details(image_selection_page.upcast_ref());
        }
    }

    fn add_cmd_arg(&self) {
        add_value(&self.imp().cmd_args);
    }

    fn add_port_mapping(&self) {
        add_port_mapping(&self.imp().port_mappings);
    }

    fn add_volume(&self) {
        add_volume(&self.imp().volumes);
    }

    fn add_env_var(&self) {
        add_key_val(&self.imp().env_vars);
    }

    fn add_label(&self) {
        add_key_val(&self.imp().labels);
    }

    fn finish(&self, run: bool) {
        let imp = self.imp();

        if imp.remote_image_row.is_visible() {
            self.pull_and_create(imp.remote_image_row.subtitle().unwrap().as_str(), true, run);
        } else if let Some(image) = self.image().or_else(|| {
            imp.local_image_combo_row
                .selected_item()
                .map(|item| item.downcast().unwrap())
        }) {
            if imp.pull_latest_image_switch.is_active() {
                self.pull_and_create(image.repo_tags().get(0).unwrap().full(), false, run);
            } else {
                let page = view::ActionPage::from(
                    &self.client().unwrap().action_list().create_container(
                        imp.name_entry_row.text().as_str(),
                        &image
                            .repo_tags()
                            .get(0)
                            .as_ref()
                            .map(model::RepoTag::full)
                            .map(str::to_owned)
                            .unwrap_or_else(|| utils::format_id(image.id())),
                        self.create().image(image.id()).build(),
                        run,
                    ),
                );

                imp.action_page_bin.set_child(Some(&page));
                imp.stack.set_visible_child(&*imp.action_page_bin);
            }
        } else {
            log::error!("Error while starting container: no image selected");
            utils::show_error_toast(
                self.upcast_ref(),
                &gettext("Failed to create container"),
                &gettext("no image selected"),
            )
        }
    }

    fn pull_and_create(&self, reference: &str, remote: bool, run: bool) {
        let imp = self.imp();

        let pull_opts = podman::opts::PullOpts::builder()
            .reference(reference)
            .policy(if remote {
                podman::opts::PullPolicy::Always
            } else {
                podman::opts::PullPolicy::Newer
            })
            .build();

        let page = view::ActionPage::from(
            &self
                .client()
                .unwrap()
                .action_list()
                .create_container_download_image(
                    imp.name_entry_row.text().as_str(),
                    reference,
                    pull_opts,
                    self.create(),
                    run,
                ),
        );

        imp.action_page_bin.set_child(Some(&page));
        imp.stack.set_visible_child(&*imp.action_page_bin);
    }

    fn create(&self) -> podman::opts::ContainerCreateOptsBuilder {
        let imp = self.imp();

        let create_opts = podman::opts::ContainerCreateOpts::builder()
            .name(imp.name_entry_row.text().as_str())
            .pod(self.pod().as_ref().map(model::Pod::name))
            .terminal(imp.terminal_switch.is_active())
            .portmappings(
                imp.port_mappings
                    .iter::<glib::Object>()
                    .unwrap()
                    .map(|mapping| mapping.unwrap().downcast::<model::PortMapping>().unwrap())
                    .map(|port_mapping| podman::models::PortMapping {
                        container_port: Some(port_mapping.container_port() as u16),
                        host_ip: None,
                        host_port: Some(port_mapping.host_port() as u16),
                        protocol: Some(port_mapping.protocol().to_string()),
                        range: None,
                    }),
            )
            .mounts(
                imp.volumes
                    .iter::<glib::Object>()
                    .unwrap()
                    .map(|volume| volume.unwrap().downcast::<model::Volume>().unwrap())
                    .map(|volume| podman::models::ContainerMount {
                        destination: Some(volume.container_path()),
                        source: Some(volume.host_path()),
                        _type: Some("bind".to_owned()),
                        options: Some({
                            let mut options =
                                vec![if volume.writable() { "rw" } else { "ro" }.to_owned()];

                            let selinux = volume.selinux().to_string();
                            if !selinux.is_empty() {
                                options.push(selinux)
                            }

                            options
                        }),
                        uid_mappings: None,
                        gid_mappings: None,
                    }),
            )
            .env(
                imp.env_vars
                    .iter::<glib::Object>()
                    .unwrap()
                    .map(|entry| entry.unwrap().downcast::<model::KeyVal>().unwrap())
                    .map(|entry| (entry.key(), entry.value())),
            )
            .labels(
                imp.labels
                    .iter::<glib::Object>()
                    .unwrap()
                    .map(|entry| entry.unwrap().downcast::<model::KeyVal>().unwrap())
                    .map(|entry| (entry.key(), entry.value())),
            );

        let create_opts = if imp.memory_switch.is_active() {
            create_opts.resource_limits(podman::models::LinuxResources {
                block_io: None,
                cpu: None,
                devices: None,
                hugepage_limits: None,
                memory: Some(podman::models::LinuxMemory {
                    disable_oom_killer: None,
                    kernel: None,
                    kernel_tcp: None,
                    limit: Some(
                        imp.mem_value.value() as i64
                            * 1000_i64.pow(imp.mem_drop_down.selected() + 1),
                    ),
                    reservation: None,
                    swap: None,
                    swappiness: None,
                    use_hierarchy: None,
                }),
                network: None,
                pids: None,
                rdma: None,
                unified: None,
            })
        } else {
            create_opts
        };

        let cmd = imp.command_entry_row.text();
        let create_opts = if cmd.is_empty() {
            create_opts
        } else {
            let args = imp
                .cmd_args
                .iter::<glib::Object>()
                .unwrap()
                .map(|value| value.unwrap().downcast::<model::Value>().unwrap())
                .map(|value| value.value());
            let mut cmd = vec![cmd.to_string()];
            cmd.extend(args);
            create_opts.command(&cmd)
        };

        let healthcheck_cmd = imp.health_check_command_entry_row.text();

        if healthcheck_cmd.is_empty() {
            create_opts
        } else {
            create_opts.health_config(podman::models::Schema2HealthConfig {
                interval: Some(imp.health_check_interval_value.value() as i64 * 1_000_000_000),
                retries: Some(imp.health_check_retries_value.value() as i64),
                start_period: Some(
                    imp.health_check_start_period_value.value() as i64 * 1_000_000_000,
                ),
                test: Some(
                    healthcheck_cmd
                        .split(' ')
                        .map(str::to_string)
                        .collect::<Vec<_>>(),
                ),
                timeout: Some(imp.health_check_timeout_value.value() as i64 * 1_000_000_000),
            })
        }
    }
}

fn bind_model<F>(list_box: &gtk::ListBox, model: &gio::ListStore, widget_func: F, action_name: &str)
where
    F: Fn(&glib::Object) -> gtk::Widget + 'static,
{
    list_box.bind_model(Some(model), widget_func);
    list_box.append(
        &gtk::ListBoxRow::builder()
            .action_name(action_name)
            .selectable(false)
            .child(
                &gtk::Image::builder()
                    .icon_name("list-add-symbolic")
                    .margin_top(12)
                    .margin_bottom(12)
                    .build(),
            )
            .build(),
    );
}

fn add_port_mapping(model: &gio::ListStore) -> model::PortMapping {
    let port_mapping = model::PortMapping::default();

    port_mapping.connect_remove_request(clone!(@weak model => move |port_mapping| {
        if let Some(pos) = model.find(port_mapping) {
            model.remove(pos);
        }
    }));

    model.append(&port_mapping);

    port_mapping
}

fn add_volume(model: &gio::ListStore) {
    let volume = model::Volume::default();

    volume.connect_remove_request(clone!(@weak model => move |volume| {
        if let Some(pos) = model.find(volume) {
            model.remove(pos);
        }
    }));

    model.append(&volume);
}

fn add_value(model: &gio::ListStore) {
    let value = model::Value::default();

    value.connect_remove_request(clone!(@weak model => move |value| {
        if let Some(pos) = model.find(value) {
            model.remove(pos);
        }
    }));

    model.append(&value);
}

fn add_key_val(model: &gio::ListStore) {
    let entry = model::KeyVal::default();

    entry.connect_remove_request(clone!(@weak model => move |entry| {
        if let Some(pos) = model.find(entry) {
            model.remove(pos);
        }
    }));

    model.append(&entry);
}
