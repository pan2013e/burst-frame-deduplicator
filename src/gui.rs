use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, RichText};

use crate::pipeline::run_scan;
use crate::progress::{ProgressReporter, ProgressStage, ProgressUpdate};
use crate::types::{AccelerationPreference, DetectorPreference, ScanOptions};

pub fn run() -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 680.0])
            .with_min_inner_size([720.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Burst Frame Deduplicator",
        native_options,
        Box::new(|creation| Ok(Box::new(DesktopApp::new(creation)))),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Locale {
    English,
    SimplifiedChinese,
}

impl Locale {
    fn system_default() -> Self {
        let locale = std::env::var("LANG")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if locale.starts_with("zh") {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }
}

enum GuiMessage {
    Progress(ProgressUpdate),
    Finished(Result<PathBuf, String>),
    ServerFailed(String),
}

struct DesktopApp {
    locale: Locale,
    source: String,
    output: String,
    acceleration: AccelerationPreference,
    detector: DetectorPreference,
    preview_size: u32,
    refine_size: u32,
    scanning: bool,
    open_after_scan: bool,
    progress: Option<ProgressUpdate>,
    stage_log: VecDeque<ProgressStage>,
    sender: Sender<GuiMessage>,
    receiver: Receiver<GuiMessage>,
    run_dir: Option<PathBuf>,
    review_url: Option<String>,
    review_server_stop: Option<tokio::sync::oneshot::Sender<()>>,
    error: Option<String>,
}

impl DesktopApp {
    fn new(creation: &eframe::CreationContext<'_>) -> Self {
        install_cjk_font(&creation.egui_ctx);
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::from_rgb(246, 247, 244);
        visuals.window_corner_radius = 6.0.into();
        creation.egui_ctx.set_visuals(visuals);
        let (sender, receiver) = mpsc::channel();
        Self {
            locale: Locale::system_default(),
            source: String::new(),
            output: String::new(),
            acceleration: AccelerationPreference::Auto,
            detector: DetectorPreference::Auto,
            preview_size: 1280,
            refine_size: 2048,
            scanning: false,
            open_after_scan: true,
            progress: None,
            stage_log: VecDeque::new(),
            sender,
            receiver,
            run_dir: None,
            review_url: None,
            review_server_stop: None,
            error: None,
        }
    }

    fn poll_messages(&mut self, context: &egui::Context) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                GuiMessage::Progress(update) => {
                    if self.stage_log.back() != Some(&update.stage) {
                        self.stage_log.push_back(update.stage);
                        if self.stage_log.len() > 9 {
                            self.stage_log.pop_front();
                        }
                    }
                    self.progress = Some(update);
                }
                GuiMessage::Finished(result) => {
                    self.scanning = false;
                    match result {
                        Ok(run_dir) => {
                            self.run_dir = Some(run_dir.clone());
                            if self.open_after_scan {
                                self.start_review_server(run_dir);
                            }
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                GuiMessage::ServerFailed(error) => {
                    self.review_url = None;
                    self.review_server_stop = None;
                    self.error = Some(error);
                }
            }
        }
        if self.scanning {
            context.request_repaint_after(Duration::from_millis(80));
        }
    }

    fn start_scan(&mut self) {
        let root = PathBuf::from(self.source.trim());
        if !root.is_dir() {
            self.error = Some(tr(self.locale, "source_invalid").to_string());
            return;
        }
        let output = Some(if self.output.trim().is_empty() {
            default_gui_run_dir()
        } else {
            PathBuf::from(self.output.trim())
        });
        let options = ScanOptions {
            preview_size: self.preview_size,
            refine_size: self.refine_size.max(self.preview_size),
            acceleration: self.acceleration,
            detector: self.detector,
            ..ScanOptions::default()
        };
        let sender = self.sender.clone();
        self.stop_review_server();
        self.scanning = true;
        self.error = None;
        self.run_dir = None;
        self.review_url = None;
        self.stage_log.clear();
        self.progress = None;
        thread::spawn(move || {
            let progress_sender = sender.clone();
            let reporter = ProgressReporter::new(move |update| {
                let _ = progress_sender.send(GuiMessage::Progress(update));
            });
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
                .and_then(|runtime| {
                    runtime
                        .block_on(run_scan(&root, output, options, reporter))
                        .map_err(|error| error.to_string())
                });
            let _ = sender.send(GuiMessage::Finished(result));
        });
    }

    fn start_review_server(&mut self, run_dir: PathBuf) {
        if let Some(url) = &self.review_url {
            let _ = open::that(url);
            return;
        }
        let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else {
            self.error = Some(tr(self.locale, "server_failed").to_string());
            return;
        };
        let Ok(addr) = listener.local_addr() else {
            self.error = Some(tr(self.locale, "server_failed").to_string());
            return;
        };
        drop(listener);
        let url = format!("http://{addr}/?lang={}", self.locale.code());
        self.review_url = Some(url.clone());
        let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
        self.review_server_stop = Some(stop_sender);
        let sender = self.sender.clone();
        thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = sender.send(GuiMessage::ServerFailed(error.to_string()));
                    return;
                }
            };
            let server = runtime.block_on(crate::server::serve_with_shutdown(
                run_dir,
                addr,
                async move {
                    let _ = stop_receiver.await;
                },
            ));
            if let Err(error) = server {
                let _ = sender.send(GuiMessage::ServerFailed(error.to_string()));
            }
        });
        thread::spawn(move || {
            for _ in 0..30 {
                if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(80)).is_ok() {
                    let _ = open::that(&url);
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
    }

    fn choose_source(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.source = path.display().to_string();
        }
    }

    fn choose_output(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.output = path.display().to_string();
        }
    }

    fn stop_review_server(&mut self) {
        if let Some(stop) = self.review_server_stop.take() {
            let _ = stop.send(());
        }
        self.review_url = None;
    }
}

