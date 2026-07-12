mod config;
mod locale;
mod preview;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use anyhow::Context;
use gtk::gio;
use gtk::glib;

use crate::app_backend::{ReviewPayload, export_run, load_run_with_progress, set_decision};
use crate::counterpart::{
    CounterpartMoveResponse, CounterpartPlanResponse, apply_counterparts, plan_counterparts,
    restore_counterparts,
};
use crate::operations::{
    MoveRejectsResponse, RestoreResponse, final_action_for_asset, move_rejects, restore_moved,
};
use crate::pipeline::run_scan_controlled;
use crate::progress::{CancellationToken, ProgressReporter, ProgressUpdate};
use crate::run_storage::{RelocationProgress, RelocationResult, relocate_run};
use crate::types::{
    AccelerationPreference, AssetRecord, DetectorDevicePreference, DetectorModelPreference,
    DetectorPreference, SuggestedAction, UserDecision,
};

use self::config::{Appearance, GuiConfig, TutorialOutcome, cache_bytes};
use self::locale::LocaleCatalog;

const APPLICATION_ID: &str = "org.burstframe.Deduplicator";
const APP_ICON: &str = "org.burstframe.Deduplicator";

const CSS: &str = r#"
.welcome-title { font-size: 2rem; font-weight: 700; }
.welcome-subtitle { font-size: 1.1rem; color: alpha(currentColor, .68); }
.section-title { font-size: 1.05rem; font-weight: 650; }
.muted { color: alpha(currentColor, .62); }
.review-sidebar { background: alpha(currentColor, .035); padding: 18px; }
.cluster { margin: 0 0 10px 0; }
.photo-card { border: 1px solid alpha(currentColor, .12); border-radius: 6px; background: alpha(currentColor, .025); padding: 10px; }
.photo-card:hover { background: alpha(@accent_color, .055); }
.photo-title { font-weight: 650; }
.quality-bar trough { min-height: 5px; border-radius: 3px; }
.quality-bar block.filled { background: alpha(@accent_color, .62); }
.workload-bar trough { min-height: 4px; }
.workload-bar progress { background: alpha(@accent_color, .45); }
.exif-difference { color: #9a6700; font-weight: 600; }
.status-moved { color: #1c71d8; }
.status-counterpart { color: #008a7a; }
.preview-toolbar { padding: 6px 10px; }
.tutorial-demo { border: 1px solid alpha(currentColor, .12); border-radius: 6px; padding: 14px; }
"#;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ReviewFilter {
    #[default]
    All,
    Keep,
    Reject,
    Review,
}

struct AppState {
    config: GuiConfig,
    locale: LocaleCatalog,
    payload: Option<ReviewPayload>,
    expanded_clusters: HashSet<usize>,
    filter: ReviewFilter,
    source: Option<PathBuf>,
    busy: bool,
    scan_cancellation: Option<CancellationToken>,
}

enum BackgroundEvent {
    Progress(ProgressUpdate),
    ScanFinished {
        result: Result<PathBuf, String>,
        cancelled: bool,
    },
    OpenRun(PathBuf),
    RunLoaded(Result<ReviewPayload, String>),
    DecisionSaved(Result<ReviewPayload, String>),
    Exported(Result<ReviewPayload, String>),
    MoveFinished(Result<MoveRejectsResponse, String>),
    RestoreFinished(Result<RestoreResponse, String>),
    CounterpartPlanned(Result<(PathBuf, CounterpartPlanResponse), String>),
    CounterpartMoved(Result<CounterpartMoveResponse, String>),
    CounterpartRestored(Result<RestoreResponse, String>),
    RelocationProgress(RelocationProgress),
    Relocated(Result<(RelocationResult, PathBuf), String>),
    CacheRemoved(Result<usize, String>),
}

type EventQueue = Arc<Mutex<VecDeque<BackgroundEvent>>>;

struct Controller {
    window: adw::ApplicationWindow,
    application: adw::Application,
    title: adw::WindowTitle,
    stack: gtk::Stack,
    welcome_recent: gtk::ListBox,
    results_path_label: gtk::Label,
    progress_heading: gtk::Label,
    progress_bar: gtk::ProgressBar,
    progress_stage: gtk::Label,
    progress_detail: gtk::Label,
    scan_cancel_button: gtk::Button,
    review_host: gtk::Box,
    review_actions: gtk::Box,
    back_button: gtk::Button,
    move_button: gtk::Button,
    restore_button: gtk::Button,
    counterpart_button: gtk::MenuButton,
    save_button: gtk::Button,
    spinner: gtk::Spinner,
    events: EventQueue,
    state: RefCell<AppState>,
}

thread_local! {
    static CONTROLLERS: RefCell<Vec<Rc<Controller>>> = const { RefCell::new(Vec::new()) };
}

fn request_application_quit(application: &adw::Application) {
    let waiting_for_scan = CONTROLLERS.with(|controllers| {
        let controllers = controllers.borrow();
        let mut waiting = false;
        for controller in controllers.iter() {
            if controller.state.borrow().scan_cancellation.is_some() {
                controller.cancel_scan();
                waiting = true;
            }
        }
        waiting
    });
    if !waiting_for_scan {
        application.quit();
        return;
    }

    let application = application.clone();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let waiting = CONTROLLERS.with(|controllers| {
            controllers
                .borrow()
                .iter()
                .any(|controller| controller.state.borrow().scan_cancellation.is_some())
        });
        if waiting {
            glib::ControlFlow::Continue
        } else {
            application.quit();
            glib::ControlFlow::Break
        }
    });
}

fn build_and_retain_controller(application: &adw::Application) {
    let controller = Controller::build(application);
    CONTROLLERS.with(|controllers| controllers.borrow_mut().push(controller));
}

pub fn run() -> anyhow::Result<()> {
    adw::init().context("initializing libadwaita")?;
    install_css();
    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let quit = gio::SimpleAction::new("quit", None);
    let application_for_quit = application.clone();
    quit.connect_activate(move |_, _| request_application_quit(&application_for_quit));
    application.add_action(&quit);
    application.set_accels_for_action("app.quit", &["<Primary>q"]);
    application.connect_activate(build_and_retain_controller);
    application
        .connect_shutdown(|_| CONTROLLERS.with(|controllers| controllers.borrow_mut().clear()));
    application.run();
    Ok(())
}

fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

