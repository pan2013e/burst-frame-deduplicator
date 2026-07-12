use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use adw::prelude::*;
use gtk::gdk;
use gtk::glib;

use crate::app_backend::{
    ReviewPayload, prepare_embedded_preview, prepare_preview, preview_needs_refinement,
};
use crate::decode::load_preview;
use crate::operations::final_action_for_asset;
use crate::types::{FileKind, UserDecision};

use super::locale::LocaleCatalog;

const TARGET_LONG_EDGE: u32 = 4096;
const CACHE_LIMIT_BYTES: usize = 384 * 1024 * 1024;

#[derive(Clone)]
struct DecodedImage {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl DecodedImage {
    fn bytes(&self) -> usize {
        self.pixels.len()
    }

    fn long_edge(&self) -> u32 {
        self.width.max(self.height)
    }
}

struct CacheEntry {
    key: String,
    image: DecodedImage,
}

#[derive(Default)]
struct PreviewCache {
    entries: VecDeque<CacheEntry>,
    bytes: usize,
}

static CACHE: OnceLock<Mutex<PreviewCache>> = OnceLock::new();

enum PreviewEvent {
    Loaded {
        generation: u64,
        refined: bool,
        result: Result<DecodedImage, String>,
    },
}

type PreviewEvents = Arc<Mutex<VecDeque<PreviewEvent>>>;
type DecisionHandler = Rc<dyn Fn(&str, UserDecision)>;

struct PreviewState {
    payload: ReviewPayload,
    asset_ids: Vec<String>,
    index: usize,
    image: Option<DecodedImage>,
    zoom: f64,
    fit_mode: bool,
    refined: bool,
    refining: bool,
    generation: u64,
    refinement_token: u64,
    updating_check: bool,
}

struct PreviewController {
    window: adw::Window,
    title: gtk::Label,
    keep: gtk::CheckButton,
    previous: gtk::Button,
    next: gtk::Button,
    spinner: gtk::Spinner,
    picture: gtk::Picture,
    scroller: gtk::ScrolledWindow,
    error: gtk::Label,
    locale: LocaleCatalog,
    state: RefCell<PreviewState>,
    events: PreviewEvents,
    decision: DecisionHandler,
}

thread_local! {
    static CONTROLLERS: RefCell<Vec<Rc<PreviewController>>> = const { RefCell::new(Vec::new()) };
}

pub fn open(
    parent: &adw::ApplicationWindow,
    locale: LocaleCatalog,
    payload: ReviewPayload,
    asset_id: &str,
    decision: impl Fn(&str, UserDecision) + 'static,
) {
    let Some(asset) = payload
        .manifest
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
    else {
        return;
    };
    let asset_ids = payload
        .manifest
        .clusters
        .iter()
        .find(|cluster| cluster.id == asset.cluster_id)
        .map(|cluster| cluster.asset_ids.clone())
        .unwrap_or_else(|| vec![asset_id.to_string()]);
    let index = asset_ids
        .iter()
        .position(|id| id == asset_id)
        .unwrap_or_default();

    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(false)
        .default_width(1040)
        .default_height(760)
        .title(asset.representative.rel_path.clone())
        .build();
    if let Some(application) = parent.application() {
        window.set_application(Some(&application));
    }
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    toolbar.add_css_class("preview-toolbar");
    let title = gtk::Label::new(Some(&asset.representative.rel_path));
    title.set_hexpand(true);
    title.set_halign(gtk::Align::Start);
    title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    title.add_css_class("photo-title");
    let keep = gtk::CheckButton::with_label(&locale.text("keep"));
    let previous = icon_button("go-previous-symbolic", &locale.text("previousFrame"));
    let next = icon_button("go-next-symbolic", &locale.text("nextFrame"));
    let zoom_out = icon_button("zoom-out-symbolic", &locale.text("zoomOut"));
    let zoom_in = icon_button("zoom-in-symbolic", &locale.text("zoomIn"));
    let fit = icon_button("zoom-fit-best-symbolic", &locale.text("fit"));
    let close = icon_button("window-close-symbolic", &locale.text("close"));
    let spinner = gtk::Spinner::new();
    spinner.set_size_request(18, 18);
    toolbar.append(&title);
    toolbar.append(&keep);
    toolbar.append(&spinner);
    toolbar.append(&previous);
    toolbar.append(&next);
    toolbar.append(&zoom_out);
    toolbar.append(&zoom_in);
    toolbar.append(&fit);
    toolbar.append(&close);
    root.append(&toolbar);
    root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let picture = gtk::Picture::new();
    picture.set_can_shrink(false);
    picture.set_content_fit(gtk::ContentFit::Fill);
    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&picture)
        .build();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scroller));
    let error = gtk::Label::new(None);
    error.set_wrap(true);
    error.set_halign(gtk::Align::Center);
    error.set_valign(gtk::Align::Center);
    error.set_margin_start(40);
    error.set_margin_end(40);
    error.set_visible(false);
    overlay.add_overlay(&error);
    root.append(&overlay);
    window.set_content(Some(&root));

    let controller = Rc::new(PreviewController {
        window,
        title,
        keep,
        previous,
        next,
        spinner,
        picture,
        scroller,
        error,
        locale,
        state: RefCell::new(PreviewState {
            payload,
            asset_ids,
            index,
            image: None,
            zoom: 1.0,
            fit_mode: true,
            refined: false,
            refining: false,
            generation: 0,
            refinement_token: 0,
            updating_check: false,
        }),
        events: Arc::new(Mutex::new(VecDeque::new())),
        decision: Rc::new(decision),
    });
    controller.install_handlers(&zoom_out, &zoom_in, &fit, &close);
    controller.install_event_pump();
    controller.update_navigation();
    let weak = Rc::downgrade(&controller);
    controller.window.connect_close_request(move |window| {
        if weak.upgrade().is_some() {
            CONTROLLERS.with(|controllers| {
                controllers
                    .borrow_mut()
                    .retain(|controller| controller.window != *window);
            });
        }
        glib::Propagation::Proceed
    });
    CONTROLLERS.with(|controllers| controllers.borrow_mut().push(controller.clone()));
    controller.window.present();
    controller.load_current(false);
}