fn default_gui_run_dir() -> PathBuf {
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("Pictures")
        .join("Burst Frame Deduplicator Runs");
    root.join(format!(
        "run_{}",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ))
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_messages(&root.ctx().clone());
        egui::CentralPanel::default().show(root, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new(tr(self.locale, "title")).size(22.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.selectable_value(&mut self.locale, Locale::SimplifiedChinese, "简体中文");
                    ui.selectable_value(&mut self.locale, Locale::English, "English");
                });
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_sized([110.0, 30.0], egui::Label::new(tr(self.locale, "source")));
                let field_width = (ui.available_width() - 86.0).max(220.0);
                ui.add_enabled(
                    !self.scanning,
                    egui::TextEdit::singleline(&mut self.source).desired_width(field_width),
                );
                if ui
                    .add_enabled(
                        !self.scanning,
                        egui::Button::new(tr(self.locale, "choose")).min_size([76.0, 30.0].into()),
                    )
                    .clicked()
                {
                    self.choose_source();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_sized([110.0, 30.0], egui::Label::new(tr(self.locale, "output")));
                let field_width = (ui.available_width() - 86.0).max(220.0);
                ui.add_enabled(
                    !self.scanning,
                    egui::TextEdit::singleline(&mut self.output)
                        .hint_text(tr(self.locale, "automatic"))
                        .desired_width(field_width),
                );
                if ui
                    .add_enabled(
                        !self.scanning,
                        egui::Button::new(tr(self.locale, "choose")).min_size([76.0, 30.0].into()),
                    )
                    .clicked()
                {
                    self.choose_output();
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            ui.add_enabled_ui(!self.scanning, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(tr(self.locale, "acceleration"));
                    egui::ComboBox::from_id_salt("acceleration")
                        .selected_text(acceleration_name(self.locale, self.acceleration))
                        .show_ui(ui, |ui| {
                            for value in [
                                AccelerationPreference::Auto,
                                AccelerationPreference::Cpu,
                                AccelerationPreference::Metal,
                            ] {
                                ui.selectable_value(
                                    &mut self.acceleration,
                                    value,
                                    acceleration_name(self.locale, value),
                                );
                            }
                        });
                    ui.add_space(14.0);
                    ui.label(tr(self.locale, "detector"));
                    egui::ComboBox::from_id_salt("detector")
                        .selected_text(detector_name(self.locale, self.detector))
                        .show_ui(ui, |ui| {
                            for value in [
                                DetectorPreference::Auto,
                                DetectorPreference::Heuristic,
                                DetectorPreference::Vision,
                                DetectorPreference::Off,
                            ] {
                                ui.selectable_value(
                                    &mut self.detector,
                                    value,
                                    detector_name(self.locale, value),
                                );
                            }
                        });
                });

                egui::CollapsingHeader::new(tr(self.locale, "quality_settings")).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr(self.locale, "preview_size"));
                        ui.add(egui::DragValue::new(&mut self.preview_size).range(512..=4096));
                        ui.add_space(16.0);
                        ui.label(tr(self.locale, "refine_size"));
                        ui.add(egui::DragValue::new(&mut self.refine_size).range(512..=8192));
                    });
                });
            });
            ui.checkbox(
                &mut self.open_after_scan,
                tr(self.locale, "open_after_scan"),
            );

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let can_start = !self.scanning && !self.source.trim().is_empty();
                let start = ui.add_enabled(
                    can_start,
                    egui::Button::new(
                        RichText::new(tr(self.locale, "start_scan"))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(23, 107, 70))
                    .min_size([112.0, 36.0].into()),
                );
                if start.clicked() {
                    self.start_scan();
                }
                if let Some(run_dir) = self.run_dir.clone()
                    && ui
                        .add(egui::Button::new(tr(self.locale, "open_review")))
                        .clicked()
                {
                    self.start_review_server(run_dir);
                }
            });

            if let Some(error) = &self.error {
                ui.add_space(10.0);
                ui.colored_label(Color32::from_rgb(177, 45, 35), error);
            }

            ui.add_space(18.0);
            if let Some(progress) = &self.progress {
                let label = stage_name(self.locale, progress.stage);
                ui.add(
                    egui::ProgressBar::new(progress.overall_fraction)
                        .show_percentage()
                        .text(label),
                );
                ui.horizontal(|ui| {
                    if let Some(total) = progress.total {
                        ui.label(format!("{} / {}", progress.current, total));
                    }
                    if let Some(detail) = &progress.detail
                        && matches!(
                            progress.stage,
                            ProgressStage::Discovering
                                | ProgressStage::Analyzing
                                | ProgressStage::Refining
                                | ProgressStage::Complete
                        )
                    {
                        ui.weak(detail);
                    }
                });
            }

            if !self.stage_log.is_empty() {
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .max_height(190.0)
                    .show(ui, |ui| {
                        for stage in &self.stage_log {
                            let marker = if self.progress.as_ref().is_some_and(|progress| {
                                progress.stage == *stage && *stage != ProgressStage::Complete
                            }) {
                                "•"
                            } else {
                                "✓"
                            };
                            ui.label(format!("{marker} {}", stage_name(self.locale, *stage)));
                        }
                    });
            }
        });
    }
}