impl Controller {
    fn build(application: &adw::Application) -> Rc<Self> {
        let config = GuiConfig::load();
        let locale = LocaleCatalog::load(&config.locale)
            .or_else(|_| LocaleCatalog::load("en"))
            .expect("embedded English locale is valid");
        apply_appearance(config.appearance);

        let window = adw::ApplicationWindow::builder()
            .application(application)
            .default_width(config.window_width)
            .default_height(config.window_height)
            .title(locale.text("appTitle"))
            .icon_name(APP_ICON)
            .build();
        let title = adw::WindowTitle::new(&locale.text("appTitle"), "");
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&title));

        let back_button = icon_button("go-previous-symbolic", &locale.text("backToStart"));
        back_button.set_visible(false);
        header.pack_start(&back_button);

        let review_actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        review_actions.set_visible(false);
        let save_button = icon_button("document-save-symbolic", &locale.text("saveReview"));
        let move_button = gtk::Button::builder()
            .icon_name("folder-move-symbolic")
            .tooltip_text(locale.text("move"))
            .build();
        move_button.add_css_class("destructive-action");
        let restore_button = icon_button("edit-undo-symbolic", &locale.text("restoreMoved"));
        let counterpart_button = gtk::MenuButton::builder()
            .icon_name("folder-visiting-symbolic")
            .tooltip_text(locale.text("counterpartCardHelp"))
            .build();
        review_actions.append(&save_button);
        review_actions.append(&move_button);
        review_actions.append(&restore_button);
        review_actions.append(&counterpart_button);
        header.pack_end(&review_actions);

        let spinner = gtk::Spinner::new();
        spinner.set_visible(false);
        header.pack_end(&spinner);
        let settings_button = icon_button("emblem-system-symbolic", &locale.text("settings"));
        let tutorial_button = icon_button("help-browser-symbolic", &locale.text("tutorialMenu"));
        let about_button = icon_button("help-about-symbolic", &locale.text("aboutTitle"));
        header.pack_end(&about_button);
        header.pack_end(&tutorial_button);
        header.pack_end(&settings_button);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(180)
            .hexpand(true)
            .vexpand(true)
            .build();
        let (welcome, welcome_recent, results_path_label, new_scan, open_run) =
            welcome_page(&locale, &config);
        stack.add_named(&welcome, Some("welcome"));
        let (
            scanning,
            progress_heading,
            progress_bar,
            progress_stage,
            progress_detail,
            scan_cancel_button,
        ) = scanning_page(&locale);
        stack.add_named(&scanning, Some("scanning"));
        let review_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
        review_host.set_hexpand(true);
        review_host.set_vexpand(true);
        stack.add_named(&review_host, Some("review"));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&stack));
        window.set_content(Some(&toolbar));

        let controller = Rc::new(Self {
            window,
            application: application.clone(),
            title,
            stack,
            welcome_recent,
            results_path_label,
            progress_heading,
            progress_bar,
            progress_stage,
            progress_detail,
            scan_cancel_button,
            review_host,
            review_actions,
            back_button,
            move_button,
            restore_button,
            counterpart_button,
            save_button,
            spinner,
            events: Arc::new(Mutex::new(VecDeque::new())),
            state: RefCell::new(AppState {
                config,
                locale,
                payload: None,
                expanded_clusters: HashSet::new(),
                filter: ReviewFilter::All,
                source: None,
                busy: false,
                scan_cancellation: None,
            }),
        });

        controller.install_handlers(
            &new_scan,
            &open_run,
            &settings_button,
            &tutorial_button,
            &about_button,
        );
        controller.configure_counterpart_menu();
        controller.refresh_history();
        controller.install_event_pump();
        controller.window.present();
        if !controller.state.borrow().config.tutorial_finished() {
            let weak = Rc::downgrade(&controller);
            glib::idle_add_local_once(move || {
                if let Some(controller) = weak.upgrade() {
                    controller.show_tutorial(false);
                }
            });
        }
        controller
    }

    fn install_handlers(
        self: &Rc<Self>,
        new_scan: &gtk::Button,
        open_run: &gtk::Button,
        settings: &gtk::Button,
        tutorial: &gtk::Button,
        about: &gtk::Button,
    ) {
        let weak = Rc::downgrade(self);
        new_scan.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.choose_photo_folder();
            }
        });
        let weak = Rc::downgrade(self);
        open_run.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.choose_run_folder();
            }
        });
        let weak = Rc::downgrade(self);
        self.back_button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.show_welcome();
            }
        });
        let weak = Rc::downgrade(self);
        self.save_button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.export_review();
            }
        });
        let weak = Rc::downgrade(self);
        self.move_button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.confirm_move_rejects();
            }
        });
        let weak = Rc::downgrade(self);
        self.restore_button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.confirm_restore_rejects();
            }
        });
        let weak = Rc::downgrade(self);
        settings.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.show_settings();
            }
        });
        let weak = Rc::downgrade(self);
        tutorial.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.show_tutorial(true);
            }
        });
        let weak = Rc::downgrade(self);
        about.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.show_about();
            }
        });
        let weak = Rc::downgrade(self);
        self.scan_cancel_button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.cancel_scan();
            }
        });
        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |window| {
            if let Some(controller) = weak.upgrade() {
                if controller.state.borrow().scan_cancellation.is_some() {
                    controller.cancel_scan();
                    window.set_sensitive(false);
                    let window = window.clone();
                    let weak = Rc::downgrade(&controller);
                    glib::timeout_add_local(Duration::from_millis(50), move || {
                        let waiting = weak.upgrade().is_some_and(|controller| {
                            controller.state.borrow().scan_cancellation.is_some()
                        });
                        if waiting {
                            glib::ControlFlow::Continue
                        } else {
                            window.close();
                            glib::ControlFlow::Break
                        }
                    });
                    return glib::Propagation::Stop;
                }
                let mut state = controller.state.borrow_mut();
                state.config.window_width = window.width();
                state.config.window_height = window.height();
                let _ = state.config.save();
            }
            CONTROLLERS.with(|controllers| {
                controllers
                    .borrow_mut()
                    .retain(|controller| controller.window != *window);
            });
            glib::Propagation::Proceed
        });
    }

    fn install_event_pump(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let Some(controller) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            controller.drain_events();
            glib::ControlFlow::Continue
        });
    }

    fn drain_events(self: &Rc<Self>) {
        let events: Vec<_> = self
            .events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain(..)
            .collect();
        for event in events {
            self.handle_event(event);
        }
    }

    fn handle_event(self: &Rc<Self>, event: BackgroundEvent) {
        match event {
            BackgroundEvent::OpenRun(path) => self.load_run_async(path),
            BackgroundEvent::Progress(update) => self.update_progress(&update),
            BackgroundEvent::ScanFinished { result, cancelled } => {
                self.state.borrow_mut().scan_cancellation = None;
                self.scan_cancel_button.set_sensitive(true);
                match result {
                    Ok(run_dir) => self.load_run_async(run_dir),
                    Err(_) if cancelled => {
                        self.set_busy(false);
                        self.show_welcome();
                    }
                    Err(error) => {
                        self.set_busy(false);
                        self.show_error(&error);
                        self.show_welcome();
                    }
                }
            }
            BackgroundEvent::RunLoaded(result) => match result {
                Ok(payload) => self.install_payload(payload),
                Err(error) => {
                    let had_payload = self.state.borrow().payload.is_some();
                    self.set_busy(false);
                    if had_payload {
                        self.stack.set_visible_child_name("review");
                        self.review_actions.set_visible(true);
                        self.back_button.set_visible(true);
                    } else {
                        self.show_welcome();
                    }
                    self.show_error(&error);
                }
            },
            BackgroundEvent::DecisionSaved(result) | BackgroundEvent::Exported(result) => {
                match result {
                    Ok(payload) => self.install_payload(payload),
                    Err(error) => {
                        self.set_busy(false);
                        self.stack.set_visible_child_name("review");
                        self.review_actions.set_visible(true);
                        self.back_button.set_visible(true);
                        self.show_error(&error);
                    }
                }
            }
            BackgroundEvent::MoveFinished(result) => match result {
                Ok(response) => {
                    self.set_busy(false);
                    self.show_notice(&format!(
                        "{}\n{}",
                        self.locale().format(
                            "moveComplete",
                            &[
                                ("files", response.moved_files.to_string()),
                                ("assets", response.moved_assets.to_string()),
                            ],
                        ),
                        response.destination.display()
                    ));
                    self.reload_current_run();
                }
                Err(error) => {
                    self.set_busy(false);
                    self.show_error(&error);
                }
            },
            BackgroundEvent::RestoreFinished(result)
            | BackgroundEvent::CounterpartRestored(result) => match result {
                Ok(response) => {
                    self.set_busy(false);
                    self.show_notice(&self.locale().format(
                        "restoreComplete",
                        &[
                            ("files", response.restored_files.to_string()),
                            ("assets", response.restored_assets.to_string()),
                        ],
                    ));
                    self.reload_current_run();
                }
                Err(error) => {
                    self.set_busy(false);
                    self.show_error(&error);
                }
            },
            BackgroundEvent::CounterpartPlanned(result) => {
                self.set_busy(false);
                match result {
                    Ok((card_root, plan)) => self.show_counterpart_plan(card_root, plan),
                    Err(error) => self.show_error(&error),
                }
            }
            BackgroundEvent::CounterpartMoved(result) => match result {
                Ok(response) => {
                    self.set_busy(false);
                    self.show_notice(&self.locale().format(
                        "counterpartMoveComplete",
                        &[
                            ("files", response.moved_files.to_string()),
                            ("assets", response.moved_assets.to_string()),
                        ],
                    ));
                    self.reload_current_run();
                }
                Err(error) => {
                    self.set_busy(false);
                    self.show_error(&error);
                }
            },
            BackgroundEvent::RelocationProgress(update) => {
                self.progress_bar
                    .set_fraction(f64::from(update.overall_fraction));
                self.progress_stage
                    .set_text(&self.locale().text("movingRunFolder"));
                self.progress_detail
                    .set_text(update.detail.as_deref().unwrap_or_default());
            }
            BackgroundEvent::Relocated(result) => match result {
                Ok((relocation, new_root)) => {
                    {
                        let mut state = self.state.borrow_mut();
                        state.config.results_root = new_root;
                        state.config.register_run(relocation.run_dir.clone());
                        let _ = state.config.save();
                    }
                    self.load_run_async(relocation.run_dir);
                }
                Err(error) => {
                    self.set_busy(false);
                    self.show_error(&error);
                }
            },
            BackgroundEvent::CacheRemoved(result) => {
                self.set_busy(false);
                match result {
                    Ok(count) => {
                        {
                            let mut state = self.state.borrow_mut();
                            state.config.recent_runs.retain(|path| path.is_dir());
                            let _ = state.config.save();
                        }
                        self.show_notice(&format!(
                            "{}: {count}",
                            self.locale().text("removePreviousRuns")
                        ));
                        self.refresh_history();
                    }
                    Err(error) => self.show_error(&error),
                }
            }
        }
    }

    fn locale(&self) -> LocaleCatalog {
        self.state.borrow().locale.clone()
    }

    fn set_busy(&self, busy: bool) {
        self.state.borrow_mut().busy = busy;
        self.spinner.set_visible(busy);
        if busy {
            self.spinner.start();
        } else {
            self.spinner.stop();
        }
        self.review_actions.set_sensitive(!busy);
    }
}

#[derive(Clone)]
enum ReviewListEntry {
    Cluster {
        id: usize,
        burst_id: usize,
        frame_count: usize,
        keep_count: usize,
        confidence: f64,
        expanded: bool,
    },
    Asset {
        asset: Box<AssetRecord>,
        exif_differs: bool,
    },
}

impl Controller {
    fn show_welcome(&self) {
        self.stack.set_visible_child_name("welcome");
        self.review_actions.set_visible(false);
        self.back_button.set_visible(false);
        self.title.set_subtitle("");
        self.state.borrow_mut().payload = None;
        self.refresh_history();
    }

    fn choose_photo_folder(self: &Rc<Self>) {
        let locale = self.locale();
        self.choose_folder(
            &locale.text("selectPhotosTitle"),
            self.state.borrow().source.as_deref(),
            |controller, path| controller.start_scan(path),
        );
    }

    fn choose_run_folder(self: &Rc<Self>) {
        let locale = self.locale();
        self.choose_folder(&locale.text("selectRunTitle"), None, |controller, path| {
            controller.load_run_async(path)
        });
    }

