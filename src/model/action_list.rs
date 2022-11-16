use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::path::Path;

use gtk::gio;
use gtk::glib;
use gtk::glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use indexmap::IndexMap;
use once_cell::sync::Lazy;

use crate::model;
use crate::podman;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub(crate) struct ActionList {
        pub(super) client: glib::WeakRef<model::Client>,
        pub(super) list: RefCell<IndexMap<u32, model::Action>>,
        pub(super) action_counter: Cell<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ActionList {
        const NAME: &'static str = "ActionList";
        type Type = super::ActionList;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ActionList {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::Client>("client")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    glib::ParamSpecUInt::builder("len")
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    glib::ParamSpecUInt::builder("ongoing")
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    glib::ParamSpecUInt::builder("finished")
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    glib::ParamSpecUInt::builder("cancelled")
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    glib::ParamSpecUInt::builder("failed")
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "client" => self.client.set(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = &*self.obj();
            match pspec.name() {
                "client" => obj.client().to_value(),
                "len" => obj.len().to_value(),
                "ongoing" => obj.ongoing().to_value(),
                "finished" => obj.finished().to_value(),
                "cancelled" => obj.cancelled().to_value(),
                "failed" => obj.failed().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.obj().connect_items_changed(|obj, _, _, _| {
                obj.notify("len");
                obj.notify_num_states();
            });
        }
    }

    impl ListModelImpl for ActionList {
        fn item_type(&self) -> glib::Type {
            model::Action::static_type()
        }

        fn n_items(&self) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.list
                .borrow()
                .get_index(position as usize)
                .map(|(_, obj)| obj.upcast_ref())
                .cloned()
        }
    }
}

glib::wrapper! {
    pub(crate) struct ActionList(ObjectSubclass<imp::ActionList>)
        @implements gio::ListModel;
}

impl From<Option<&model::Client>> for ActionList {
    fn from(client: Option<&model::Client>) -> Self {
        glib::Object::builder::<Self>()
            .property("client", &client)
            .build()
    }
}

impl ActionList {
    pub(crate) fn client(&self) -> Option<model::Client> {
        self.imp().client.upgrade()
    }

    pub(crate) fn len(&self) -> u32 {
        self.n_items()
    }

    pub(crate) fn ongoing(&self) -> u32 {
        self.count_state(model::ActionState::Ongoing)
    }

    pub(crate) fn finished(&self) -> u32 {
        self.count_state(model::ActionState::Finished)
    }

    pub(crate) fn cancelled(&self) -> u32 {
        self.count_state(model::ActionState::Cancelled)
    }

    pub(crate) fn failed(&self) -> u32 {
        self.count_state(model::ActionState::Failed)
    }

    fn count_state(&self, state: model::ActionState) -> u32 {
        self.imp()
            .list
            .borrow()
            .values()
            .filter(|action| action.state() == state)
            .count() as u32
    }

    pub(crate) fn get(&self, num: u32) -> Option<model::Action> {
        self.imp().list.borrow().get(&num).cloned()
    }

    pub(crate) fn remove(&self, num: u32) {
        let mut list = self.imp().list.borrow_mut();
        if let Some((idx, _, _)) = list.shift_remove_full(&num) {
            drop(list);
            self.items_changed(idx as u32, 1, 0);
        }
    }
    pub(crate) fn clean_up(&self) {
        let indexes = {
            let mut list = self.imp().list.borrow_mut();

            let indexes = list
                .values()
                .enumerate()
                .rev()
                .filter(|(_, action)| action.state() != model::ActionState::Ongoing)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();

            indexes.iter().for_each(|i| {
                list.shift_remove_index(*i);
            });

            indexes
        };

        indexes.into_iter().for_each(|pos| {
            self.items_changed(pos as u32, 1, 0);
        });
    }

    pub(crate) fn prune_images(&self, opts: podman::opts::ImagePruneOpts) -> model::Action {
        self.insert_action(model::Action::prune_images(
            self.imp().action_counter.get(),
            self.client().unwrap(),
            opts,
        ))
    }

    pub(crate) fn download_image(
        &self,
        image: &str,
        opts: podman::opts::PullOpts,
    ) -> model::Action {
        self.insert_action(model::Action::download_image(
            self.imp().action_counter.get(),
            image,
            self.client().unwrap(),
            opts,
        ))
    }

    pub(crate) fn build_image(
        &self,
        image: &str,
        opts: podman::opts::ImageBuildOpts,
    ) -> model::Action {
        self.insert_action(model::Action::build_image(
            self.imp().action_counter.get(),
            image,
            self.client().unwrap(),
            opts,
        ))
    }

    pub(crate) fn create_container(
        &self,
        container: &str,
        image: &str,
        opts: podman::opts::ContainerCreateOpts,
        run: bool,
    ) -> model::Action {
        self.insert_action(model::Action::create_container(
            self.imp().action_counter.get(),
            container,
            image,
            self.client().unwrap(),
            opts,
            run,
        ))
    }

    pub(crate) fn commit_container(
        &self,
        image: Option<&str>,
        container: &str,
        api: podman::api::Container,
        opts: podman::opts::ContainerCommitOpts,
    ) -> model::Action {
        self.insert_action(model::Action::commit_container(
            self.imp().action_counter.get(),
            image,
            container,
            api,
            opts,
        ))
    }

    pub(crate) fn create_container_download_image(
        &self,
        container: &str,
        image: &str,
        pull_opts: podman::opts::PullOpts,
        create_opts_builder: podman::opts::ContainerCreateOptsBuilder,
        run: bool,
    ) -> model::Action {
        self.insert_action(model::Action::create_container_download_image(
            self.imp().action_counter.get(),
            container,
            image,
            self.client().unwrap(),
            pull_opts,
            create_opts_builder,
            run,
        ))
    }

    pub(crate) fn copy_files_into_container(
        &self,
        host_path: impl AsRef<Path> + fmt::Display + Send + Sync + 'static,
        container_path: impl AsRef<Path> + fmt::Display + Send + Sync + 'static,
        directory: bool,
        container: &model::Container,
    ) -> model::Action {
        self.insert_action(model::Action::copy_files_into_container(
            self.imp().action_counter.get(),
            host_path,
            container_path,
            directory,
            container,
        ))
    }

    pub(crate) fn copy_files_from_container(
        &self,
        container: &model::Container,
        container_path: impl AsRef<Path> + fmt::Display + Send + Sync + 'static,
        host_path: impl AsRef<Path> + fmt::Display + Clone + Send + Sync + 'static,
    ) -> model::Action {
        self.insert_action(model::Action::copy_files_from_container(
            self.imp().action_counter.get(),
            container,
            container_path,
            host_path,
        ))
    }

    pub(crate) fn play_kubernetes_yaml(
        &self,
        kube_file_path: impl AsRef<Path> + Into<String> + Send + 'static,
        podman: podman::Podman,
        opts: podman::opts::PlayKubernetesYamlOpts,
    ) -> model::Action {
        self.insert_action(model::Action::play_kubernetes_yaml(
            self.imp().action_counter.get(),
            kube_file_path,
            podman,
            opts,
        ))
    }

    pub(crate) fn create_pod(&self, pod: &str, opts: podman::opts::PodCreateOpts) -> model::Action {
        self.insert_action(model::Action::create_pod(
            self.imp().action_counter.get(),
            pod,
            self.client().unwrap(),
            opts,
        ))
    }

    pub(crate) fn create_pod_download_infra(
        &self,
        pod: &str,
        image: &str,
        pull_opts: podman::opts::PullOpts,
        create_opts_builder: podman::opts::PodCreateOptsBuilder,
    ) -> model::Action {
        self.insert_action(model::Action::create_pod_download_infra(
            self.imp().action_counter.get(),
            pod,
            image,
            self.client().unwrap(),
            pull_opts,
            create_opts_builder,
        ))
    }

    fn insert_action(&self, action: model::Action) -> model::Action {
        let imp = self.imp();

        let position = {
            let mut list = imp.list.borrow_mut();
            list.insert(imp.action_counter.replace(action.num() + 1), action.clone());
            list.len() - 1
        };

        action.connect_notify_local(
            Some("state"),
            clone!(@weak self as obj => move |_, _| obj.notify_num_states()),
        );

        self.items_changed(position as u32, 0, 1);

        action
    }

    fn notify_num_states(&self) {
        self.notify("ongoing");
        self.notify("finished");
        self.notify("cancelled");
        self.notify("failed");
    }
}