impl Drop for DesktopApp {
    fn drop(&mut self) {
        self.stop_review_server();
    }
}

fn install_cjk_font(context: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ];
    let Some(bytes) = CANDIDATES.iter().find_map(|path| std::fs::read(path).ok()) else {
        return;
    };
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("system-cjk".to_string(), FontData::from_owned(bytes).into());
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("system-cjk".to_string());
    }
    context.set_fonts(fonts);
}

fn tr(locale: Locale, key: &str) -> &'static str {
    match (locale, key) {
        (Locale::English, "title") => "Burst Frame Deduplicator",
        (Locale::English, "source") => "Photo folder",
        (Locale::English, "output") => "Run folder",
        (Locale::English, "choose") => "Choose",
        (Locale::English, "automatic") => "Automatic",
        (Locale::English, "acceleration") => "Acceleration",
        (Locale::English, "detector") => "Subject detector",
        (Locale::English, "quality_settings") => "Quality settings",
        (Locale::English, "preview_size") => "Preview long edge",
        (Locale::English, "refine_size") => "Refine long edge",
        (Locale::English, "open_after_scan") => "Open review when scan completes",
        (Locale::English, "start_scan") => "Start scan",
        (Locale::English, "open_review") => "Open review",
        (Locale::English, "source_invalid") => "Select an accessible photo folder.",
        (Locale::English, "server_failed") => "Could not start the local review server.",
        (Locale::SimplifiedChinese, "title") => "连拍照片筛选器",
        (Locale::SimplifiedChinese, "source") => "照片文件夹",
        (Locale::SimplifiedChinese, "output") => "运行结果文件夹",
        (Locale::SimplifiedChinese, "choose") => "选择",
        (Locale::SimplifiedChinese, "automatic") => "自动创建",
        (Locale::SimplifiedChinese, "acceleration") => "硬件加速",
        (Locale::SimplifiedChinese, "detector") => "主体检测器",
        (Locale::SimplifiedChinese, "quality_settings") => "质量设置",
        (Locale::SimplifiedChinese, "preview_size") => "预览图长边",
        (Locale::SimplifiedChinese, "refine_size") => "精细分析长边",
        (Locale::SimplifiedChinese, "open_after_scan") => "扫描完成后打开审核页",
        (Locale::SimplifiedChinese, "start_scan") => "开始扫描",
        (Locale::SimplifiedChinese, "open_review") => "打开审核页",
        (Locale::SimplifiedChinese, "source_invalid") => "请选择可访问的照片文件夹。",
        (Locale::SimplifiedChinese, "server_failed") => "无法启动本地审核服务。",
        _ => "",
    }
}