    fn choose_folder(
        self: &Rc<Self>,
        title: &str,
        initial: Option<&Path>,
        action: impl Fn(&Rc<Self>, PathBuf) + 'static,
    ) {
        let dialog = gtk::FileDialog::builder().title(title).modal(true).build();
        if let Some(initial) = initial {
            dialog.set_initial_folder(Some(&gio::File::for_path(initial)));
        }
        let weak = Rc::downgrade(self);
        dialog.select_folder(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |result| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                match result.and_then(|file| {
                    file.path().ok_or_else(|| {
                        glib::Error::new(gio::IOErrorEnum::NotSupported, "Folder has no local path")
                    })
                }) {
                    Ok(path) => action(&controller, path),
                    Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {}
                    Err(error) => controller.show_error(&error.to_string()),
                }
            },
        );
    }

    fn start_scan(self: &Rc<Self>, source: PathBuf) {
        if self.state.borrow().busy {
            return;
        }
        let cancellation = CancellationToken::new();
        let (mut options, output) = {
            let mut state = self.state.borrow_mut();
            state.source = Some(source.clone());
            state.scan_cancellation = Some(cancellation.clone());
            let mut options = state.config.options.clone();
            options.detector_model_pack = state.config.model_pack.clone();
            let output = unique_run_directory(&state.config.results_root);
            (options, output)
        };
        if options.workers.is_none() {
            options.workers = std::thread::available_parallelism()
                .ok()
                .map(|value| value.get().min(8));
        }
        self.title.set_subtitle(
            source
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        );
        self.progress_bar.set_fraction(0.0);
        self.progress_heading
            .set_text(&self.locale().text("analyzingPhotoFolder"));
        self.progress_stage
            .set_text(&self.locale().text("preparing"));
        self.progress_detail.set_text("");
        self.scan_cancel_button.set_visible(true);
        self.scan_cancel_button.set_sensitive(true);
        self.stack.set_visible_child_name("scanning");
        self.review_actions.set_visible(false);
        self.back_button.set_visible(false);
        self.set_busy(true);

        let events = self.events.clone();
        thread::spawn(move || {
            let progress_events = events.clone();
            let reporter = ProgressReporter::new(move |update| {
                push_event(&progress_events, BackgroundEvent::Progress(update));
            });
            let result = run_scan_controlled(
                &source,
                Some(output),
                options,
                reporter,
                cancellation.clone(),
            )
            .map_err(|error| format!("{error:#}"));
            push_event(
                &events,
                BackgroundEvent::ScanFinished {
                    result,
                    cancelled: cancellation.is_cancelled(),
                },
            );
        });
    }

    fn load_run_async(&self, run_dir: PathBuf) {
        self.progress_heading
            .set_text(&self.locale().text("openingRun"));
        self.progress_bar.set_fraction(0.0);
        self.progress_stage
            .set_text(&self.locale().text("reading_manifest"));
        self.progress_detail.set_text("");
        self.scan_cancel_button.set_visible(false);
        self.title.set_subtitle(
            run_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        );
        self.stack.set_visible_child_name("scanning");
        self.review_actions.set_visible(false);
        self.back_button.set_visible(false);
        self.set_busy(true);
        let events = self.events.clone();
        thread::spawn(move || {
            let progress_events = events.clone();
            let reporter = ProgressReporter::new(move |update| {
                push_event(&progress_events, BackgroundEvent::Progress(update));
            });
            let result =
                load_run_with_progress(run_dir, reporter).map_err(|error| format!("{error:#}"));
            push_event(&events, BackgroundEvent::RunLoaded(result));
        });
    }

    fn cancel_scan(&self) {
        let cancellation = self.state.borrow().scan_cancellation.clone();
        let Some(cancellation) = cancellation else {
            return;
        };
        cancellation.cancel();
        self.scan_cancel_button.set_sensitive(false);
        self.progress_stage
            .set_text(&self.locale().text("cancellingScan"));
    }

    fn reload_current_run(&self) {
        if let Some(run_dir) = self
            .state
            .borrow()
            .payload
            .as_ref()
            .map(|payload| payload.run_dir.clone())
        {
            self.load_run_async(run_dir);
        }
    }

    fn install_payload(self: &Rc<Self>, payload: ReviewPayload) {
        {
            let mut state = self.state.borrow_mut();
            if state.payload.as_ref().map(|value| &value.run_dir) != Some(&payload.run_dir) {
                state.expanded_clusters.clear();
                let assets_by_id: HashMap<_, _> = payload
                    .manifest
                    .assets
                    .iter()
                    .map(|asset| (asset.id.as_str(), asset))
                    .collect();
                let decisions_by_id: HashMap<_, _> = payload
                    .review
                    .decisions
                    .iter()
                    .filter_map(|decision| {
                        decision
                            .decision
                            .map(|value| (decision.asset_id.as_str(), value))
                    })
                    .collect();
                for cluster in &payload.manifest.clusters {
                    let all_kept = cluster.asset_ids.iter().all(|asset_id| {
                        assets_by_id.get(asset_id.as_str()).is_some_and(|asset| {
                            decisions_by_id.get(asset_id.as_str()).copied().unwrap_or(
                                match asset.suggestion.action {
                                    SuggestedAction::Keep => UserDecision::Keep,
                                    SuggestedAction::Reject => UserDecision::Reject,
                                    SuggestedAction::Review | SuggestedAction::Error => {
                                        UserDecision::Review
                                    }
                                },
                            ) == UserDecision::Keep
                        })
                    });
                    if cluster.asset_ids.len() > 1 && !all_kept {
                        state.expanded_clusters.insert(cluster.id);
                    }
                }
            }
            state.config.register_run(payload.run_dir.clone());
            let _ = state.config.save();
            state.payload = Some(payload);
        }
        self.set_busy(false);
        self.stack.set_visible_child_name("review");
        self.review_actions.set_visible(true);
        self.back_button.set_visible(true);
        self.rebuild_review();
        self.refresh_history();
    }

    fn update_progress(&self, update: &ProgressUpdate) {
        self.progress_bar
            .set_fraction(f64::from(update.overall_fraction));
        let cancelling = self
            .state
            .borrow()
            .scan_cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled);
        self.progress_stage
            .set_text(&self.locale().text(if cancelling {
                "cancellingScan"
            } else {
                update.stage.locale_key()
            }));
        let detail = update
            .detail
            .as_deref()
            .and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        self.progress_detail.set_text(detail);
    }

    fn refresh_history(&self) {
        clear_boxed_list(&self.welcome_recent);
        let (locale, mut paths, root) = {
            let state = self.state.borrow();
            (
                state.locale.clone(),
                state.config.recent_runs.clone(),
                state.config.results_root.clone(),
            )
        };
        if let Ok(entries) = std::fs::read_dir(&root) {
            paths.extend(
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.join("manifest.json").is_file()),
            );
        }
        let mut seen = HashSet::new();
        paths.retain(|path| seen.insert(path.clone()) && path.join("manifest.json").is_file());
        paths.sort_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH)
        });
        paths.reverse();

        for path in paths.into_iter().take(10) {
            let Ok(manifest) = crate::artifacts::read_manifest(&path) else {
                continue;
            };
            let button = gtk::Button::new();
            button.add_css_class("flat");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.set_margin_top(8);
            row.set_margin_bottom(8);
            row.set_margin_start(10);
            row.set_margin_end(10);
            let image = gtk::Image::from_icon_name(if manifest.root.is_dir() {
                "folder-pictures-symbolic"
            } else {
                "media-removable-symbolic"
            });
            let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
            labels.set_hexpand(true);
            let name = left_label(
                manifest
                    .root
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_else(|| {
                        path.file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("Run")
                    }),
            );
            name.add_css_class("photo-title");
            let detail = left_label(&format!(
                "{} · {}",
                manifest.created_at,
                locale.format(
                    "photosCount",
                    &[("count", manifest.assets.len().to_string())],
                )
            ));
            detail.add_css_class("muted");
            labels.append(&name);
            labels.append(&detail);
            row.append(&image);
            row.append(&labels);
            button.set_child(Some(&row));
            let events = self.events.clone();
            button.connect_clicked(move |_| {
                push_event(&events, BackgroundEvent::OpenRun(path.clone()));
            });
            self.welcome_recent.append(&button);
        }
        if self.welcome_recent.first_child().is_none() {
            let empty = left_label(&locale.text("noRecentRunsDetail"));
            empty.set_margin_top(24);
            empty.set_margin_bottom(24);
            empty.set_margin_start(12);
            empty.set_margin_end(12);
            empty.add_css_class("muted");
            self.welcome_recent.append(&empty);
        }
        self.results_path_label
            .set_text(&root.display().to_string());
    }

    fn rebuild_review(self: &Rc<Self>) {
        clear_box(&self.review_host);
        let (payload, locale, filter, expanded) = {
            let state = self.state.borrow();
            let Some(payload) = state.payload.clone() else {
                return;
            };
            (
                payload,
                state.locale.clone(),
                state.filter,
                state.expanded_clusters.clone(),
            )
        };
        self.title.set_subtitle(
            payload
                .manifest
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        );

        let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        paned.set_position(238);
        paned.set_shrink_start_child(false);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 12);
        sidebar.add_css_class("review-sidebar");
        sidebar.set_size_request(220, -1);
        let review_title = left_label(&locale.text("review"));
        review_title.add_css_class("section-title");
        sidebar.append(&review_title);

        let counts = decision_counts(&payload);
        for (label, value) in [
            (locale.text("images"), payload.manifest.assets.len()),
            (locale.text("stacks"), payload.manifest.clusters.len()),
            (locale.text("keptFrames"), counts.0),
            (locale.text("rejectedFrames"), counts.1),
            (locale.text("needsReview"), counts.2),
            (
                locale.text("movedFrames"),
                payload.move_status.active_primary_asset_ids.len(),
            ),
        ] {
            sidebar.append(&stat_row(&label, value));
        }
        let filter_label = left_label(&locale.text("filter"));
        filter_label.add_css_class("muted");
        let filter_labels = [
            locale.text("allSuggestions"),
            locale.text("keptFrames"),
            locale.text("rejectedFrames"),
            locale.text("needsReview"),
        ];
        let filter_label_refs: Vec<_> = filter_labels.iter().map(String::as_str).collect();
        let filter_combo = gtk::DropDown::from_strings(&filter_label_refs);
        filter_combo.set_selected(match filter {
            ReviewFilter::All => 0,
            ReviewFilter::Keep => 1,
            ReviewFilter::Reject => 2,
            ReviewFilter::Review => 3,
        });
        let weak = Rc::downgrade(self);
        filter_combo.connect_selected_notify(move |combo| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            controller.state.borrow_mut().filter = match combo.selected() {
                1 => ReviewFilter::Keep,
                2 => ReviewFilter::Reject,
                3 => ReviewFilter::Review,
                _ => ReviewFilter::All,
            };
            controller.rebuild_review();
        });
        sidebar.append(&filter_label);
        sidebar.append(&filter_combo);
        paned.set_start_child(Some(&sidebar));

        let entries = flattened_review_entries(&payload, &expanded, filter);
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        for entry in entries {
            model.append(&glib::BoxedAnyObject::new(entry));
        }
        let selection = gtk::NoSelection::new(Some(model));
        let factory = gtk::SignalListItemFactory::new();
        let weak = Rc::downgrade(self);
        let payload_for_rows = payload.clone();
        factory.connect_bind(move |_, list_item| {
            let Some(list_item) = list_item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(controller) = weak.upgrade() else {
                return;
            };
            let Some(item) = list_item.item().and_downcast::<glib::BoxedAnyObject>() else {
                return;
            };
            let entry = item.borrow::<ReviewListEntry>().clone();
            let widget = match entry {
                ReviewListEntry::Cluster {
                    id,
                    burst_id,
                    frame_count,
                    keep_count,
                    confidence,
                    expanded,
                } => controller.cluster_row(
                    id,
                    burst_id,
                    frame_count,
                    keep_count,
                    confidence,
                    expanded,
                ),
                ReviewListEntry::Asset {
                    asset,
                    exif_differs,
                } => controller.asset_row(&payload_for_rows, *asset, exif_differs),
            };
            list_item.set_child(Some(&widget));
        });
        factory.connect_unbind(|_, object| {
            if let Some(list_item) = object.downcast_ref::<gtk::ListItem>() {
                list_item.set_child(None::<&gtk::Widget>);
            }
        });
        let list = gtk::ListView::new(Some(selection), Some(factory));
        list.set_single_click_activate(false);
        list.add_css_class("navigation-sidebar");
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .hexpand(true)
            .child(&list)
            .build();
        paned.set_end_child(Some(&scroller));
        self.review_host.append(&paned);

        self.move_button
            .set_sensitive(counts.1 > 0 && !self.state.borrow().busy);
        self.move_button.set_tooltip_text(Some(
            &locale.format("moveRejects", &[("count", counts.1.to_string())]),
        ));
        self.restore_button
            .set_visible(!payload.move_status.active_primary_asset_ids.is_empty());
        self.counterpart_button.set_sensitive(
            counts.1 > 0 || !payload.move_status.active_counterpart_asset_ids.is_empty(),
        );
    }

    fn cluster_row(
        self: &Rc<Self>,
        id: usize,
        burst_id: usize,
        frame_count: usize,
        keep_count: usize,
        confidence: f64,
        expanded: bool,
    ) -> gtk::Widget {
        let locale = self.locale();
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.add_css_class("cluster");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_margin_top(10);
        row.set_margin_bottom(8);
        row.set_margin_start(12);
        row.set_margin_end(12);
        let arrow = gtk::Image::from_icon_name(if expanded {
            "pan-down-symbolic"
        } else {
            "pan-end-symbolic"
        });
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let title = left_label(&locale.format(
            "stackTitle",
            &[("burst", burst_id.to_string()), ("stack", id.to_string())],
        ));
        title.add_css_class("section-title");
        let summary = left_label(&locale.format(
            "stackSummary",
            &[
                ("count", frame_count.to_string()),
                (
                    "state",
                    locale.text(if expanded { "expanded" } else { "collapsed" }),
                ),
                ("keep", keep_count.to_string()),
                ("confidence", format!("{confidence:.2}")),
            ],
        ));
        summary.add_css_class("muted");
        labels.append(&title);
        labels.append(&summary);
        row.append(&arrow);
        row.append(&labels);
        button.set_child(Some(&row));
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            let mut state = controller.state.borrow_mut();
            if !state.expanded_clusters.remove(&id) {
                state.expanded_clusters.insert(id);
            }
            drop(state);
            controller.rebuild_review();
        });
        button.upcast()
    }

    fn asset_row(
        self: &Rc<Self>,
        payload: &ReviewPayload,
        asset: AssetRecord,
        exif_differs: bool,
    ) -> gtk::Widget {
        let locale = self.locale();
        let action = final_action_for_asset(&asset, &payload.review);
        let moved = payload
            .move_status
            .active_primary_asset_ids
            .contains(&asset.id);
        let counterpart_moved = payload
            .move_status
            .active_counterpart_asset_ids
            .contains(&asset.id);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        row.add_css_class("photo-card");
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.set_margin_bottom(6);
        row.set_margin_top(2);

        let preview_button = gtk::Button::new();
        preview_button.add_css_class("flat");
        preview_button.set_tooltip_text(Some(&locale.text("openPreview")));
        let picture = if let Some(relative) = &asset.thumb {
            gtk::Picture::for_filename(payload.run_dir.join(relative))
        } else {
            gtk::Picture::new()
        };
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(168, 112);
        preview_button.set_child(Some(&picture));
        let weak = Rc::downgrade(self);
        let preview_asset_id = asset.id.clone();
        preview_button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.open_preview(&preview_asset_id);
            }
        });
        row.append(&preview_button);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_hexpand(true);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let check = gtk::CheckButton::new();
        check.set_active(action == UserDecision::Keep);
        check.set_inconsistent(action == UserDecision::Review);
        check.set_tooltip_text(Some(&locale.text("keep")));
        let filename = left_label(&asset.representative.rel_path);
        filename.add_css_class("photo-title");
        filename.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        filename.set_hexpand(true);
        header.append(&check);
        header.append(&filename);
        if moved {
            let label = gtk::Label::new(Some(&locale.text("moved")));
            label.add_css_class("status-moved");
            header.append(&label);
        }
        if counterpart_moved {
            let label = gtk::Label::new(Some(&locale.text("counterpartMoved")));
            label.add_css_class("status-counterpart");
            header.append(&label);
        }
        let menu = self.asset_decision_menu(&asset.id);
        header.append(&menu);
        content.append(&header);

        let quality = gtk::LevelBar::for_interval(0.0, 1.0);
        quality.set_value(asset.suggestion.score.clamp(0.0, 1.0));
        quality.add_css_class("quality-bar");
        quality.set_tooltip_text(Some(&locale.format(
            "qualityScore",
            &[(
                "score",
                format!("{:.0}", asset.suggestion.score.clamp(0.0, 1.0) * 100.0),
            )],
        )));
        content.append(&quality);
        let reason = left_label(&suggestion_reason(&locale, &asset));
        reason.set_wrap(true);
        content.append(&reason);
        let exif = left_label(&exif_summary(&locale, &asset));
        exif.add_css_class("muted");
        if exif_differs {
            exif.add_css_class("exif-difference");
        }
        content.append(&exif);
        let details = gtk::Expander::builder()
            .label(locale.text("details"))
            .child(&detail_label(&locale, &asset))
            .build();
        content.append(&details);
        row.append(&content);

        let weak = Rc::downgrade(self);
        let asset_id = asset.id.clone();
        check.connect_toggled(move |check| {
            if let Some(controller) = weak.upgrade() {
                controller.persist_decision(
                    &asset_id,
                    Some(if check.is_active() {
                        UserDecision::Keep
                    } else {
                        UserDecision::Reject
                    }),
                );
            }
        });
        row.upcast()
    }

    fn asset_decision_menu(self: &Rc<Self>, asset_id: &str) -> gtk::MenuButton {
        let locale = self.locale();
        let menu = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(locale.text("manualEdits"))
            .build();
        let popover = gtk::Popover::new();
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 2);
        actions.set_margin_top(6);
        actions.set_margin_bottom(6);
        actions.set_margin_start(6);
        actions.set_margin_end(6);
        for (label, decision) in [
            (locale.text("keep"), Some(UserDecision::Keep)),
            (locale.text("rejected"), Some(UserDecision::Reject)),
            (locale.text("needsReview"), Some(UserDecision::Review)),
            (locale.text("resetSuggestion"), None),
        ] {
            let button = gtk::Button::with_label(&label);
            button.add_css_class("flat");
            let weak = Rc::downgrade(self);
            let asset_id = asset_id.to_string();
            let popover = popover.clone();
            button.connect_clicked(move |_| {
                popover.popdown();
                if let Some(controller) = weak.upgrade() {
                    controller.persist_decision(&asset_id, decision);
                }
            });
            actions.append(&button);
        }
        popover.set_child(Some(&actions));
        menu.set_popover(Some(&popover));
        menu
    }

    fn persist_decision(&self, asset_id: &str, decision: Option<UserDecision>) {
        let Some(run_dir) = self
            .state
            .borrow()
            .payload
            .as_ref()
            .map(|payload| payload.run_dir.clone())
        else {
            return;
        };
        let events = self.events.clone();
        let asset_id = asset_id.to_string();
        thread::spawn(move || {
            let result =
                set_decision(&run_dir, asset_id, decision).map_err(|error| format!("{error:#}"));
            push_event(&events, BackgroundEvent::DecisionSaved(result));
        });
    }
}