impl PreviewController {
    fn install_handlers(
        self: &Rc<Self>,
        zoom_out: &gtk::Button,
        zoom_in: &gtk::Button,
        fit: &gtk::Button,
        close: &gtk::Button,
    ) {
        let weak = Rc::downgrade(self);
        self.previous.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.navigate(-1);
            }
        });
        let weak = Rc::downgrade(self);
        self.next.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.navigate(1);
            }
        });
        let weak = Rc::downgrade(self);
        zoom_out.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.zoom_by(0.8);
            }
        });
        let weak = Rc::downgrade(self);
        zoom_in.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.zoom_by(1.25);
            }
        });
        let weak = Rc::downgrade(self);
        fit.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.fit();
            }
        });
        let window = self.window.clone();
        close.connect_clicked(move |_| window.close());
        let weak = Rc::downgrade(self);
        self.keep.connect_toggled(move |check| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            let state = controller.state.borrow();
            if state.updating_check {
                return;
            }
            let Some(asset_id) = state.asset_ids.get(state.index) else {
                return;
            };
            (controller.decision)(
                asset_id,
                if check.is_active() {
                    UserDecision::Keep
                } else {
                    UserDecision::Reject
                },
            );
        });

        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(controller) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Left => {
                    controller.navigate(-1);
                    glib::Propagation::Stop
                }
                gdk::Key::Right => {
                    controller.navigate(1);
                    glib::Propagation::Stop
                }
                gdk::Key::Escape => {
                    controller.window.close();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.window.add_controller(keys);

        let zoom = gtk::GestureZoom::new();
        let starting_zoom = Rc::new(RefCell::new(1.0));
        let weak = Rc::downgrade(self);
        let starting_for_begin = starting_zoom.clone();
        zoom.connect_begin(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                *starting_for_begin.borrow_mut() = controller.state.borrow().zoom;
                controller.state.borrow_mut().fit_mode = false;
            }
        });
        let weak = Rc::downgrade(self);
        zoom.connect_scale_changed(move |_, scale| {
            if let Some(controller) = weak.upgrade() {
                controller.set_zoom(*starting_zoom.borrow() * scale);
            }
        });
        self.picture.add_controller(zoom);

        let drag = gtk::GestureDrag::new();
        let start = Rc::new(RefCell::new((0.0, 0.0)));
        let start_for_begin = start.clone();
        let horizontal = self.scroller.hadjustment();
        let vertical = self.scroller.vadjustment();
        drag.connect_drag_begin(move |_, _, _| {
            *start_for_begin.borrow_mut() = (horizontal.value(), vertical.value());
        });
        let horizontal = self.scroller.hadjustment();
        let vertical = self.scroller.vadjustment();
        drag.connect_drag_update(move |_, x, y| {
            let (start_x, start_y) = *start.borrow();
            horizontal.set_value(start_x - x);
            vertical.set_value(start_y - y);
        });
        self.picture.add_controller(drag);
    }

    fn install_event_pump(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(35), move || {
            let Some(controller) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let events: Vec<_> = controller
                .events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .drain(..)
                .collect();
            for event in events {
                controller.handle_event(event);
            }
            glib::ControlFlow::Continue
        });
    }

    fn load_current(self: &Rc<Self>, refined: bool) {
        let (run_dir, asset_id, extension, raw, generation) = {
            let mut state = self.state.borrow_mut();
            state.generation += 1;
            state.refinement_token += 1;
            state.refining = refined;
            let Some(asset_id) = state.asset_ids.get(state.index).cloned() else {
                return;
            };
            let Some(asset) = state
                .payload
                .manifest
                .assets
                .iter()
                .find(|asset| asset.id == asset_id)
            else {
                return;
            };
            (
                state.payload.run_dir.clone(),
                asset_id,
                asset.representative.extension.clone(),
                asset.representative.kind == FileKind::Raw,
                state.generation,
            )
        };
        self.spinner.start();
        self.error.set_visible(false);
        let events = self.events.clone();
        thread::spawn(move || {
            let result = (|| -> anyhow::Result<DecodedImage> {
                let preview = if raw && !refined {
                    prepare_embedded_preview(&run_dir, &asset_id)
                        .or_else(|_| prepare_preview(&run_dir, &asset_id, 2048, true))?
                } else {
                    prepare_preview(&run_dir, &asset_id, TARGET_LONG_EDGE, true)?
                };
                let decode_extension = preview
                    .path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or(&extension)
                    .to_ascii_lowercase();
                decode_image(&preview.path, &decode_extension, TARGET_LONG_EDGE)
            })()
            .map_err(|error| format!("{error:#}"));
            events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push_back(PreviewEvent::Loaded {
                    generation,
                    refined,
                    result,
                });
        });
    }

    fn handle_event(self: &Rc<Self>, event: PreviewEvent) {
        let PreviewEvent::Loaded {
            generation,
            refined,
            result,
        } = event;
        if generation != self.state.borrow().generation {
            return;
        }
        self.spinner.stop();
        match result {
            Ok(image) => {
                let (old_width, old_zoom, fit_mode) = {
                    let state = self.state.borrow();
                    (
                        state.image.as_ref().map(|image| image.width),
                        state.zoom,
                        state.fit_mode,
                    )
                };
                {
                    let mut state = self.state.borrow_mut();
                    if refined {
                        state.refined = true;
                        state.refining = false;
                        if !fit_mode && let Some(old_width) = old_width {
                            state.zoom = old_zoom * f64::from(old_width) / f64::from(image.width);
                        }
                    } else {
                        state.refined = false;
                        state.fit_mode = true;
                    }
                    state.image = Some(image.clone());
                }
                self.apply_image(&image);
                if fit_mode || !refined {
                    let weak = Rc::downgrade(self);
                    glib::idle_add_local_once(move || {
                        if let Some(controller) = weak.upgrade() {
                            controller.fit();
                        }
                    });
                } else {
                    self.apply_zoom();
                }
            }
            Err(error) => {
                self.state.borrow_mut().refining = false;
                self.error.set_text(&format!(
                    "{}\n\n{}",
                    self.locale.text("previewUnavailable"),
                    error
                ));
                self.error.set_visible(true);
            }
        }
    }

    fn apply_image(&self, image: &DecodedImage) {
        let bytes = glib::Bytes::from_owned(image.pixels.clone());
        let texture = gdk::MemoryTexture::new(
            image.width as i32,
            image.height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            image.width as usize * 4,
        );
        self.picture.set_paintable(Some(&texture));
        self.apply_zoom();
    }

    fn fit(self: &Rc<Self>) {
        let Some(image) = self.state.borrow().image.clone() else {
            return;
        };
        let width = (self.scroller.width() - 28).max(1) as f64;
        let height = (self.scroller.height() - 28).max(1) as f64;
        let zoom = (width / f64::from(image.width))
            .min(height / f64::from(image.height))
            .clamp(0.01, 1.0);
        {
            let mut state = self.state.borrow_mut();
            state.fit_mode = true;
            state.zoom = zoom;
        }
        self.apply_zoom();
        self.request_refinement_if_needed();
    }

    fn zoom_by(self: &Rc<Self>, factor: f64) {
        self.state.borrow_mut().fit_mode = false;
        let zoom = self.state.borrow().zoom * factor;
        self.set_zoom(zoom);
    }

    fn set_zoom(self: &Rc<Self>, zoom: f64) {
        let old_width = self.picture.width().max(1) as f64;
        let old_height = self.picture.height().max(1) as f64;
        let horizontal = self.scroller.hadjustment();
        let vertical = self.scroller.vadjustment();
        let center_x = (horizontal.value() + horizontal.page_size() * 0.5) / old_width;
        let center_y = (vertical.value() + vertical.page_size() * 0.5) / old_height;
        self.state.borrow_mut().zoom = zoom.clamp(0.01, 12.0);
        self.apply_zoom();
        let horizontal = self.scroller.hadjustment();
        let vertical = self.scroller.vadjustment();
        let picture = self.picture.clone();
        glib::idle_add_local_once(move || {
            horizontal
                .set_value(center_x * f64::from(picture.width()) - horizontal.page_size() * 0.5);
            vertical.set_value(center_y * f64::from(picture.height()) - vertical.page_size() * 0.5);
        });
        self.request_refinement_if_needed();
    }

    fn apply_zoom(&self) {
        let state = self.state.borrow();
        let Some(image) = &state.image else {
            return;
        };
        self.picture.set_size_request(
            (f64::from(image.width) * state.zoom)
                .round()
                .clamp(1.0, f64::from(i32::MAX)) as i32,
            (f64::from(image.height) * state.zoom)
                .round()
                .clamp(1.0, f64::from(i32::MAX)) as i32,
        );
    }

    fn request_refinement_if_needed(self: &Rc<Self>) {
        let (needs, token) = {
            let mut state = self.state.borrow_mut();
            let raw = state
                .asset_ids
                .get(state.index)
                .and_then(|id| {
                    state
                        .payload
                        .manifest
                        .assets
                        .iter()
                        .find(|asset| &asset.id == id)
                })
                .is_some_and(|asset| asset.representative.kind == FileKind::Raw);
            let needs = raw
                && !state.refined
                && !state.refining
                && state.image.as_ref().is_some_and(|image| {
                    preview_needs_refinement(
                        image.long_edge(),
                        state.zoom,
                        f64::from(self.window.scale_factor()),
                        TARGET_LONG_EDGE,
                    )
                });
            state.refinement_token += 1;
            (needs, state.refinement_token)
        };
        if !needs {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(350), move || {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            if controller.state.borrow().refinement_token == token {
                controller.load_current(true);
            }
        });
    }

    fn navigate(self: &Rc<Self>, delta: isize) {
        {
            let mut state = self.state.borrow_mut();
            let next = (state.index as isize + delta)
                .clamp(0, state.asset_ids.len().saturating_sub(1) as isize)
                as usize;
            if next == state.index {
                return;
            }
            state.index = next;
            state.image = None;
            state.refined = false;
            state.refining = false;
            state.fit_mode = true;
        }
        self.update_navigation();
        self.load_current(false);
    }

    fn update_navigation(&self) {
        let (index, count, path, action) = {
            let state = self.state.borrow();
            let Some(asset_id) = state.asset_ids.get(state.index) else {
                return;
            };
            let Some(asset) = state
                .payload
                .manifest
                .assets
                .iter()
                .find(|asset| &asset.id == asset_id)
            else {
                return;
            };
            (
                state.index,
                state.asset_ids.len(),
                asset.representative.rel_path.clone(),
                final_action_for_asset(asset, &state.payload.review),
            )
        };
        self.previous.set_sensitive(index > 0);
        self.next.set_sensitive(index + 1 < count);
        self.title.set_text(&path);
        self.window.set_title(Some(&path));
        self.state.borrow_mut().updating_check = true;
        self.keep.set_active(action == UserDecision::Keep);
        self.keep.set_inconsistent(action == UserDecision::Review);
        self.state.borrow_mut().updating_check = false;
    }
}

