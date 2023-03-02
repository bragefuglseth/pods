use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::BufWriter;
use std::io::Write;
use std::mem;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use ashpd::desktop::file_chooser::Choice;
use ashpd::desktop::file_chooser::SaveFileRequest;
use ashpd::WindowIdentifier;
use futures::StreamExt;
use gettextrs::gettext;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::glib::clone;
use gtk::glib::closure;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;
use sourceview5::traits::BufferExt;
use sourceview5::traits::GutterRendererExt;
use sourceview5::traits::GutterRendererTextExt;
use sourceview5::traits::ViewExt;

use crate::model;
use crate::podman;
use crate::utils;
use crate::view;

const ACTION_SAVE_TO_FILE: &str = "container-log-page.save-to-file";
const ACTION_TOGGLE_SEARCH: &str = "container-log-page.toggle-search";
const ACTION_SCROLL_DOWN: &str = "container-log-page.scroll-down";
const ACTION_START_CONTAINER: &str = "container-log-page.start-container";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FetchLinesState {
    #[default]
    Waiting,
    Fetching,
    Finished,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/marhkb/Pods/ui/container/log-page.ui")]
    pub(crate) struct LogPage {
        pub(super) settings: utils::PodsSettings,
        pub(super) container: glib::WeakRef<model::Container>,
        pub(super) renderer_timestamps: OnceCell<sourceview5::GutterRendererText>,
        pub(super) log_timestamps: RefCell<VecDeque<String>>,
        pub(super) fetch_until: OnceCell<String>,
        pub(super) fetch_lines_state: Cell<FetchLinesState>,
        pub(super) fetched_lines: RefCell<VecDeque<Vec<u8>>>,
        pub(super) prev_adj: Cell<f64>,
        pub(super) is_auto_scrolling: Cell<bool>,
        pub(super) sticky: Cell<bool>,
        #[template_child]
        pub(super) show_timestamps_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) save_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_widget: TemplateChild<view::SourceViewSearchWidget>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) lines_loading_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) source_view: TemplateChild<sourceview5::View>,
        #[template_child]
        pub(super) source_buffer: TemplateChild<sourceview5::Buffer>,
        #[template_child]
        pub(super) info_bar: TemplateChild<gtk::InfoBar>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LogPage {
        const NAME: &'static str = "PdsContainerLogPage";
        type Type = super::LogPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action_async(ACTION_SAVE_TO_FILE, None, |widget, _, _| async move {
                widget.save_to_file().await;
            });
            klass.install_action(ACTION_TOGGLE_SEARCH, None, |widget, _, _| {
                widget.toggle_search();
            });
            klass.install_action(ACTION_SCROLL_DOWN, None, |widget, _, _| {
                widget.scroll_down();
            });
            klass.install_action(ACTION_START_CONTAINER, None, |widget, _, _| {
                widget.start_or_resume_container();
            });

            klass.add_binding_action(
                gdk::Key::F,
                gdk::ModifierType::CONTROL_MASK,
                ACTION_TOGGLE_SEARCH,
                None,
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LogPage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::Container>("container")
                        .construct_only()
                        .build(),
                    glib::ParamSpecBoolean::builder("sticky")
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "container" => self.container.set(value.get().unwrap()),
                "sticky" => self.obj().set_sticky(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = &*self.obj();
            match pspec.name() {
                "container" => obj.container().to_value(),
                "sticky" => obj.sticky().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            let adw_style_manager = adw::StyleManager::default();
            obj.on_notify_dark(&adw_style_manager);
            adw_style_manager.connect_dark_notify(clone!(@weak obj => move |style_manager| {
                obj.on_notify_dark(style_manager);
            }));

            let renderer_timestamps = sourceview5::GutterRendererText::builder()
                .margin_end(6)
                .build();
            renderer_timestamps.connect_query_data(clone!(@weak obj => move |renderer, _, line| {
                let log_timestamps = obj.imp().log_timestamps.borrow();
                if let Some(timestamp) = log_timestamps.get(line as usize) {
                    let date_time = format!(
                        "<span foreground=\"#865e3c\">{timestamp}</span>",
                    );
                    renderer.set_markup(&date_time);

                    let (width, _) = renderer.measure_markup(&date_time);
                    renderer.set_width_request(width.max(renderer.width_request()));
                }
            }));
            self.source_buffer.connect_cursor_moved(
                clone!(@weak renderer_timestamps => move |_| renderer_timestamps.queue_draw()),
            );
            <sourceview5::View as ViewExt>::gutter(&*self.source_view, gtk::TextWindowType::Left)
                .insert(&renderer_timestamps, 0);
            self.renderer_timestamps.set(renderer_timestamps).unwrap();

            let mut maybe_gutter_child = <sourceview5::View as ViewExt>::gutter(
                &*self.source_view,
                gtk::TextWindowType::Left,
            )
            .first_child();

            while let Some(child) = maybe_gutter_child {
                if child.is::<sourceview5::GutterRenderer>() {
                    child.set_margin_start(4);
                }

                maybe_gutter_child = child.next_sibling()
            }

            self.search_bar.connect_search_mode_enabled_notify(
                clone!(@weak obj => move |search_bar| {
                    let search_entry = &*obj.imp().search_widget;
                    if search_bar.is_search_mode() {
                        search_entry.grab_focus();
                    } else {
                        search_entry.set_text("");
                    }
                }),
            );

            self.show_timestamps_button
                .bind_property("active", self.renderer_timestamps.get().unwrap(), "visible")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();

            self.settings
                .bind(
                    "show-log-timestamps",
                    self.renderer_timestamps.get().unwrap(),
                    "visible",
                )
                .build();

            self.search_button
                .bind_property("active", &*self.search_bar, "search-mode-enabled")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();

            self.search_widget.set_source_view(Some(&*self.source_view));

            let adj = self.scrolled_window.vadjustment();
            obj.on_adjustment_changed(&adj);
            adj.connect_value_changed(clone!(@weak obj => move |adj| {
                obj.on_adjustment_changed(adj);
            }));

            adj.connect_upper_notify(clone!(@weak obj => move |_| {
                if obj.sticky() || obj.imp().is_auto_scrolling.get() {
                    obj.scroll_down();
                }
            }));

            Self::Type::this_expression("container")
                .chain_property::<model::Container>("status")
                .chain_closure::<bool>(closure!(|_: Self::Type, status: model::ContainerStatus| {
                    status != model::ContainerStatus::Running
                }))
                .bind(&*self.info_bar, "revealed", Some(obj));

            if let Some(container) = obj.container() {
                container.connect_notify_local(
                    Some("status"),
                    clone!(@weak obj => move |container, _| {
                        if container.status() == model::ContainerStatus::Running {
                            obj.follow_log();
                        }
                    }),
                );
            }

            obj.init_log();
        }

        fn dispose(&self) {
            utils::ChildIter::from(self.obj().upcast_ref()).for_each(|child| child.unparent());
        }
    }

    impl WidgetImpl for LogPage {}
}

glib::wrapper! {
    pub(crate) struct LogPage(ObjectSubclass<imp::LogPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::Container> for LogPage {
    fn from(container: &model::Container) -> Self {
        glib::Object::builder()
            .property("container", container)
            .build()
    }
}

impl LogPage {
    fn container(&self) -> Option<model::Container> {
        self.imp().container.upgrade()
    }

    fn sticky(&self) -> bool {
        self.imp().sticky.get()
    }

    fn set_sticky(&self, sticky: bool) {
        if self.sticky() == sticky {
            return;
        }

        self.imp().sticky.set(sticky);
        self.notify("sticky");
    }

    fn scroll_down(&self) {
        let imp = self.imp();

        imp.is_auto_scrolling.set(true);
        imp.scrolled_window
            .emit_scroll_child(gtk::ScrollType::End, false);
    }

    fn on_adjustment_changed(&self, adj: &gtk::Adjustment) {
        let imp = self.imp();

        if imp.is_auto_scrolling.get() {
            if adj.value() + adj.page_size() >= adj.upper() {
                imp.is_auto_scrolling.set(false);
                self.set_sticky(true);
            }
        } else {
            self.set_sticky(adj.value() + adj.page_size() >= adj.upper());
            self.load_previous_messages(adj);
        }

        imp.prev_adj.replace(adj.value());
    }

    fn init_log(&self) {
        if let Some(container) = self.container().as_ref().and_then(model::Container::api) {
            let mut perform = MarkupPerform::default();

            utils::run_stream_with_finish_handler(
                container,
                move |container| {
                    container
                        .logs(&basic_opts_builder(false, true).tail("512").build())
                        .boxed()
                },
                clone!(@weak self as obj => @default-return glib::Continue(false), move |result| {
                    obj.imp().stack.set_visible_child_name("loaded");
                    obj.append_line(result, &mut perform)
                }),
                clone!(@weak self as obj => move || {
                    obj.imp().stack.set_visible_child_name("loaded");
                    obj.follow_log();
                }),
            );
        }
    }

    fn follow_log(&self) {
        if let Some(container) = self.container().as_ref().and_then(model::Container::api) {
            let timestamps = self.imp().log_timestamps.borrow();
            let mut iter = timestamps.iter().rev();

            let opts = basic_opts_builder(true, true);
            let (opts, skip) = match iter.next() {
                Some(last) => (
                    opts.since(
                        glib::DateTime::from_iso8601(last, None)
                            .unwrap()
                            .to_unix()
                            .to_string(),
                    ),
                    AtomicUsize::new(iter.take_while(|t| *t == last).count() + 1),
                ),
                None => (opts, AtomicUsize::new(0)),
            };

            let mut perform = MarkupPerform::default();

            utils::run_stream(
                container,
                move |container| container.logs(&opts.build()).boxed(),
                clone!(@weak self as obj => @default-return glib::Continue(false), move |result: podman::Result<podman::conn::TtyChunk>| {
                    if skip.load(Ordering::Relaxed) == 0 {
                        obj.append_line(result, &mut perform)
                    } else {
                        skip.fetch_sub(1, Ordering::Relaxed);
                        glib::Continue(true)
                    }
                }),
            );
        }
    }

    fn append_line(
        &self,
        result: podman::Result<podman::conn::TtyChunk>,
        perform: &mut MarkupPerform,
    ) -> glib::Continue {
        glib::Continue(match result {
            Ok(line) => {
                self.insert(Vec::from(line), perform, true);
                true
            }
            Err(e) => {
                log::warn!("Stopping container log stream due to error: {e}");
                utils::show_error_toast(
                    self.upcast_ref(),
                    &gettext("Error while following log"),
                    &e.to_string(),
                );
                false
            }
        })
    }

    fn insert(&self, line: Vec<u8>, perform: &mut MarkupPerform, at_end: bool) {
        let imp = self.imp();

        let line_buffer = perform.decode(&line);
        let (timestamp, log_message) = line_buffer.split_once(' ').unwrap();

        imp.fetch_until.get_or_init(|| timestamp.to_owned());

        let source_buffer = &*imp.source_buffer;
        source_buffer.insert_markup(
            &mut if at_end {
                imp.source_buffer.end_iter()
            } else {
                imp.source_buffer.start_iter()
            },
            &if source_buffer.start_iter() == source_buffer.end_iter() {
                Cow::Borrowed(log_message)
            } else if at_end {
                Cow::Owned(format!("\n{log_message}"))
            } else {
                Cow::Owned(format!("{log_message}\n"))
            },
        );

        let mut timestamps = imp.log_timestamps.borrow_mut();
        if at_end {
            timestamps.push_back(timestamp.to_owned());
        } else {
            timestamps.push_front(timestamp.to_owned());
        }
    }

    fn load_previous_messages(&self, adj: &gtk::Adjustment) {
        let imp = self.imp();

        if adj.value() >= imp.prev_adj.get() || adj.value() >= adj.page_size() {
            return;
        }

        match imp.fetch_lines_state.get() {
            FetchLinesState::Waiting => {
                if let Some(until) = imp.fetch_until.get().map(ToOwned::to_owned) {
                    if let Some(container) =
                        self.container().as_ref().and_then(model::Container::api)
                    {
                        imp.lines_loading_revealer.set_reveal_child(true);

                        utils::run_stream_with_finish_handler(
                            container,
                            move |container| {
                                container
                                    .logs(&basic_opts_builder(false, true).until(until).build())
                                    .boxed()
                            },
                            clone!(@weak self as obj => @default-return glib::Continue(false), move |result| {
                                let imp = obj.imp();
                                imp.fetch_lines_state.set(FetchLinesState::Fetching);

                                glib::Continue(match result {
                                    Ok(line) => {
                                        imp.fetched_lines.borrow_mut().push_back(Vec::from(line));
                                        true
                                    }
                                    Err(e) => {
                                        log::warn!("Stopping container log stream due to error: {e}");
                                        false
                                    }
                                })
                            }),
                            clone!(@weak self as obj => move || {
                                let imp = obj.imp();
                                imp.lines_loading_revealer.set_reveal_child(false);
                                imp.fetch_lines_state.set(FetchLinesState::Finished);

                                obj.move_lines_to_buffer();
                            }),
                        );
                    }
                }
            }
            FetchLinesState::Finished => self.move_lines_to_buffer(),
            _ => {}
        }
    }

    fn move_lines_to_buffer(&self) {
        let mut perform = MarkupPerform::default();

        let imp = self.imp();
        let mut lines = imp.fetched_lines.borrow_mut();

        let had_lines = !lines.is_empty();

        for _ in 0..128 {
            match lines.pop_back() {
                Some(line) => self.insert(line, &mut perform, false),
                None => break,
            }
        }

        if had_lines {
            let adj = imp.scrolled_window.vadjustment();
            if adj.value() < 30.0 {
                adj.set_value(adj.value() + 30.0);
            }
        }
    }

    fn on_notify_dark(&self, style_manager: &adw::StyleManager) {
        self.imp().source_buffer.set_style_scheme(
            sourceview5::StyleSchemeManager::default()
                .scheme(if style_manager.is_dark() {
                    "Adwaita-dark"
                } else {
                    "Adwaita"
                })
                .as_ref(),
        );
    }

    async fn save_to_file(&self) {
        if let Some(container) = self.container() {
            let request = SaveFileRequest::default()
                .identifier(WindowIdentifier::from_native(&self.native().unwrap()).await)
                .current_name(format!("{}.log", container.name()).as_str())
                .choice(Choice::boolean(
                    "timestamps",
                    &gettext("Include timestamps"),
                    false,
                ))
                .modal(true);

            utils::show_save_file_dialog(
                request,
                self.upcast_ref(),
                clone!(@weak self as obj => move |files| {
                    obj.imp().save_stack.set_visible_child_name("spinner");

                    let file = gio::File::for_uri(files.uris()[0].as_str());

                    if let Some(path) = file.path() {
                        let file = std::fs::OpenOptions::new()
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .open(path)
                            .unwrap();

                        let mut writer = BufWriter::new(file);
                        let mut perform = PlainTextPerform::default();

                        let timestamps = files.choices()[0].1 == "true";

                        utils::run_stream_with_finish_handler(
                            container.api().unwrap(),
                            move |container| {
                                container
                                    .logs(&basic_opts_builder(false, timestamps).build())
                                    .boxed()
                            },
                            clone!(
                                @weak obj => @default-return glib::Continue(false),
                                move |result: podman::Result<podman::conn::TtyChunk>|
                            {
                                glib::Continue(match result.map(Vec::from) {
                                    Ok(line) => {
                                        perform.decode(&line);

                                        let line = perform.move_out_buffer();
                                        if !line.is_empty() {
                                            match writer
                                                .write_all(line.as_bytes())
                                                .and_then(|_| writer.write_all(b"\n"))
                                            {
                                                Ok(_) => true,
                                                Err(e) => {
                                                    log::warn!("Error on saving logs: {e}");
                                                    utils::show_error_toast(
                                                        obj.upcast_ref(),
                                                        &gettext("Error on saving logs"),
                                                        &e.to_string(),
                                                    );
                                                    false
                                                }
                                            }
                                        } else {
                                            true
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Error on retrieving logs: {e}");
                                        utils::show_error_toast(
                                            obj.upcast_ref(),
                                            &gettext("Error on retrieving logs"),
                                            &e.to_string(),
                                        );
                                        false
                                    }
                                })
                            }),
                            clone!(@weak obj => move || {
                                obj.imp().save_stack.set_visible_child_name("button");
                            }),
                        );
                    }
                }),
            )
            .await;
        }
    }

    fn toggle_search(&self) {
        let imp = self.imp();
        imp.search_bar
            .set_search_mode(!imp.search_bar.is_search_mode());
    }

    fn start_or_resume_container(&self) {
        if let Some(container) = self.container() {
            if container.can_start() {
                container.start(clone!(@weak self as obj => move |result| {
                    if let Err(e) = result {
                        utils::show_error_toast(obj.upcast_ref(), &gettext("Error starting container"), &e.to_string());
                    }
                }));
            } else if container.can_resume() {
                container.resume(clone!(@weak self as obj => move |result| {
                    if let Err(e) = result {
                        utils::show_error_toast(obj.upcast_ref(), &gettext("Error resuming container"), &e.to_string());
                    }
                }));
            }
        }
    }
}

fn basic_opts_builder(follow: bool, timestamps: bool) -> podman::opts::ContainerLogsOptsBuilder {
    podman::opts::ContainerLogsOpts::builder()
        .follow(follow)
        .stdout(true)
        .stderr(true)
        .timestamps(timestamps)
}

#[derive(Debug)]
enum MarkupAttribute {
    Bold,
    Foreground(&'static str),
    Background(&'static str),
}

impl MarkupAttribute {
    fn open_tag(&self) -> Cow<str> {
        match self {
            Self::Bold => Cow::Borrowed("<b>"),
            Self::Foreground(value) => Cow::Owned(format!("<span foreground=\"{value}\">")),
            Self::Background(value) => Cow::Owned(format!("<span background=\"{value}\">")),
        }
    }

    fn close_tag(&self) -> &'static str {
        match self {
            Self::Bold => "</b>",
            Self::Foreground(_) | Self::Background(_) => "</span>",
        }
    }
}

#[derive(Debug, Default)]
pub struct MarkupPerform {
    buffer: String,
    attributes: Vec<MarkupAttribute>,
}

impl MarkupPerform {
    fn move_out_buffer(&mut self) -> String {
        let mut buffer = String::new();
        mem::swap(&mut self.buffer, &mut buffer);
        buffer
    }

    fn begin_line(&mut self) {
        self.attributes.iter().for_each(|attr| {
            self.buffer.push_str(attr.open_tag().as_ref());
        });
    }

    fn end_line(&mut self) {
        self.attributes.iter().rev().for_each(|attr| {
            self.buffer.push_str(attr.close_tag());
        });
    }

    fn reset_all(&mut self) {
        while let Some(attr) = self.attributes.pop() {
            self.buffer.push_str(attr.close_tag());
        }
    }

    fn reset<F: Fn(&MarkupAttribute) -> bool>(&mut self, op: F) {
        let mut t = Vec::new();
        while let Some(attr) = self.attributes.pop() {
            self.buffer.push_str(attr.close_tag());
            if op(&attr) {
                t.insert(0, attr);
            }
        }

        mem::swap(&mut t, &mut self.attributes);

        self.begin_line();
    }

    /// Decode the specified bytes. Return true if finished.
    fn decode(&mut self, ansi_encoded_bytes: &[u8]) -> String {
        let mut parser = vte::Parser::new();

        self.begin_line();

        let line = String::from_utf8_lossy(ansi_encoded_bytes);
        let (timestamp, message) = line.split_once(' ').unwrap();

        message.bytes().for_each(|byte| parser.advance(self, byte));

        self.end_line();

        format!("{timestamp} {}", self.move_out_buffer())
    }
}

impl vte::Perform for MarkupPerform {
    fn print(&mut self, c: char) {
        self.buffer.push(c);
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
        for param in params.iter() {
            param.iter().copied().for_each(|param| {
                match param {
                    0 => self.reset_all(),
                    39 => {
                        // Reset foreground
                        self.reset(|attr| !matches!(attr, MarkupAttribute::Foreground(_)));
                    }
                    49 => {
                        // Reset background
                        self.reset(|attr| !matches!(attr, MarkupAttribute::Background(_)));
                    }
                    _ => {
                        if let Some(attr) = ansi_escape_to_markup_attribute(param) {
                            self.buffer.push_str(attr.open_tag().as_ref());
                            self.attributes.push(attr);
                        }
                    }
                }
            });
        }
    }
}

fn ansi_escape_to_markup_attribute(item: u16) -> Option<MarkupAttribute> {
    Some(match item {
        1 => MarkupAttribute::Bold,

        30 => MarkupAttribute::Foreground("#000000"),
        31 => MarkupAttribute::Foreground("#e01b24"),
        32 => MarkupAttribute::Foreground("#33d17a"),
        33 => MarkupAttribute::Foreground("#f6d32d"),
        34 => MarkupAttribute::Foreground("#3584e4"),
        35 => MarkupAttribute::Foreground("#d4267e"),
        36 => MarkupAttribute::Foreground("#00f7f7"),
        37 => MarkupAttribute::Foreground("#ffffff"),

        40 => MarkupAttribute::Background("#000000"),
        41 => MarkupAttribute::Background("#e01b24"),
        42 => MarkupAttribute::Background("#33d17a"),
        43 => MarkupAttribute::Background("#f6d32d"),
        44 => MarkupAttribute::Background("#3584e4"),
        45 => MarkupAttribute::Background("#d4267e"),
        46 => MarkupAttribute::Background("#00f7f7"),
        47 => MarkupAttribute::Background("#ffffff"),

        90 => MarkupAttribute::Foreground("#3d3846"),
        91 => MarkupAttribute::Foreground("#f66151"),
        92 => MarkupAttribute::Foreground("#8ff0a4"),
        93 => MarkupAttribute::Foreground("#f9f06b"),
        94 => MarkupAttribute::Foreground("#99c1f1"),
        95 => MarkupAttribute::Foreground("#c061cb"),
        96 => MarkupAttribute::Foreground("#33c7de"),
        97 => MarkupAttribute::Foreground("#f66151"),

        100 => MarkupAttribute::Background("#3d3846"),
        101 => MarkupAttribute::Background("#f66151"),
        102 => MarkupAttribute::Background("#8ff0a4"),
        103 => MarkupAttribute::Background("#f9f06b"),
        104 => MarkupAttribute::Background("#99c1f1"),
        105 => MarkupAttribute::Background("#c061cb"),
        106 => MarkupAttribute::Background("#33c7de"),
        109 => MarkupAttribute::Background("#f66151"),

        _ => return None,
    })
}

#[derive(Debug, Default)]
pub struct PlainTextPerform(String);

impl PlainTextPerform {
    fn move_out_buffer(&mut self) -> String {
        let mut buffer = String::new();
        mem::swap(&mut self.0, &mut buffer);
        buffer
    }

    fn decode(&mut self, ansi_encoded_bytes: &[u8]) {
        let mut parser = vte::Parser::new();

        String::from_utf8_lossy(ansi_encoded_bytes)
            .bytes()
            .for_each(|byte| parser.advance(self, byte));
    }
}

impl vte::Perform for PlainTextPerform {
    fn print(&mut self, c: char) {
        self.0.push(c);
    }
}
