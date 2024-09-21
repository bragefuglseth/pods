use adw::prelude::*;
use gettextrs::gettext;
use glib::clone;
use gtk::gio;
use gtk::glib;

use crate::model;
use crate::utils;
use crate::view;

pub(crate) fn pod_status_css_class(status: model::PodStatus) -> &'static str {
    use model::PodStatus::*;

    match status {
        Created => "pod-status-created",
        Dead => "pod-status-dead",
        Degraded => "pod-status-degraded",
        Error => "pod-status-error",
        Exited => "pod-status-exited",
        Paused => "pod-status-paused",
        Restarting => "pod-status-restarting",
        Running => "pod-status-running",
        Stopped => "pod-status-stopped",
        Unknown => "pod-status-unknown",
    }
}

macro_rules! pod_action {
    (fn $name:ident => $action:ident($($param:literal),*) => $error:tt) => {
        pub(crate) fn $name(widget: &gtk::Widget) {
            use gtk::glib;

            if let Some(pod) = <gtk::Widget as gtk::prelude::ObjectExt>::property::<Option<crate::model::Pod>>(widget, "pod") {
                pod.$action(
                    $($param,)*
                    glib::clone!(@weak widget => move |result| if let Err(e) = result {
                        crate::utils::show_error_toast(&widget, &$error, &e.to_string());
                    }),
                );
            }
        }
    };
}

pod_action!(fn start => start() => { gettextrs::gettext("Error on starting pod") });
pod_action!(fn stop => stop(false) => { gettextrs::gettext("Error on stopping pod") });
pod_action!(fn kill => stop(true) => { gettextrs::gettext("Error on killing pod") });
pod_action!(fn restart => restart(false) => { gettextrs::gettext("Error on restarting pod") });
pod_action!(fn pause => pause() => { gettextrs::gettext("Error on pausing pod") });
pod_action!(fn resume => resume() => { gettextrs::gettext("Error on resuming pod") });
pod_action!(fn delete => delete(false) => { gettextrs::gettext("Error on deleting pod") });

pub(crate) fn show_delete_confirmation_dialog(widget: &gtk::Widget) {
    if let Some(pod) =
        <gtk::Widget as gtk::prelude::ObjectExt>::property::<Option<model::Pod>>(widget, "pod")
    {
        let first_container = pod.container_list().get(0);

        if pod.num_containers() > 0 || first_container.is_some() {
            let dialog = adw::MessageDialog::builder()
                .heading(gettext("Confirm Forced Pod Deletion"))
                .body_use_markup(true)
                .body(
                    match first_container.as_ref().map(|c| c.name()) {
                        Some(id) => gettext!(
                            // Translators: The "{}" is a placeholder for the pod name.
                            "Pod contains container <b>{}</b>. Deleting the pod will also delete all its containers.",
                            id
                        ),
                        None => gettext(
                           "Pod contains a container. Deleting the pod will also delete all its containers.",
                       ),
                    }

                )
                .modal(true)
                .transient_for(&utils::root(widget))
                .build();

            dialog.add_responses(&[
                ("cancel", &gettext("_Cancel")),
                ("delete", &gettext("_Force Delete")),
            ]);
            dialog.set_default_response(Some("cancel"));
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

            dialog.choose(
                gio::Cancellable::NONE,
                clone!(@weak widget, @weak pod => move |response| {
                    if response == "delete" {
                        delete(&widget);
                    }
                }),
            );
        } else {
            delete(widget);
        }
    }
}

pub(crate) fn create_container(widget: &gtk::Widget, pod: Option<model::Pod>) {
    if let Some(pod) = pod {
        utils::show_dialog(widget, view::ContainerCreationPage::from(&pod).upcast_ref());
    }
}