impl Controller {
    fn export_review(&self) {
        let Some(run_dir) = self.current_run_dir() else {
            return;
        };
        self.set_busy(true);
        let events = self.events.clone();
        thread::spawn(move || {
            let result = export_run(&run_dir).map_err(|error| format!("{error:#}"));
            push_event(&events, BackgroundEvent::Exported(result));
        });
    }

    fn confirm_move_rejects(self: &Rc<Self>) {
        let locale = self.locale();
        let destination = self.state.borrow().config.reject_destination.clone();
        self.confirm(
            &locale.text("moveConfirmTitle"),
            &locale.text("moveConfirmMessage"),
            &locale.text("move"),
            true,
            move |controller| {
                let Some(run_dir) = controller.current_run_dir() else {
                    return;
                };
                controller.set_busy(true);
                let events = controller.events.clone();
                let destination = destination.clone();
                thread::spawn(move || {
                    let result = move_rejects(&run_dir, destination.as_deref(), true)
                        .map_err(|error| format!("{error:#}"));
                    push_event(&events, BackgroundEvent::MoveFinished(result));
                });
            },
        );
    }

    fn confirm_restore_rejects(self: &Rc<Self>) {
        let locale = self.locale();
        self.confirm(
            &locale.text("restoreConfirmTitle"),
            &locale.text("restoreConfirmMessage"),
            &locale.text("restore"),
            false,
            move |controller| {
                let Some(run_dir) = controller.current_run_dir() else {
                    return;
                };
                controller.set_busy(true);
                let events = controller.events.clone();
                thread::spawn(move || {
                    let result =
                        restore_moved(&run_dir, None, true).map_err(|error| format!("{error:#}"));
                    push_event(&events, BackgroundEvent::RestoreFinished(result));
                });
            },
        );
    }

    fn configure_counterpart_menu(self: &Rc<Self>) {
        let locale = self.locale();
        let popover = gtk::Popover::new();
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 2);
        actions.set_margin_top(6);
        actions.set_margin_bottom(6);
        actions.set_margin_start(6);
        actions.set_margin_end(6);
        let apply = gtk::Button::with_label(&locale.text("applyToCounterpartCard"));
        apply.add_css_class("flat");
        let restore = gtk::Button::with_label(&locale.text("restoreCounterpart"));
        restore.add_css_class("flat");
        let weak = Rc::downgrade(self);
        let popover_for_apply = popover.clone();
        apply.connect_clicked(move |_| {
            popover_for_apply.popdown();
            if let Some(controller) = weak.upgrade() {
                controller.choose_counterpart_card(false);
            }
        });
        let weak = Rc::downgrade(self);
        let popover_for_restore = popover.clone();
        restore.connect_clicked(move |_| {
            popover_for_restore.popdown();
            if let Some(controller) = weak.upgrade() {
                controller.choose_counterpart_card(true);
            }
        });
        actions.append(&apply);
        actions.append(&restore);
        popover.set_child(Some(&actions));
        self.counterpart_button.set_popover(Some(&popover));
    }

    fn choose_counterpart_card(self: &Rc<Self>, restore: bool) {
        let locale = self.locale();
        self.choose_folder(
            &locale.text("selectCounterpartCardTitle"),
            None,
            move |controller, card_root| {
                let Some(run_dir) = controller.current_run_dir() else {
                    return;
                };
                controller.set_busy(true);
                let events = controller.events.clone();
                if restore {
                    thread::spawn(move || {
                        let result = restore_counterparts(&run_dir, &card_root, true)
                            .map_err(|error| format!("{error:#}"));
                        push_event(&events, BackgroundEvent::CounterpartRestored(result));
                    });
                } else {
                    thread::spawn(move || {
                        let result = plan_counterparts(&run_dir, &card_root)
                            .map(|plan| (card_root, plan))
                            .map_err(|error| format!("{error:#}"));
                        push_event(&events, BackgroundEvent::CounterpartPlanned(result));
                    });
                }
            },
        );
    }

    fn show_counterpart_plan(self: &Rc<Self>, card_root: PathBuf, plan: CounterpartPlanResponse) {
        let locale = self.locale();
        let body = format!(
            "{}: {}\n{}: {}\n{}: {}\n\n{}",
            locale.text("counterpartMatched"),
            plan.matched_assets,
            locale.text("counterpartFiles"),
            plan.matched_files,
            locale.text("counterpartExpected"),
            plan.expected_assets,
            locale.text("counterpartStemRule")
        );
        self.confirm(
            &locale.text("counterpartPlanTitle"),
            &body,
            &locale.text("move"),
            true,
            move |controller| {
                let Some(run_dir) = controller.current_run_dir() else {
                    return;
                };
                let destination = controller.state.borrow().config.reject_destination.clone();
                controller.set_busy(true);
                let events = controller.events.clone();
                let card_root = card_root.clone();
                thread::spawn(move || {
                    let result =
                        apply_counterparts(&run_dir, &card_root, destination.as_deref(), true)
                            .map_err(|error| format!("{error:#}"));
                    push_event(&events, BackgroundEvent::CounterpartMoved(result));
                });
            },
        );
    }

    fn current_run_dir(&self) -> Option<PathBuf> {
        self.state
            .borrow()
            .payload
            .as_ref()
            .map(|payload| payload.run_dir.clone())
    }

    fn confirm(
        self: &Rc<Self>,
        heading: &str,
        body: &str,
        confirm_label: &str,
        destructive: bool,
        action: impl Fn(&Rc<Self>) + 'static,
    ) {
        let locale = self.locale();
        let dialog = adw::MessageDialog::new(Some(&self.window), Some(heading), Some(body));
        dialog.add_response("cancel", &locale.text("cancel"));
        dialog.add_response("confirm", confirm_label);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.set_response_appearance(
            "confirm",
            if destructive {
                adw::ResponseAppearance::Destructive
            } else {
                adw::ResponseAppearance::Suggested
            },
        );
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |dialog, response| {
            if response == "confirm"
                && let Some(controller) = weak.upgrade()
            {
                action(&controller);
            }
            dialog.close();
        });
        dialog.present();
    }

    fn show_error(&self, message: &str) {
        self.show_message(&self.locale().text("appTitle"), message, true);
    }

    fn show_notice(&self, message: &str) {
        self.show_message(&self.locale().text("appTitle"), message, false);
    }

    fn show_message(&self, heading: &str, message: &str, error: bool) {
        let dialog = adw::MessageDialog::new(Some(&self.window), Some(heading), Some(message));
        dialog.add_response("close", &self.locale().text("close"));
        if error {
            dialog.set_response_appearance("close", adw::ResponseAppearance::Destructive);
        }
        dialog.set_default_response(Some("close"));
        dialog.set_close_response("close");
        dialog.connect_response(None, |dialog, _| dialog.close());
        dialog.present();
    }
}

