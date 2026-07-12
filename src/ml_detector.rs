use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{Context, anyhow};
use image::{RgbImage, imageops::FilterType};
use ort::ep::cuda::ConvAlgorithmSearch;
use ort::session::{OutputSelector, RunOptions, Session};
use ort::value::TensorRef;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};

use crate::detector::DetectorTimingSnapshot;
use crate::types::{
    DetectorDevicePreference, DetectorModelReport, DetectorOutput, DetectorPreference, ScanOptions,
};

const RUNTIME_VERSION: &str = "onnxruntime-1.24.2";
static LOADED_RUNTIME: OnceLock<Result<PathBuf, String>> = OnceLock::new();

#[derive(Clone, Copy)]
struct ModelSpec {
    id: &'static str,
    filename: &'static str,
    sha256: &'static str,
    input_name: &'static str,
    output_name: &'static str,
    size: usize,
    mean: [f32; 3],
    std: [f32; 3],
}

const LIGHT: ModelSpec = ModelSpec {
    id: "u2netp-sod-v1",
    filename: "models/u2netp.onnx",
    sha256: "309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8",
    input_name: "input.1",
    output_name: "1959",
    size: 320,
    mean: [0.485, 0.456, 0.406],
    std: [0.229, 0.224, 0.225],
};

const HEAVY: ModelSpec = ModelSpec {
    id: "isnet-general-use-v1",
    filename: "models/isnet-general-use.onnx",
    sha256: "60920e99c45464f2ba57bee2ad08c919a52bbf852739e96947fbb4358c0d964a",
    input_name: "input_image",
    output_name: "output_image",
    size: 1024,
    mean: [0.5, 0.5, 0.5],
    std: [1.0, 1.0, 1.0],
};