fn stage_name(locale: Locale, stage: ProgressStage) -> &'static str {
    match (locale, stage) {
        (Locale::English, stage) => stage.english_label(),
        (Locale::SimplifiedChinese, ProgressStage::Preparing) => "准备扫描",
        (Locale::SimplifiedChinese, ProgressStage::Discovering) => "查找照片",
        (Locale::SimplifiedChinese, ProgressStage::Analyzing) => "分析预览图",
        (Locale::SimplifiedChinese, ProgressStage::Grouping) => "划分连拍与相似组",
        (Locale::SimplifiedChinese, ProgressStage::Refining) => "精细分析候选照片",
        (Locale::SimplifiedChinese, ProgressStage::Ranking) => "生成保留建议",
        (Locale::SimplifiedChinese, ProgressStage::Writing) => "写入扫描结果",
        (Locale::SimplifiedChinese, ProgressStage::Exporting) => "导出审核文件",
        (Locale::SimplifiedChinese, ProgressStage::Complete) => "扫描完成",
    }
}

fn acceleration_name(locale: Locale, value: AccelerationPreference) -> &'static str {
    match (locale, value) {
        (Locale::English, AccelerationPreference::Auto) => "Automatic",
        (Locale::English, AccelerationPreference::Cpu) => "CPU",
        (Locale::English, AccelerationPreference::Metal) => "Metal",
        (Locale::English, AccelerationPreference::Cuda) => "CUDA",
        (Locale::English, AccelerationPreference::OpenCl) => "OpenCL",
        (Locale::SimplifiedChinese, AccelerationPreference::Auto) => "自动",
        (_, AccelerationPreference::Cpu) => "CPU",
        (_, AccelerationPreference::Metal) => "Metal",
        (_, AccelerationPreference::Cuda) => "CUDA",
        (_, AccelerationPreference::OpenCl) => "OpenCL",
    }
}

fn detector_name(locale: Locale, value: DetectorPreference) -> &'static str {
    match (locale, value) {
        (Locale::English, DetectorPreference::Auto) => "Automatic",
        (Locale::English, DetectorPreference::Off) => "Off",
        (Locale::English, DetectorPreference::Heuristic) => "Heuristic",
        (Locale::English, DetectorPreference::Vision) => "macOS Vision",
        (Locale::SimplifiedChinese, DetectorPreference::Auto) => "自动",
        (Locale::SimplifiedChinese, DetectorPreference::Off) => "关闭",
        (Locale::SimplifiedChinese, DetectorPreference::Heuristic) => "启发式检测",
        (Locale::SimplifiedChinese, DetectorPreference::Vision) => "macOS 视觉框架",
    }
}