impl Controller {
    fn show_settings(self: &Rc<Self>) {
        let locale = self.locale();
        let settings = adw::PreferencesWindow::builder()
            .transient_for(&self.window)
            .modal(true)
            .title(locale.text("settings"))
            .default_width(640)
            .default_height(680)
            .build();

        let general = adw::PreferencesPage::new();
        general.set_title(&locale.text("general"));
        general.set_icon_name(Some("preferences-system-symbolic"));
        let general_group = adw::PreferencesGroup::new();
        general_group.set_title(&locale.text("general"));

        let language_model = gtk::StringList::new(&["English", "简体中文"]);
        let language = adw::ComboRow::builder()
            .title(locale.text("language"))
            .model(&language_model)
            .selected(u32::from(self.state.borrow().config.locale == "zh-CN"))
            .build();
        let weak = Rc::downgrade(self);
        language.connect_selected_notify(move |row| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            let code = if row.selected() == 1 { "zh-CN" } else { "en" };
            if controller.state.borrow().config.locale == code {
                return;
            }
            {
                let mut state = controller.state.borrow_mut();
                state.config.locale = code.to_string();
                let _ = state.config.save();
            }
            let application = controller.application.clone();
            build_and_retain_controller(&application);
            controller.window.close();
        });
        general_group.add(&language);

        let appearance_model = gtk::StringList::new(&[
            &locale.text("followSystem"),
            &locale.text("lightMode"),
            &locale.text("darkMode"),
        ]);
        let appearance = adw::ComboRow::builder()
            .title(locale.text("appearance"))
            .model(&appearance_model)
            .selected(match self.state.borrow().config.appearance {
                Appearance::System => 0,
                Appearance::Light => 1,
                Appearance::Dark => 2,
            })
            .build();
        let weak = Rc::downgrade(self);
        appearance.connect_selected_notify(move |row| {
            if let Some(controller) = weak.upgrade() {
                let appearance = match row.selected() {
                    1 => Appearance::Light,
                    2 => Appearance::Dark,
                    _ => Appearance::System,
                };
                controller.state.borrow_mut().config.appearance = appearance;
                apply_appearance(appearance);
                let _ = controller.state.borrow().config.save();
            }
        });
        general_group.add(&appearance);
        general.add(&general_group);