fn decode_image(path: &Path, extension: &str, max_long_edge: u32) -> anyhow::Result<DecodedImage> {
    let key = cache_key(path, max_long_edge);
    if let Some(image) = cache_get(&key) {
        return Ok(image);
    }
    let decoded = load_preview(path, extension, max_long_edge)?;
    let width = decoded.image.width();
    let height = decoded.image.height();
    let rgb = decoded.image.into_raw();
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for values in rgb.chunks_exact(3) {
        pixels.extend_from_slice(&[values[0], values[1], values[2], u8::MAX]);
    }
    let image = DecodedImage {
        pixels,
        width,
        height,
    };
    cache_put(key, image.clone());
    Ok(image)
}

fn cache_key(path: &Path, max_long_edge: u32) -> String {
    let metadata = std::fs::metadata(path).ok();
    format!(
        "{}#{max_long_edge}#{}#{}",
        path.display(),
        metadata
            .as_ref()
            .map(|value| value.len())
            .unwrap_or_default(),
        metadata
            .and_then(|value| value.modified().ok())
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_secs())
            .unwrap_or_default()
    )
}

fn cache_get(key: &str) -> Option<DecodedImage> {
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(PreviewCache::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let index = cache.entries.iter().position(|entry| entry.key == key)?;
    let entry = cache.entries.remove(index)?;
    let image = entry.image.clone();
    cache.entries.push_back(entry);
    Some(image)
}

fn cache_put(key: String, image: DecodedImage) {
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(PreviewCache::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(index) = cache.entries.iter().position(|entry| entry.key == key)
        && let Some(previous) = cache.entries.remove(index)
    {
        cache.bytes = cache.bytes.saturating_sub(previous.image.bytes());
    }
    cache.bytes += image.bytes();
    cache.entries.push_back(CacheEntry { key, image });
    while cache.bytes > CACHE_LIMIT_BYTES || cache.entries.len() > 8 {
        let Some(removed) = cache.entries.pop_front() else {
            break;
        };
        cache.bytes = cache.bytes.saturating_sub(removed.image.bytes());
    }
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .build()
}