pub(crate) struct MlDetector {
    spec: ModelSpec,
    backend_name: String,
    model_path: PathBuf,
    threads: usize,
    report: DetectorModelReport,
    session: Mutex<SessionState>,
    disabled: AtomicBool,
    failure: Mutex<Option<String>>,
    timings: Mutex<DetectorTimingSnapshot>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionProvider {
    Cpu,
    Cuda,
}

struct SessionState {
    session: Session,
    provider: SessionProvider,
}

impl MlDetector {
    pub(crate) fn initialize(
        options: &ScanOptions,
        worker_count: usize,
    ) -> Result<(Self, Vec<String>), String> {
        let spec = match options.detector {
            DetectorPreference::MlLight => LIGHT,
            DetectorPreference::MlHeavy => HEAVY,
            _ => return Err("Internal local ML model selection error.".to_string()),
        };
        let pack = options
            .detector_model_pack
            .clone()
            .or_else(|| std::env::var_os("BFD_ML_MODEL_PACK").map(PathBuf::from))
            .ok_or_else(|| {
                "Local ML detection requires an offline model pack; used heuristic saliency instead."
                    .to_string()
            })?;
        let model_path = pack.join(spec.filename);
        let metadata = fs::metadata(&model_path).map_err(|_| {
            "The selected model is missing from the offline model pack; used heuristic saliency instead."
                .to_string()
        })?;
        let observed_hash = sha256(&model_path).map_err(|_| {
            "The selected model could not be verified; used heuristic saliency instead.".to_string()
        })?;
        if observed_hash != spec.sha256 {
            return Err(format!(
                "The selected model failed SHA-256 verification (expected {}, observed {}); used heuristic saliency instead.",
                spec.sha256, observed_hash
            ));
        }

        let cpu_runtime = pack
            .join("runtime-cpu")
            .join("lib")
            .join("libonnxruntime.so");
        let cuda_runtime = pack
            .join("runtime-cuda")
            .join("lib")
            .join("libonnxruntime.so");
        let mut notes = Vec::new();
        let (runtime_path, attempt_cuda, runtime_label) = match options.detector_device {
            DetectorDevicePreference::Cpu if cpu_runtime.is_file() => {
                (cpu_runtime.clone(), false, format!("{RUNTIME_VERSION}-cpu"))
            }
            DetectorDevicePreference::Cpu if cuda_runtime.is_file() => {
                notes.push(
                    "The CPU-only runtime pack is absent; using the CUDA runtime's CPU provider."
                        .to_string(),
                );
                (
                    cuda_runtime.clone(),
                    false,
                    format!("{RUNTIME_VERSION}-cuda12"),
                )
            }
            DetectorDevicePreference::Cpu => {
                (cpu_runtime.clone(), false, format!("{RUNTIME_VERSION}-cpu"))
            }
            DetectorDevicePreference::Cuda if cuda_runtime.is_file() => {
                (cuda_runtime, true, format!("{RUNTIME_VERSION}-cuda12"))
            }
            DetectorDevicePreference::Cuda => {
                notes.push(
                    "CUDA was requested for local ML detection, but the CUDA runtime pack is absent; trying the CPU provider."
                        .to_string(),
                );
                (cpu_runtime.clone(), false, format!("{RUNTIME_VERSION}-cpu"))
            }
            DetectorDevicePreference::Auto if cpu_runtime.is_file() => {
                (cpu_runtime, false, format!("{RUNTIME_VERSION}-cpu"))
            }
            DetectorDevicePreference::Auto if cuda_runtime.is_file() => {
                notes.push(
                    "The CPU-only runtime pack is absent; automatic selection is using the CUDA runtime's CPU provider. Pass --detector-device cuda to initialize a GPU."
                        .to_string(),
                );
                (cuda_runtime, false, format!("{RUNTIME_VERSION}-cuda12"))
            }
            DetectorDevicePreference::Auto => {
                (cpu_runtime, false, format!("{RUNTIME_VERSION}-cpu"))
            }
        };
        if !runtime_path.is_file() {
            return Err(
                "The ONNX Runtime shared library is missing from the offline model pack; used heuristic saliency instead."
                    .to_string(),
            );
        }
        initialize_runtime(&runtime_path)?;

        let threads = options
            .detector_threads
            .unwrap_or_else(|| worker_count.clamp(1, 8))
            .clamp(1, 64);
        let (session, used_cuda) = if attempt_cuda {
            match build_session(&model_path, threads, true) {
                Ok(session) => (session, true),
                Err(_) => {
                    notes.push(
                        "The ONNX Runtime CUDA provider was unavailable; the local ML model is using the CPU provider."
                            .to_string(),
                    );
                    let session = build_session(&model_path, threads, false).map_err(|_| {
                        "The local ML model could not initialize on CUDA or CPU; used heuristic saliency instead."
                            .to_string()
                    })?;
                    (session, false)
                }
            }
        } else {
            let session = build_session(&model_path, threads, false).map_err(|_| {
                "The local ML model could not initialize on the CPU provider; used heuristic saliency instead."
                    .to_string()
            })?;
            (session, false)
        };

        let actual_input = session.inputs().first().map(|input| input.name());
        let has_output = session
            .outputs()
            .iter()
            .any(|output| output.name() == spec.output_name);
        if actual_input != Some(spec.input_name) || !has_output {
            return Err(
                "The selected model has an unexpected tensor contract; used heuristic saliency instead."
                    .to_string(),
            );
        }

        let provider = if used_cuda { "cuda_then_cpu" } else { "cpu" };
        let backend_name = format!("onnx_{}_{provider}", spec.id);
        notes.push(format!(
            "Local ML model {} selected with ONNX Runtime provider {}; inference is serialized through one session using {} thread(s).",
            spec.id, provider, threads
        ));
        let report = DetectorModelReport {
            id: spec.id.to_string(),
            sha256: observed_hash,
            bytes: metadata.len(),
            runtime: runtime_label,
            provider: provider.to_string(),
        };
        Ok((
            Self {
                spec,
                backend_name,
                report,
                model_path,
                threads,
                session: Mutex::new(SessionState {
                    session,
                    provider: if used_cuda {
                        SessionProvider::Cuda
                    } else {
                        SessionProvider::Cpu
                    },
                }),
                disabled: AtomicBool::new(false),
                failure: Mutex::new(None),
                timings: Mutex::new(DetectorTimingSnapshot::default()),
            },
            notes,
        ))
    }

    pub(crate) fn backend_name(&self) -> &str {
        &self.backend_name
    }

    pub(crate) fn model_report(&self) -> DetectorModelReport {
        let mut report = self.report.clone();
        report.provider = match self.session.lock().provider {
            SessionProvider::Cpu => "cpu",
            SessionProvider::Cuda => "cuda_then_cpu",
        }
        .to_string();
        report
    }