        let storage_group = adw::PreferencesGroup::new();
        storage_group.set_title(&locale.text("storage"));
        let results = adw::ActionRow::builder()
            .title(locale.text("resultDirectory"))
            .subtitle(
                self.state
                    .borrow()
                    .config
                    .results_root
                    .display()
                    .to_string(),
            )
            .activatable(true)
            .build();
        results.add_suffix(&gtk::Image::from_icon_name("folder-symbolic"));
        let weak = Rc::downgrade(self);
        results.connect_activated(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.choose_result_directory();
            }
        });
        storage_group.add(&results);
        let reject = adw::ActionRow::builder()
            .title(locale.text("defaultMoveDestination"))
            .subtitle(
                self.state
                    .borrow()
                    .config
                    .reject_destination
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| locale.text("insideRunFolder")),
            )
            .activatable(true)
            .build();
        reject.add_suffix(&gtk::Image::from_icon_name("folder-symbolic"));
        let weak = Rc::downgrade(self);
        reject.connect_activated(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.choose_reject_destination();
            }
        });
        storage_group.add(&reject);
        let cache = adw::ActionRow::builder()
            .title(locale.text("cacheCleanup"))
            .subtitle(format_cache_size(&self.state.borrow().config.recent_runs))
            .activatable(true)
            .build();
        cache.add_suffix(&gtk::Image::from_icon_name("user-trash-symbolic"));
        let weak = Rc::downgrade(self);
        cache.connect_activated(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.show_cache_cleanup();
            }
        });
        storage_group.add(&cache);
        general.add(&storage_group);
        settings.add(&general);

        let analysis = adw::PreferencesPage::new();
        analysis.set_title(&locale.text("analysis"));
        analysis.set_icon_name(Some("applications-graphics-symbolic"));
        let workload_bar = gtk::ProgressBar::new();
        workload_bar.set_size_request(150, 5);
        workload_bar.set_valign(gtk::Align::Center);
        workload_bar.add_css_class("workload-bar");
        let workload_row = adw::ActionRow::builder()
            .title(locale.text("estimatedSystemLoad"))
            .build();
        workload_row.add_suffix(&workload_bar);
        refresh_workload(self, &workload_bar, &workload_row);

        let quality_group = adw::PreferencesGroup::new();
        quality_group.set_title(&locale.text("quality"));
        let preset_model = gtk::StringList::new(&[
            &locale.text("fastPreset"),
            &locale.text("balancedPreset"),
            &locale.text("bestQualityPreset"),
            &locale.text("customPreset"),
        ]);
        let preset = adw::ComboRow::builder()
            .title(locale.text("qualityPreset"))
            .model(&preset_model)
            .selected(preset_index(&self.state.borrow().config.options))
            .build();
        let weak = Rc::downgrade(self);
        let workload_for_preset = workload_bar.clone();
        let workload_row_for_preset = workload_row.clone();
        preset.connect_selected_notify(move |row| {
            if let Some(controller) = weak.upgrade() {
                apply_preset(
                    &mut controller.state.borrow_mut().config.options,
                    row.selected(),
                );
                let _ = controller.state.borrow().config.save();
                refresh_workload(&controller, &workload_for_preset, &workload_row_for_preset);
            }
        });
        quality_group.add(&preset);
        add_numeric_setting(
            self,
            &quality_group,
            NumericSetting {
                title: locale.text("previewSize"),
                value: self.state.borrow().config.options.preview_size,
                minimum: 512.0,
                maximum: 4096.0,
                step: 128.0,
            },
            &workload_bar,
            &workload_row,
            |options, value| options.preview_size = value,
        );
        add_numeric_setting(
            self,
            &quality_group,
            NumericSetting {
                title: locale.text("refineSize"),
                value: self.state.borrow().config.options.refine_size,
                minimum: 1024.0,
                maximum: 8192.0,
                step: 256.0,
            },
            &workload_bar,
            &workload_row,
            |options, value| options.refine_size = value,
        );
        add_numeric_setting(
            self,
            &quality_group,
            NumericSetting {
                title: locale.text("refineCandidates"),
                value: self
                    .state
                    .borrow()
                    .config
                    .options
                    .refine_candidates_per_cluster as u32,
                minimum: 1.0,
                maximum: 8.0,
                step: 1.0,
            },
            &workload_bar,
            &workload_row,
            |options, value| options.refine_candidates_per_cluster = value as usize,
        );
        let refinement = adw::SwitchRow::builder()
            .title(locale.text("highResolutionRefinement"))
            .active(!self.state.borrow().config.options.disable_refinement)
            .build();
        let weak = Rc::downgrade(self);
        let workload_for_refinement = workload_bar.clone();
        let workload_row_for_refinement = workload_row.clone();
        refinement.connect_active_notify(move |row| {
            if let Some(controller) = weak.upgrade() {
                controller
                    .state
                    .borrow_mut()
                    .config
                    .options
                    .disable_refinement = !row.is_active();
                let _ = controller.state.borrow().config.save();
                refresh_workload(
                    &controller,
                    &workload_for_refinement,
                    &workload_row_for_refinement,
                );
            }
        });
        quality_group.add(&refinement);
        analysis.add(&quality_group);

        let processing_group = adw::PreferencesGroup::new();
        processing_group.set_title(&locale.text("processing"));

        let acceleration_choices = acceleration_choices(&locale);
        let acceleration_model = gtk::StringList::new(
            &acceleration_choices
                .iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>(),
        );
        let current_acceleration = self.state.borrow().config.options.acceleration.canonical();
        let acceleration = adw::ComboRow::builder()
            .title(locale.text("acceleration"))
            .model(&acceleration_model)
            .selected(
                acceleration_choices
                    .iter()
                    .position(|(_, value)| *value == current_acceleration)
                    .unwrap_or_default() as u32,
            )
            .build();
        let choices = Rc::new(acceleration_choices);
        let weak = Rc::downgrade(self);
        let workload_for_acceleration = workload_bar.clone();
        let workload_row_for_acceleration = workload_row.clone();
        acceleration.connect_selected_notify(move |row| {
            if let Some(controller) = weak.upgrade()
                && let Some((_, value)) = choices.get(row.selected() as usize)
            {
                controller.state.borrow_mut().config.options.acceleration = *value;
                let _ = controller.state.borrow().config.save();
                refresh_workload(
                    &controller,
                    &workload_for_acceleration,
                    &workload_row_for_acceleration,
                );
            }
        });
        processing_group.add(&acceleration);

        let detector_choices = [
            (locale.text("automaticOption"), DetectorPreference::Auto),
            (
                locale.text("heuristicOption"),
                DetectorPreference::Heuristic,
            ),
            (locale.text("mlOption"), DetectorPreference::Ml),
            (locale.text("offOption"), DetectorPreference::Off),
        ];
        let detector_model = gtk::StringList::new(
            &detector_choices
                .iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>(),
        );
        let current_detector = self.state.borrow().config.options.detector.canonical();
        let detector = adw::ComboRow::builder()
            .title(locale.text("detector"))
            .model(&detector_model)
            .selected(
                detector_choices
                    .iter()
                    .position(|(_, value)| *value == current_detector)
                    .unwrap_or_default() as u32,
            )
            .build();

        let detector_model_choices = [
            (
                locale.text("fastModelOption"),
                DetectorModelPreference::Fast,
            ),
            (
                locale.text("accurateModelOption"),
                DetectorModelPreference::Accurate,
            ),
        ];
        let detector_model_list = gtk::StringList::new(
            &detector_model_choices
                .iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>(),
        );
        let current_model = self.state.borrow().config.options.detector_model;
        let detector_model = adw::ComboRow::builder()
            .title(locale.text("detectorModel"))
            .model(&detector_model_list)
            .selected(
                detector_model_choices
                    .iter()
                    .position(|(_, value)| *value == current_model)
                    .unwrap_or_default() as u32,
            )
            .visible(current_detector == DetectorPreference::Ml)
            .build();

        let mut detector_device_choices = vec![
            (
                locale.text("automaticOption"),
                DetectorDevicePreference::Auto,
            ),
            (locale.text("cpuOption"), DetectorDevicePreference::Cpu),
        ];
        if cfg!(feature = "cuda-accel") {
            detector_device_choices.push((locale.text("gpuOption"), DetectorDevicePreference::Gpu));
        }
        let detector_device_list = gtk::StringList::new(
            &detector_device_choices
                .iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>(),
        );
        let current_device = self
            .state
            .borrow()
            .config
            .options
            .detector_device
            .canonical();
        let detector_device = adw::ComboRow::builder()
            .title(locale.text("inferenceDevice"))
            .model(&detector_device_list)
            .selected(
                detector_device_choices
                    .iter()
                    .position(|(_, value)| *value == current_device)
                    .unwrap_or_default() as u32,
            )
            .visible(current_detector == DetectorPreference::Ml)
            .build();

        let choices = Rc::new(detector_choices);
        let weak = Rc::downgrade(self);
        let workload_for_detector = workload_bar.clone();
        let workload_row_for_detector = workload_row.clone();
        let model_for_detector = detector_model.clone();
        let device_for_detector = detector_device.clone();
        detector.connect_selected_notify(move |row| {
            if let Some(controller) = weak.upgrade()
                && let Some((_, value)) = choices.get(row.selected() as usize)
            {
                controller.state.borrow_mut().config.options.detector = *value;
                let show_ml = *value == DetectorPreference::Ml;
                model_for_detector.set_visible(show_ml);
                device_for_detector.set_visible(show_ml);
                let _ = controller.state.borrow().config.save();
                refresh_workload(
                    &controller,
                    &workload_for_detector,
                    &workload_row_for_detector,
                );
            }
        });

        let model_choices = Rc::new(detector_model_choices);
        let weak = Rc::downgrade(self);
        let workload_for_model = workload_bar.clone();
        let workload_row_for_model = workload_row.clone();
        detector_model.connect_selected_notify(move |row| {
            if let Some(controller) = weak.upgrade()
                && let Some((_, value)) = model_choices.get(row.selected() as usize)
            {
                controller.state.borrow_mut().config.options.detector_model = *value;
                let _ = controller.state.borrow().config.save();
                refresh_workload(&controller, &workload_for_model, &workload_row_for_model);
            }
        });

        let device_choices = Rc::new(detector_device_choices);
        let weak = Rc::downgrade(self);
        detector_device.connect_selected_notify(move |row| {
            if let Some(controller) = weak.upgrade()
                && let Some((_, value)) = device_choices.get(row.selected() as usize)
            {
                controller.state.borrow_mut().config.options.detector_device = *value;
                let _ = controller.state.borrow().config.save();
            }
        });
        processing_group.add(&detector);
        processing_group.add(&detector_model);
        processing_group.add(&detector_device);

        let model_pack = adw::ActionRow::builder()
            .title(locale.text("modelPack"))
            .subtitle(
                self.state
                    .borrow()
                    .config
                    .model_pack
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| locale.text("automatic")),
            )
            .activatable(true)
            .build();
        model_pack.add_suffix(&gtk::Image::from_icon_name("folder-symbolic"));
        let weak = Rc::downgrade(self);
        model_pack.connect_activated(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.choose_model_pack();
            }
        });
        processing_group.add(&model_pack);
        analysis.add(&processing_group);

        let assessment_group = adw::PreferencesGroup::new();
        assessment_group.set_title(&locale.text("deviceAssessment"));
        assessment_group.add(&workload_row);
        analysis.add(&assessment_group);
        settings.add(&analysis);
        settings.present();
    }

    fn choose_result_directory(self: &Rc<Self>) {
        let locale = self.locale();
        let initial = self.state.borrow().config.results_root.clone();
        self.choose_folder(
            &locale.text("selectResultsTitle"),
            Some(&initial),
            move |controller, new_root| {
                let current_run = controller.current_run_dir();
                if let Some(run_dir) = current_run {
                    controller.set_busy(true);
                    controller.stack.set_visible_child_name("scanning");
                    controller.progress_bar.set_fraction(0.0);
                    let events = controller.events.clone();
                    let progress_events = events.clone();
                    let new_root_for_result = new_root.clone();
                    thread::spawn(move || {
                        let result = relocate_run(&run_dir, &new_root, move |update| {
                            push_event(
                                &progress_events,
                                BackgroundEvent::RelocationProgress(update),
                            );
                        })
                        .map(|relocation| (relocation, new_root_for_result))
                        .map_err(|error| format!("{error:#}"));
                        push_event(&events, BackgroundEvent::Relocated(result));
                    });
                } else {
                    controller.state.borrow_mut().config.results_root = new_root;
                    let _ = controller.state.borrow().config.save();
                    controller.refresh_history();
                }
            },
        );
    }

    fn choose_reject_destination(self: &Rc<Self>) {
        let locale = self.locale();
        let initial = self.state.borrow().config.reject_destination.clone();
        self.choose_folder(
            &locale.text("selectMoveDestinationTitle"),
            initial.as_deref(),
            move |controller, path| {
                controller.state.borrow_mut().config.reject_destination = Some(path);
                let _ = controller.state.borrow().config.save();
            },
        );
    }

    fn choose_model_pack(self: &Rc<Self>) {
        let locale = self.locale();
        let initial = self.state.borrow().config.model_pack.clone();
        self.choose_folder(
            &locale.text("modelPack"),
            initial.as_deref(),
            move |controller, path| {
                controller.state.borrow_mut().config.model_pack = Some(path);
                let _ = controller.state.borrow().config.save();
            },
        );
    }

    fn show_cache_cleanup(self: &Rc<Self>) {
        let locale = self.locale();
        let dialog = adw::Window::builder()
            .transient_for(&self.window)
            .modal(true)
            .title(locale.text("chooseRunsToRemove"))
            .default_width(620)
            .default_height(520)
            .build();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new(&locale.text("chooseRunsToRemove"), "");
        header.set_title_widget(Some(&title));
        toolbar.add_top_bar(&header);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(18);
        content.set_margin_end(18);
        let warning = left_label(&locale.text("cacheScopeDetail"));
        warning.set_wrap(true);
        warning.add_css_class("muted");
        content.append(&warning);
        let checks = Rc::new(RefCell::new(Vec::<(gtk::CheckButton, PathBuf)>::new()));
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::None);
        for path in self
            .state
            .borrow()
            .config
            .recent_runs
            .iter()
            .filter(|path| path.is_dir())
        {
            let check = gtk::CheckButton::with_label(&format!(
                "{}  ·  {}",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default(),
                format_bytes(cache_bytes(path))
            ));
            check.set_active(true);
            check.set_margin_top(9);
            check.set_margin_bottom(9);
            check.set_margin_start(10);
            check.set_margin_end(10);
            list.append(&check);
            checks.borrow_mut().push((check, path.clone()));
        }
        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();
        content.append(&scroller);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label(&locale.text("cancel"));
        let remove = gtk::Button::with_label(&locale.text("removeSelectedRuns"));
        remove.add_css_class("destructive-action");
        buttons.append(&cancel);
        buttons.append(&remove);
        content.append(&buttons);
        toolbar.set_content(Some(&content));
        dialog.set_content(Some(&toolbar));
        let dialog_for_cancel = dialog.clone();
        cancel.connect_clicked(move |_| dialog_for_cancel.close());
        let weak = Rc::downgrade(self);
        let dialog_for_remove = dialog.clone();
        remove.connect_clicked(move |_| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            let selected: Vec<_> = checks
                .borrow()
                .iter()
                .filter(|(check, _)| check.is_active())
                .map(|(_, path)| path.clone())
                .collect();
            if selected.is_empty() {
                return;
            }
            dialog_for_remove.close();
            let locale = controller.locale();
            controller.confirm(
                &locale.text("removeCacheTitle"),
                &locale.text("removeCacheMovedMessage"),
                &locale.text("removeSelectedRuns"),
                true,
                move |controller| {
                    controller.set_busy(true);
                    let events = controller.events.clone();
                    let selected = selected.clone();
                    thread::spawn(move || {
                        let result = (|| -> anyhow::Result<usize> {
                            for path in &selected {
                                std::fs::remove_dir_all(path).with_context(|| {
                                    format!("removing run folder {}", path.display())
                                })?;
                            }
                            Ok(selected.len())
                        })()
                        .map_err(|error| format!("{error:#}"));
                        push_event(&events, BackgroundEvent::CacheRemoved(result));
                    });
                },
            );
        });
        dialog.present();
    }

    fn show_tutorial(self: &Rc<Self>, reopened: bool) {
        let locale = self.locale();
        let dialog = adw::Window::builder()
            .transient_for(&self.window)
            .modal(true)
            .title(locale.text("tutorialTitle"))
            .default_width(760)
            .default_height(560)
            .build();
        dialog.set_resizable(false);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);
        let window_title = adw::WindowTitle::new(&locale.text("tutorialTitle"), "1 / 4");
        header.set_title_widget(Some(&window_title));
        root.append(&header);
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::SlideLeftRight)
            .vexpand(true)
            .build();
        for (index, (title_key, body_key)) in [
            ("tutorialScanTitle", "tutorialScanBody"),
            ("tutorialSuggestionsTitle", "tutorialSuggestionsBody"),
            ("tutorialInspectTitle", "tutorialInspectBody"),
            ("tutorialMoveTitle", "tutorialMoveBody"),
        ]
        .into_iter()
        .enumerate()
        {
            let page = tutorial_page(&locale, title_key, body_key, index);
            stack.add_named(&page, Some(&format!("step-{index}")));
        }
        root.append(&stack);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(12);
        actions.set_margin_bottom(16);
        actions.set_margin_start(18);
        actions.set_margin_end(18);
        let skip = gtk::Button::with_label(&locale.text("tutorialSkip"));
        let back = gtk::Button::with_label(&locale.text("tutorialBack"));
        let next = gtk::Button::with_label(&locale.text("tutorialNext"));
        next.add_css_class("suggested-action");
        back.set_sensitive(false);
        actions.append(&skip);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        actions.append(&spacer);
        actions.append(&back);
        actions.append(&next);
        root.append(&actions);
        dialog.set_content(Some(&root));

        let index = Rc::new(RefCell::new(0usize));
        let dialog_for_skip = dialog.clone();
        let weak = Rc::downgrade(self);
        skip.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.finish_tutorial(TutorialOutcome::Skipped);
            }
            dialog_for_skip.close();
        });
        let index_for_back = index.clone();
        let stack_for_back = stack.clone();
        let back_for_back = back.clone();
        let next_for_back = next.clone();
        let title_for_back = window_title.clone();
        let locale_for_back = locale.clone();
        back.connect_clicked(move |_| {
            let mut index = index_for_back.borrow_mut();
            *index = index.saturating_sub(1);
            update_tutorial_controls(
                *index,
                &stack_for_back,
                &back_for_back,
                &next_for_back,
                &title_for_back,
                &locale_for_back,
            );
        });
        let index_for_next = index.clone();
        let stack_for_next = stack.clone();
        let back_for_next = back.clone();
        let next_for_next = next.clone();
        let title_for_next = window_title.clone();
        let locale_for_next = locale.clone();
        let dialog_for_next = dialog.clone();
        let weak = Rc::downgrade(self);
        next.connect_clicked(move |_| {
            let mut index = index_for_next.borrow_mut();
            if *index == 3 {
                if let Some(controller) = weak.upgrade() {
                    controller.finish_tutorial(TutorialOutcome::Completed);
                }
                dialog_for_next.close();
                return;
            }
            *index += 1;
            update_tutorial_controls(
                *index,
                &stack_for_next,
                &back_for_next,
                &next_for_next,
                &title_for_next,
                &locale_for_next,
            );
        });
        if reopened {
            dialog.set_title(Some(&locale.text("tutorialMenu")));
        }
        dialog.present();
    }

    fn finish_tutorial(&self, outcome: TutorialOutcome) {
        self.state.borrow_mut().config.record_tutorial(outcome);
        let _ = self.state.borrow().config.save();
    }

    fn show_about(&self) {
        let locale = self.locale();
        let dialog = adw::Window::builder()
            .transient_for(&self.window)
            .modal(true)
            .title(locale.text("aboutTitle"))
            .default_width(620)
            .default_height(560)
            .build();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new(&locale.text("aboutTitle"), "");
        header.set_title_widget(Some(&title));
        toolbar.add_top_bar(&header);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.set_margin_top(28);
        content.set_margin_bottom(28);
        content.set_margin_start(32);
        content.set_margin_end(32);
        let icon = gtk::Image::from_icon_name(APP_ICON);
        icon.set_pixel_size(72);
        let name = gtk::Label::new(Some(&locale.text("appTitle")));
        name.add_css_class("welcome-title");
        let description = gtk::Label::new(Some(&locale.text("aboutDescription")));
        description.set_wrap(true);
        let version = gtk::Label::new(Some(&locale.format(
            "versionValue",
            &[("version", env!("CARGO_PKG_VERSION").to_string())],
        )));
        version.add_css_class("muted");
        let link = gtk::LinkButton::with_label(
            "https://github.com/pan2013e/burst-frame-deduplicator",
            "GitHub",
        );
        let diagnostics = gtk::Expander::builder()
            .label(locale.text("systemDiagnostics"))
            .child(&diagnostics_label(&locale))
            .build();
        content.append(&icon);
        content.append(&name);
        content.append(&description);
        content.append(&version);
        content.append(&link);
        content.append(&diagnostics);
        toolbar.set_content(Some(&content));
        dialog.set_content(Some(&toolbar));
        dialog.present();
    }

    fn open_preview(self: &Rc<Self>, asset_id: &str) {
        let Some(payload) = self.state.borrow().payload.clone() else {
            return;
        };
        let weak = Rc::downgrade(self);
        preview::open(
            &self.window,
            self.locale(),
            payload,
            asset_id,
            move |asset_id, decision| {
                if let Some(controller) = weak.upgrade() {
                    controller.persist_decision(asset_id, Some(decision));
                }
            },
        );
    }
}

