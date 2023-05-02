use std::cell::Cell;

use adw::subclass::prelude::*;
use adw::traits::ActionRowExt;
use adw::traits::BinExt;
use ashpd::desktop::file_chooser::OpenFileRequest;
use ashpd::WindowIdentifier;
use gettextrs::gettext;
use glib::clone;
use glib::Properties;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::utils;
use crate::view;

const ACTION_SELECT_HOST_FILE: &str = "container-files-put-page.select-host-file";

const ACTION_SELECT_HOST_DIRECTORY: &str = "container-files-put-page.select-host-directory";
const ACTION_PUT: &str = "container-files-put-page.put";

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::FilesPutPage)]
    #[template(resource = "/com/github/marhkb/Pods/ui/container/files-put-page.ui")]
    pub(crate) struct FilesPutPage {
        pub(super) directory: Cell<bool>,
        #[property(get, set = Self::set_container, construct, explicit_notify, nullable)]
        pub(super) container: glib::WeakRef<model::Container>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) put_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) host_path_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) container_path_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) action_page_bin: TemplateChild<adw::Bin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FilesPutPage {
        const NAME: &'static str = "PdsContainerFilesPutPage";
        type Type = super::FilesPutPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action_async(ACTION_SELECT_HOST_FILE, None, |widget, _, _| async move {
                widget.select_file(false).await;
            });
            klass.install_action_async(
                ACTION_SELECT_HOST_DIRECTORY,
                None,
                |widget, _, _| async move {
                    widget.select_file(true).await;
                },
            );
            klass.install_action_async(ACTION_PUT, None, |widget, _, _| async move {
                widget.put().await;
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for FilesPutPage {
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

            let obj = self.obj();

            obj.action_set_enabled(ACTION_PUT, false);
            self.host_path_row
                .connect_subtitle_notify(clone!(@weak obj => move |row| {
                    obj.action_set_enabled(ACTION_PUT, row.subtitle().is_some());
                }));
        }

        fn dispose(&self) {
            utils::unparent_children(self.obj().upcast_ref());
        }
    }

    impl WidgetImpl for FilesPutPage {
        fn root(&self) {
            self.parent_root();

            let widget = &*self.obj();

            glib::idle_add_local(
                clone!(@weak widget => @default-return glib::Continue(false), move || {
                    widget.imp().container_path_row.grab_focus();
                    glib::Continue(false)
                }),
            );
            utils::root(widget.upcast_ref()).set_default_widget(Some(&*self.put_button));
        }

        fn unroot(&self) {
            utils::root(self.obj().upcast_ref()).set_default_widget(gtk::Widget::NONE);
            self.parent_unroot()
        }
    }

    impl FilesPutPage {
        pub(super) fn set_container(&self, value: Option<&model::Container>) {
            let obj = &*self.obj();
            if obj.container().as_ref() == value {
                return;
            }

            if let Some(container) = value {
                container.connect_deleted(clone!(@weak obj => move |_| {
                    obj.activate_action("action.cancel", None).unwrap();
                }));
            }

            self.container.set(value);
            obj.notify("container");
        }
    }
}

glib::wrapper! {
    pub(crate) struct FilesPutPage(ObjectSubclass<imp::FilesPutPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::Container> for FilesPutPage {
    fn from(container: &model::Container) -> Self {
        glib::Object::builder()
            .property("container", container)
            .build()
    }
}

impl FilesPutPage {
    async fn select_file(&self, directory: bool) {
        let request = OpenFileRequest::default()
            .identifier(WindowIdentifier::from_native(&self.native().unwrap()).await)
            .title(
                if directory {
                    gettext("Select Host Directory")
                } else {
                    gettext("Select Host File")
                }
                .as_str(),
            )
            .directory(directory)
            .modal(true);

        utils::show_open_file_dialog(
            request,
            self.upcast_ref(),
            clone!(@weak self as obj => move |files| {
                let file = gio::File::for_uri(files.uris()[0].as_str());

                if let Some(path) = file.path() {
                    let imp = obj.imp();

                    imp.host_path_row.set_subtitle(path.to_str().unwrap());
                    imp.directory.set(directory);
                }
            }),
        )
        .await;
    }

    async fn put(&self) {
        if let Some(container) = self.container() {
            let imp = self.imp();

            let host_path = imp
                .host_path_row
                .subtitle()
                .unwrap_or_else(|| glib::GString::from("/"));
            let container_path = imp.container_path_row.text();

            let page = view::ActionPage::from(
                &container
                    .container_list()
                    .unwrap()
                    .client()
                    .unwrap()
                    .action_list()
                    .copy_files_into_container(
                        String::from(host_path),
                        if container_path.is_empty() {
                            String::from("/")
                        } else {
                            String::from(container_path)
                        },
                        imp.directory.get(),
                        &container,
                    ),
            );

            imp.action_page_bin.set_child(Some(&page));
            imp.stack.set_visible_child(&*imp.action_page_bin);
        }
    }
}