    pub(crate) fn failure_note(&self) -> Option<String> {
        self.failure.lock().clone()
    }

    pub(crate) fn timing_snapshot(&self) -> DetectorTimingSnapshot {
        *self.timings.lock()
    }

    pub(crate) fn detect(&self, image: &RgbImage) -> Result<Option<DetectorOutput>, bool> {
        if self.disabled.load(Ordering::Acquire) {
            return Err(false);
        }
        self.detect_inner(image)
    }

    fn detect_inner(&self, image: &RgbImage) -> Result<Option<DetectorOutput>, bool> {
        let preprocessing_start = Instant::now();
        let input = preprocess(image, self.spec);
        let preprocessing_ms = elapsed_ms(preprocessing_start);
        let queue_start = Instant::now();
        let mut state = self.session.lock();
        let queue_wait_ms = elapsed_ms(queue_start);
        if self.disabled.load(Ordering::Acquire) {
            return Err(false);
        }
        let inference_start = Instant::now();
        let mut active_provider = state.provider;
        let probabilities = match run_session(&mut state.session, &input, self.spec) {
            Ok(values) => values,
            Err(_) if state.provider == SessionProvider::Cuda => {
                match build_session(&self.model_path, self.threads, false).and_then(
                    |mut session| {
                        let values = run_session(&mut session, &input, self.spec)?;
                        Ok((session, values))
                    },
                ) {
                    Ok((session, values)) => {
                        state.session = session;
                        state.provider = SessionProvider::Cpu;
                        active_provider = SessionProvider::Cpu;
                        *self.failure.lock() = Some(
                            "CUDA inference failed after initialization; the detector switched to the ONNX Runtime CPU provider for the rest of the scan."
                                .to_string(),
                        );
                        values
                    }
                    Err(_) => return Err(self.disable_after_failure()),
                }
            }
            Err(_) => return Err(self.disable_after_failure()),
        };
        let inference_ms = elapsed_ms(inference_start);
        {
            let mut timings = self.timings.lock();
            timings.runs += 1;
            timings.preprocessing_ms += preprocessing_ms;
            timings.queue_wait_ms += queue_wait_ms;
            timings.inference_ms += inference_ms;
        }
        if probabilities.len() != self.spec.size * self.spec.size {
            return Err(self.disable_after_failure());
        }
        drop(state);
        let postprocessing_start = Instant::now();
        let backend_name = match active_provider {
            SessionProvider::Cpu => format!("onnx_{}_cpu", self.spec.id),
            SessionProvider::Cuda => self.backend_name.clone(),
        };
        let output = mask_to_output(
            &probabilities,
            self.spec.size,
            self.spec.size,
            &backend_name,
        );
        self.timings.lock().postprocessing_ms += elapsed_ms(postprocessing_start);
        Ok(output)
    }