fn tutorial_page(
    locale: &LocaleCatalog,
    title_key: &str,
    body_key: &str,
    index: usize,
) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
    page.set_margin_top(34);
    page.set_margin_bottom(24);
    page.set_margin_start(48);
    page.set_margin_end(48);
    let title = left_label(&locale.text(title_key));
    title.add_css_class("welcome-title");
    let body = left_label(&locale.text(body_key));
    body.set_wrap(true);
    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    body.set_max_width_chars(62);
    body.add_css_class("welcome-subtitle");
    page.append(&title);
    page.append(&body);
    let demo = gtk::Box::new(gtk::Orientation::Vertical, 10);
    demo.add_css_class("tutorial-demo");
    match index {
        0 => {
            let progress = gtk::ProgressBar::new();
            progress.set_fraction(0.64);
            demo.append(&left_label(&locale.text("tutorialDemoSource")));
            demo.append(&progress);
        }
        1 | 2 => {
            for (label, active, inconsistent) in [
                (locale.text("tutorialDemoKeep"), true, false),
                (locale.text("tutorialDemoReject"), false, false),
                (locale.text("tutorialDemoReview"), false, true),
            ] {
                let check = gtk::CheckButton::with_label(&label);
                check.set_active(active);
                check.set_inconsistent(inconsistent);
                demo.append(&check);
            }
        }
        _ => {
            demo.append(&left_label(&locale.text("moveConfirmMessage")));
        }
    }
    page.append(&demo);
    page.upcast()
}

fn update_tutorial_controls(
    index: usize,
    stack: &gtk::Stack,
    back: &gtk::Button,
    next: &gtk::Button,
    title: &adw::WindowTitle,
    locale: &LocaleCatalog,
) {
    stack.set_visible_child_name(&format!("step-{index}"));
    back.set_sensitive(index > 0);
    next.set_label(&locale.text(if index == 3 {
        "tutorialDone"
    } else {
        "tutorialNext"
    }));
    title.set_subtitle(&format!("{} / 4", index + 1));
}

fn diagnostics_label(locale: &LocaleCatalog) -> gtk::Label {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|value| value.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| std::env::consts::OS.to_string());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".to_string());
    let memory = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|line| line.starts_with("MemTotal:"))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "MemTotal: unavailable".to_string());
    let text = format!(
        "{}: {}\n{}: {}\n{}: {}\n{}: {}.{}.{}\n{}: {}.{}.{}\n{}\n{}: {}",
        locale.text("operatingSystem"),
        os,
        locale.text("architecture"),
        std::env::consts::ARCH,
        locale.text("desktop"),
        desktop,
        locale.text("gtkVersion"),
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version(),
        locale.text("libadwaitaVersion"),
        adw::major_version(),
        adw::minor_version(),
        adw::micro_version(),
        memory,
        locale.text("builtCommit"),
        option_env!("BFD_BUILD_COMMIT").unwrap_or("development")
    );
    let label = left_label(&text);
    label.set_selectable(true);
    label.set_wrap(true);
    label.add_css_class("monospace");
    label
}
struct NumericSetting {
    title: String,
    value: u32,
    minimum: f64,
    maximum: f64,
    step: f64,
}

fn add_numeric_setting(
    controller: &Rc<Controller>,
    group: &adw::PreferencesGroup,
    setting: NumericSetting,
    workload: &gtk::ProgressBar,
    workload_row: &adw::ActionRow,
    update: impl Fn(&mut crate::types::ScanOptions, u32) + 'static,
) {
    let row = adw::ActionRow::builder().title(&setting.title).build();
    let adjustment = gtk::Adjustment::new(
        f64::from(setting.value),
        setting.minimum,
        setting.maximum,
        setting.step,
        setting.step * 2.0,
        0.0,
    );
    let spin = gtk::SpinButton::new(Some(&adjustment), setting.step, 0);
    spin.set_valign(gtk::Align::Center);
    let weak = Rc::downgrade(controller);
    let workload = workload.clone();
    let workload_row = workload_row.clone();
    spin.connect_value_changed(move |spin| {
        if let Some(controller) = weak.upgrade() {
            update(
                &mut controller.state.borrow_mut().config.options,
                spin.value_as_int().max(0) as u32,
            );
            let _ = controller.state.borrow().config.save();
            refresh_workload(&controller, &workload, &workload_row);
        }
    });
    row.add_suffix(&spin);
    group.add(&row);
}

fn refresh_workload(controller: &Controller, bar: &gtk::ProgressBar, row: &adw::ActionRow) {
    let (workload, capability) = estimated_workload(&controller.state.borrow().config.options);
    bar.set_fraction(workload);
    row.set_subtitle(&capability);
}

fn acceleration_choices(locale: &LocaleCatalog) -> Vec<(String, AccelerationPreference)> {
    let mut choices = vec![
        (locale.text("automaticOption"), AccelerationPreference::Auto),
        (locale.text("cpuOption"), AccelerationPreference::Cpu),
        (
            locale.text("portableCpuOption"),
            AccelerationPreference::Portable,
        ),
    ];
    if cfg!(feature = "cuda-accel") {
        choices.insert(2, (locale.text("gpuOption"), AccelerationPreference::Gpu));
    }
    choices
}

fn preset_index(options: &crate::types::ScanOptions) -> u32 {
    if options.preview_size == 960
        && options.refine_size == 1536
        && options.refine_candidates_per_cluster == 1
        && options.max_duplicate_distance == 0.20
        && options.min_duplicate_confidence == 0.52
        && !options.disable_refinement
    {
        0
    } else if options.preview_size == 1280
        && options.refine_size == 2048
        && options.refine_candidates_per_cluster == 2
        && options.max_duplicate_distance == 0.20
        && options.min_duplicate_confidence == 0.52
        && !options.disable_refinement
    {
        1
    } else if options.preview_size == 2048
        && options.refine_size == 4096
        && options.refine_candidates_per_cluster == 4
        && options.max_duplicate_distance == 0.20
        && options.min_duplicate_confidence == 0.60
        && !options.disable_refinement
    {
        2
    } else {
        3
    }
}

fn apply_preset(options: &mut crate::types::ScanOptions, selected: u32) {
    match selected {
        0 => {
            options.preview_size = 960;
            options.refine_size = 1536;
            options.refine_candidates_per_cluster = 1;
            options.max_duplicate_distance = 0.20;
            options.min_duplicate_confidence = 0.52;
            options.disable_refinement = false;
        }
        1 => {
            options.preview_size = 1280;
            options.refine_size = 2048;
            options.refine_candidates_per_cluster = 2;
            options.max_duplicate_distance = 0.20;
            options.min_duplicate_confidence = 0.52;
            options.disable_refinement = false;
        }
        2 => {
            options.preview_size = 2048;
            options.refine_size = 4096;
            options.refine_candidates_per_cluster = 4;
            options.max_duplicate_distance = 0.20;
            options.min_duplicate_confidence = 0.60;
            options.disable_refinement = false;
        }
        _ => {}
    }
}

fn format_cache_size(runs: &[PathBuf]) -> String {
    let bytes: u64 = runs
        .iter()
        .filter(|path| path.is_dir())
        .map(|path| cache_bytes(path))
        .sum();
    format_bytes(bytes)
}

fn estimated_workload(options: &crate::types::ScanOptions) -> (f64, String) {
    let cores = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    let memory_gib = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("MemTotal:")
                    .and_then(|value| value.split_whitespace().next())
                    .and_then(|value| value.parse::<f64>().ok())
                    .map(|kib| kib / 1024.0 / 1024.0)
            })
        })
        .unwrap_or(4.0);
    let pixels = (f64::from(options.preview_size) / 1280.0).powi(2);
    let refinement = if options.disable_refinement {
        0.0
    } else {
        (f64::from(options.refine_size) / 2048.0).powi(2)
            * options.refine_candidates_per_cluster as f64
            / 2.0
    };
    let detector = match options.detector.canonical() {
        DetectorPreference::Ml if options.detector_model == DetectorModelPreference::Accurate => {
            1.8
        }
        DetectorPreference::Ml => 1.1,
        DetectorPreference::Heuristic | DetectorPreference::Auto => 0.45,
        _ => 0.1,
    };
    let capacity = (cores as f64 / 8.0).clamp(0.35, 2.0) * (memory_gib / 8.0).clamp(0.35, 2.0);
    let load = ((pixels * 0.42 + refinement * 0.40 + detector * 0.18) / capacity).clamp(0.0, 1.0);
    (
        load,
        format!(
            "{cores} CPU · {memory_gib:.1} GiB · {}",
            std::env::consts::ARCH
        ),
    )
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

fn flattened_review_entries(
    payload: &ReviewPayload,
    expanded: &HashSet<usize>,
    filter: ReviewFilter,
) -> Vec<ReviewListEntry> {
    let assets: HashMap<_, _> = payload
        .manifest
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect();
    let mut clusters = payload.manifest.clusters.clone();
    clusters.sort_by_key(|cluster| (!expanded.contains(&cluster.id), cluster.id));
    let mut entries = Vec::new();
    for cluster in clusters {
        let is_expanded = expanded.contains(&cluster.id);
        entries.push(ReviewListEntry::Cluster {
            id: cluster.id,
            burst_id: cluster.burst_id,
            frame_count: cluster.asset_ids.len(),
            keep_count: cluster.keep_count,
            confidence: cluster.similarity_confidence,
            expanded: is_expanded,
        });
        if !is_expanded {
            continue;
        }
        let cluster_assets: Vec<_> = cluster
            .asset_ids
            .iter()
            .filter_map(|id| assets.get(id.as_str()).copied())
            .collect();
        let exif_differs = exif_varies(&cluster_assets);
        for asset in cluster_assets {
            let action = final_action_for_asset(asset, &payload.review);
            let visible = match filter {
                ReviewFilter::All => true,
                ReviewFilter::Keep => action == UserDecision::Keep,
                ReviewFilter::Reject => action == UserDecision::Reject,
                ReviewFilter::Review => action == UserDecision::Review,
            };
            if visible {
                entries.push(ReviewListEntry::Asset {
                    asset: Box::new(asset.clone()),
                    exif_differs,
                });
            }
        }
    }
    entries
}

fn exif_varies(assets: &[&AssetRecord]) -> bool {
    let mut values = HashSet::new();
    for asset in assets {
        values.insert(format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            asset.metadata.iso,
            asset.metadata.aperture,
            asset.metadata.shutter,
            asset.metadata.focal_length_mm,
            asset.metadata.focal_length_35mm
        ));
    }
    values.len() > 1
}

fn decision_counts(payload: &ReviewPayload) -> (usize, usize, usize) {
    let mut keep = 0;
    let mut reject = 0;
    let mut review = 0;
    for asset in &payload.manifest.assets {
        match final_action_for_asset(asset, &payload.review) {
            UserDecision::Keep => keep += 1,
            UserDecision::Reject => reject += 1,
            UserDecision::Review => review += 1,
        }
    }
    (keep, reject, review)
}

fn suggestion_reason(locale: &LocaleCatalog, asset: &AssetRecord) -> String {
    match asset.suggestion.action {
        SuggestedAction::Keep if asset.similarity.duplicate_confidence < 0.5 => {
            locale.text("distinctFrame")
        }
        SuggestedAction::Keep => locale.format(
            "rankDetail",
            &[
                ("rank", asset.suggestion.rank.to_string()),
                ("score", format!("{:.2}", asset.suggestion.score)),
            ],
        ),
        SuggestedAction::Reject => locale.text("duplicate"),
        SuggestedAction::Review => locale.text("uncertainSimilarity"),
        SuggestedAction::Error => asset
            .error
            .clone()
            .unwrap_or_else(|| locale.text("decodeError")),
    }
}

fn exif_summary(locale: &LocaleCatalog, asset: &AssetRecord) -> String {
    let mut values = Vec::new();
    if let Some(iso) = asset.metadata.iso {
        values.push(locale.format("isoValue", &[("value", iso.to_string())]));
    }
    if let Some(aperture) = asset.metadata.aperture {
        values.push(locale.format("apertureValue", &[("value", format!("{aperture:.1}"))]));
    }
    if let Some(shutter) = &asset.metadata.shutter {
        values.push(shutter.clone());
    }
    if let Some(focal) = asset.metadata.focal_length_mm {
        values.push(locale.format("focalValue", &[("value", format!("{focal:.1}"))]));
    }
    if let Some(focal) = asset.metadata.focal_length_35mm {
        values.push(locale.format("equivalentFocalValue", &[("value", focal.to_string())]));
    }
    if values.is_empty() {
        locale.text("exifUnavailable")
    } else {
        values.join("  ·  ")
    }
}

fn detail_label(locale: &LocaleCatalog, asset: &AssetRecord) -> gtk::Label {
    let text = [
        locale.format(
            "sharpnessDetail",
            &[
                ("whole", format!("{:.1}", asset.metrics.sharpness)),
                ("subject", format!("{:.1}", asset.metrics.subject_sharpness)),
            ],
        ),
        locale.format(
            "similarityDetail",
            &[
                (
                    "distance",
                    format!("{:.3}", asset.similarity.nearest_distance),
                ),
                (
                    "confidence",
                    format!("{:.2}", asset.similarity.duplicate_confidence),
                ),
            ],
        ),
        locale.format(
            "completenessDetail",
            &[
                ("completeness", format!("{:.2}", asset.metrics.completeness)),
                ("exposure", format!("{:.2}", asset.metrics.exposure_score)),
            ],
        ),
    ]
    .join("\n");
    let label = left_label(&text);
    label.set_wrap(true);
    label.add_css_class("muted");
    label
}

fn stat_row(label: &str, value: usize) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = left_label(label);
    label.set_hexpand(true);
    let value = gtk::Label::new(Some(&value.to_string()));
    row.append(&label);
    row.append(&value);
    row
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn clear_boxed_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn unique_run_directory(root: &Path) -> PathBuf {
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_millis())
        .unwrap_or_default();
    root.join(format!("run_{now}_{nonce:03}"))
}

fn push_event(events: &EventQueue, event: BackgroundEvent) {
    events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push_back(event);
}

fn welcome_page(
    locale: &LocaleCatalog,
    config: &GuiConfig,
) -> (
    gtk::Widget,
    gtk::ListBox,
    gtk::Label,
    gtk::Button,
    gtk::Button,
) {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 28);
    outer.set_margin_top(36);
    outer.set_margin_bottom(36);
    outer.set_margin_start(42);
    outer.set_margin_end(42);
    outer.set_halign(gtk::Align::Center);
    outer.set_size_request(920, -1);

    let identity = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let icon = gtk::Image::from_icon_name("camera-photo-symbolic");
    icon.set_pixel_size(42);
    identity.append(&icon);
    let identity_text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let title = left_label(&locale.text("appTitle"));
    title.add_css_class("welcome-title");
    let subtitle = left_label(&locale.text("getStartedSubtitle"));
    subtitle.add_css_class("welcome-subtitle");
    identity_text.append(&title);
    identity_text.append(&subtitle);
    identity.append(&identity_text);
    outer.append(&identity);

    let quick = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let quick_title = left_label(&locale.text("quickStart"));
    quick_title.add_css_class("section-title");
    let new_scan = gtk::Button::builder()
        .label(locale.text("newScan"))
        .icon_name("list-add-symbolic")
        .halign(gtk::Align::Fill)
        .height_request(48)
        .hexpand(true)
        .build();
    new_scan.add_css_class("suggested-action");
    let open_run = gtk::Button::builder()
        .label(locale.text("openRun"))
        .icon_name("folder-open-symbolic")
        .halign(gtk::Align::Fill)
        .height_request(48)
        .hexpand(true)
        .build();
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    actions.set_homogeneous(true);
    actions.append(&new_scan);
    actions.append(&open_run);
    let stored = left_label(&locale.text("resultsStoredIn"));
    stored.add_css_class("muted");
    let results_path = left_label(&config.results_root.display().to_string());
    results_path.set_hexpand(true);
    results_path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    results_path.add_css_class("muted");
    let storage = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    storage.append(&stored);
    storage.append(&results_path);
    quick.append(&quick_title);
    quick.append(&actions);
    quick.append(&storage);
    outer.append(&quick);

    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    outer.append(&separator);
    let history = gtk::Box::new(gtk::Orientation::Vertical, 10);
    history.set_hexpand(true);
    let history_title = left_label(&locale.text("runHistory"));
    history_title.add_css_class("section-title");
    let recent = gtk::ListBox::new();
    recent.add_css_class("boxed-list");
    recent.set_selection_mode(gtk::SelectionMode::None);
    history.append(&history_title);
    history.append(&recent);
    outer.append(&history);

    (outer.upcast(), recent, results_path, new_scan, open_run)
}

fn scanning_page(
    locale: &LocaleCatalog,
) -> (
    gtk::Widget,
    gtk::Label,
    gtk::ProgressBar,
    gtk::Label,
    gtk::Label,
    gtk::Button,
) {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 20);
    outer.set_margin_top(64);
    outer.set_margin_start(80);
    outer.set_margin_end(80);
    outer.set_halign(gtk::Align::Fill);
    let heading = left_label(&locale.text("analyzingPhotoFolder"));
    heading.add_css_class("welcome-title");
    let progress = gtk::ProgressBar::new();
    progress.set_show_text(false);
    let stage = left_label(&locale.text("preparing"));
    stage.add_css_class("section-title");
    let detail = left_label("");
    detail.add_css_class("muted");
    detail.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    let cancel = gtk::Button::builder()
        .label(locale.text("cancelScan"))
        .icon_name("process-stop-symbolic")
        .halign(gtk::Align::End)
        .build();
    outer.append(&heading);
    outer.append(&progress);
    outer.append(&stage);
    outer.append(&detail);
    outer.append(&cancel);
    (outer.upcast(), heading, progress, stage, detail, cancel)
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .build()
}

fn left_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .build()
}

fn apply_appearance(appearance: Appearance) {
    let scheme = match appearance {
        Appearance::System => adw::ColorScheme::Default,
        Appearance::Light => adw::ColorScheme::ForceLight,
        Appearance::Dark => adw::ColorScheme::ForceDark,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

#[cfg(test)]
mod tests {
    use super::{apply_preset, preset_index};
    use crate::types::ScanOptions;

    #[test]
    fn linux_presets_restore_all_quality_parameters() {
        let mut options = ScanOptions::default();
        apply_preset(&mut options, 2);
        assert_eq!(options.max_duplicate_distance, 0.20);
        assert_eq!(options.min_duplicate_confidence, 0.60);
        assert_eq!(options.refine_candidates_per_cluster, 4);
        assert_eq!(preset_index(&options), 2);

        apply_preset(&mut options, 1);
        assert_eq!(options.min_duplicate_confidence, 0.52);
        assert_eq!(options.refine_candidates_per_cluster, 2);
        assert_eq!(preset_index(&options), 1);
    }
}