    fn disable_after_failure(&self) -> bool {
        let first = !self.disabled.swap(true, Ordering::AcqRel);
        if first {
            *self.failure.lock() = Some(
                "Local ML inference failed after initialization; subsequent frames used heuristic saliency."
                    .to_string(),
            );
        }
        first
    }
}

fn run_session(session: &mut Session, input: &[f32], spec: ModelSpec) -> anyhow::Result<Vec<f32>> {
    let tensor = TensorRef::from_array_view(([1usize, 3, spec.size, spec.size], input))?;
    let run_options = RunOptions::new()?
        .with_outputs(OutputSelector::no_default().with(spec.output_name.to_string()));
    let outputs = session.run_with_options(ort::inputs![tensor], &run_options)?;
    let (_, values) = outputs[0].try_extract_tensor::<f32>()?;
    Ok(values.to_vec())
}

fn initialize_runtime(path: &Path) -> Result<(), String> {
    let canonical = path.canonicalize().map_err(|_| {
        "The ONNX Runtime shared library could not be resolved; used heuristic saliency instead."
            .to_string()
    })?;
    let loaded = LOADED_RUNTIME.get_or_init(|| {
        ort::init_from(&canonical)
            .map_err(|_| "Unable to load the ONNX Runtime shared library.".to_string())?
            .commit();
        Ok(canonical.clone())
    });
    match loaded {
        Ok(existing) if existing == &canonical => Ok(()),
        Ok(_) => Err(
            "A different ONNX Runtime library is already active; used heuristic saliency instead."
                .to_string(),
        ),
        Err(note) => Err(format!("{note} Used heuristic saliency instead.")),
    }
}

fn build_session(path: &Path, threads: usize, cuda: bool) -> anyhow::Result<Session> {
    let builder = Session::builder()?
        .with_intra_threads(threads)
        .map_err(|error| anyhow!(error.to_string()))?
        .with_inter_threads(1)
        .map_err(|error| anyhow!(error.to_string()))?;
    let mut builder = if cuda {
        builder
            .with_execution_providers([ort::ep::CUDA::default()
                .with_conv_algorithm_search(ConvAlgorithmSearch::Heuristic)
                .with_conv_max_workspace(false)
                .build()
                .error_on_failure()])
            .map_err(|error| anyhow!(error.to_string()))?
    } else {
        builder
    };
    builder
        .commit_from_file(path)
        .context("initializing ONNX detector session")
}

fn sha256(path: &Path) -> anyhow::Result<String> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn preprocess(image: &RgbImage, spec: ModelSpec) -> Vec<f32> {
    let resized = image::imageops::resize(
        image,
        spec.size as u32,
        spec.size as u32,
        FilterType::Lanczos3,
    );
    let maximum = resized
        .pixels()
        .flat_map(|pixel| pixel.0)
        .max()
        .map(f32::from)
        .unwrap_or(0.0);
    let scale = if maximum > 0.0 { 1.0 / maximum } else { 0.0 };
    let plane = spec.size * spec.size;
    let mut output = vec![0.0f32; plane * 3];
    for (index, pixel) in resized.pixels().enumerate() {
        for channel in 0..3 {
            let value = f32::from(pixel[channel]) * scale;
            output[channel * plane + index] = (value - spec.mean[channel]) / spec.std[channel];
        }
    }
    output
}

#[derive(Debug)]
struct Component {
    area: usize,
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    edge_pixels: usize,
    probability_sum: f64,
    peak_probability: f64,
}

fn mask_to_output(
    probabilities: &[f32],
    width: usize,
    height: usize,
    backend: &str,
) -> Option<DetectorOutput> {
    let minimum_area = (width * height / 20_000).max(4);
    let edge_x = (width / 50).max(1);
    let edge_y = (height / 50).max(1);
    let mut visited = vec![false; probabilities.len()];
    let mut components = Vec::new();
    let mut stack = Vec::new();
    for start in 0..probabilities.len() {
        if visited[start] || !probabilities[start].is_finite() || probabilities[start] < 0.5 {
            continue;
        }
        visited[start] = true;
        stack.push(start);
        let mut component = Component {
            area: 0,
            min_x: width,
            min_y: height,
            max_x: 0,
            max_y: 0,
            edge_pixels: 0,
            probability_sum: 0.0,
            peak_probability: 0.0,
        };
        while let Some(index) = stack.pop() {
            let x = index % width;
            let y = index / width;
            let probability = f64::from(probabilities[index].clamp(0.0, 1.0));
            component.area += 1;
            component.min_x = component.min_x.min(x);
            component.min_y = component.min_y.min(y);
            component.max_x = component.max_x.max(x);
            component.max_y = component.max_y.max(y);
            component.probability_sum += probability;
            component.peak_probability = component.peak_probability.max(probability);
            if x < edge_x || y < edge_y || x + edge_x >= width || y + edge_y >= height {
                component.edge_pixels += 1;
            }
            if x > 0 {
                visit_neighbor(index - 1, probabilities, &mut visited, &mut stack);
            }
            if x + 1 < width {
                visit_neighbor(index + 1, probabilities, &mut visited, &mut stack);
            }
            if y > 0 {
                visit_neighbor(index - width, probabilities, &mut visited, &mut stack);
            }
            if y + 1 < height {
                visit_neighbor(index + width, probabilities, &mut visited, &mut stack);
            }
        }
        if component.area >= minimum_area {
            components.push(component);
        }
    }
    if components.is_empty() {
        return None;
    }
    components.sort_by_key(|component| std::cmp::Reverse(component.area));
    let largest_area = components[0].area;
    let relevant: Vec<&Component> = components
        .iter()
        .filter(|component| component.area * 20 >= largest_area)
        .collect();
    let min_x = relevant.iter().map(|component| component.min_x).min()?;
    let min_y = relevant.iter().map(|component| component.min_y).min()?;
    let max_x = relevant.iter().map(|component| component.max_x).max()?;
    let max_y = relevant.iter().map(|component| component.max_y).max()?;
    let selected_area: usize = relevant.iter().map(|component| component.area).sum();
    let edge_pixels: usize = relevant.iter().map(|component| component.edge_pixels).sum();
    let primary = &components[0];
    let mean_probability = primary.probability_sum / primary.area as f64;
    let confidence = (0.75 * mean_probability + 0.25 * primary.peak_probability).clamp(0.0, 1.0);
    let x1 = min_x as f64 / width as f64;
    let y1 = min_y as f64 / height as f64;
    let x2 = (max_x + 1) as f64 / width as f64;
    let y2 = (max_y + 1) as f64 / height as f64;
    let margin = x1.min(y1).min(1.0 - x2).min(1.0 - y2).max(0.0);
    let proximity_risk = 1.0 - (margin / 0.04).clamp(0.0, 1.0);
    let contact_risk = (edge_pixels as f64 / selected_area as f64 * 4.0).clamp(0.0, 1.0);
    let truncation_risk = (0.70 * proximity_risk + 0.30 * contact_risk).clamp(0.0, 1.0);
    Some(DetectorOutput {
        backend: backend.to_string(),
        confidence,
        subject_count: components.len(),
        truncation_risk,
        bbox_x1: x1,
        bbox_y1: y1,
        bbox_x2: x2,
        bbox_y2: y2,
        explanation: if truncation_risk > 0.65 {
            "Likely subject detail is close to the frame edge.".to_string()
        } else {
            "Subject-like detail appears inside the frame.".to_string()
        },
    })
}

fn visit_neighbor(
    index: usize,
    probabilities: &[f32],
    visited: &mut [bool],
    stack: &mut Vec<usize>,
) {
    if !visited[index] && probabilities[index].is_finite() && probabilities[index] >= 0.5 {
        visited[index] = true;
        stack.push(index);
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::{LIGHT, mask_to_output, preprocess};

    #[test]
    fn preprocessing_uses_guarded_per_image_maximum() {
        let image = RgbImage::from_pixel(1, 1, Rgb([128, 64, 0]));
        let input = preprocess(&image, LIGHT);
        let plane = LIGHT.size * LIGHT.size;
        assert!((input[0] - (1.0 - LIGHT.mean[0]) / LIGHT.std[0]).abs() < 1e-6);
        assert!((input[plane] - (0.5 - LIGHT.mean[1]) / LIGHT.std[1]).abs() < 1e-6);
        assert!((input[plane * 2] - (0.0 - LIGHT.mean[2]) / LIGHT.std[2]).abs() < 1e-6);

        let black = preprocess(&RgbImage::from_pixel(1, 1, Rgb([0, 0, 0])), LIGHT);
        assert!(black.iter().all(|value| value.is_finite()));
        assert!((black[0] + LIGHT.mean[0] / LIGHT.std[0]).abs() < 1e-6);
    }

    #[test]
    fn converts_confident_component_to_normalized_box() {
        let mut mask = vec![0.0; 100];
        for y in 3..6 {
            for x in 2..7 {
                mask[y * 10 + x] = 0.9;
            }
        }
        let output = mask_to_output(&mask, 10, 10, "test").unwrap();
        assert_eq!(output.subject_count, 1);
        assert!((output.bbox_x1 - 0.2).abs() < 1e-9);
        assert!((output.bbox_y1 - 0.3).abs() < 1e-9);
        assert!((output.bbox_x2 - 0.7).abs() < 1e-9);
        assert!((output.bbox_y2 - 0.6).abs() < 1e-9);
        assert!(output.confidence > 0.85);
    }

    #[test]
    fn reports_edge_contact_as_truncation_risk() {
        let mut mask = vec![0.0; 400];
        for y in 6..14 {
            for x in 0..5 {
                mask[y * 20 + x] = 0.95;
            }
        }
        let output = mask_to_output(&mask, 20, 20, "test").unwrap();
        assert!(output.truncation_risk > 0.65);
    }
}
