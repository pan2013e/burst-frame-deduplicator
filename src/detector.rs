use crate::types::{
    DetectorOutput, DetectorPreference, DetectorReport, QualityMetrics, ScanOptions,
};
use image::RgbImage;

pub struct DetectorEngine {
    report: DetectorReport,
    backend: DetectorBackend,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DetectorTimingSnapshot {
    pub runs: usize,
    pub preprocessing_ms: f64,
    pub queue_wait_ms: f64,
    pub inference_ms: f64,
    pub postprocessing_ms: f64,
}

enum DetectorBackend {
    Off,
    Heuristic,
    Vision,
    #[cfg(all(target_os = "linux", feature = "onnx-detector"))]
    Ml(crate::ml_detector::MlDetector),
}

impl DetectorEngine {
    pub fn initialize(options: &ScanOptions, _worker_count: usize) -> Self {
        let mut report = detector_report(options.detector);
        let backend = match options.detector {
            DetectorPreference::Off => DetectorBackend::Off,
            DetectorPreference::Vision => DetectorBackend::Vision,
            DetectorPreference::Auto | DetectorPreference::Heuristic => DetectorBackend::Heuristic,
            DetectorPreference::MlLight | DetectorPreference::MlHeavy => {
                #[cfg(all(target_os = "linux", feature = "onnx-detector"))]
                {
                    match crate::ml_detector::MlDetector::initialize(options, _worker_count) {
                        Ok((detector, notes)) => {
                            report.selected = detector.backend_name().to_string();
                            report.model = Some(detector.model_report());
                            report.notes.extend(notes);
                            DetectorBackend::Ml(detector)
                        }
                        Err(note) => {
                            report.notes.push(note);
                            DetectorBackend::Heuristic
                        }
                    }
                }

                #[cfg(not(all(target_os = "linux", feature = "onnx-detector")))]
                {
                    report.notes.push(
                        "Local ML detection is unavailable in this build; heuristic saliency will be used."
                            .to_string(),
                    );
                    DetectorBackend::Heuristic
                }
            }
        };
        Self { report, backend }
    }

    pub fn report(&self) -> DetectorReport {
        #[allow(unused_mut)]
        let mut report = self.report.clone();
        #[cfg(all(target_os = "linux", feature = "onnx-detector"))]
        if let DetectorBackend::Ml(detector) = &self.backend {
            report.model = Some(detector.model_report());
            if let Some(note) = detector.failure_note() {
                report.notes.push(note);
            }
        }
        report
    }

    pub fn detect(
        &self,
        image: &RgbImage,
        metrics: &QualityMetrics,
    ) -> (Option<DetectorOutput>, Vec<String>) {
        match &self.backend {
            DetectorBackend::Off => (None, Vec::new()),
            DetectorBackend::Heuristic => (Some(heuristic_output(metrics)), Vec::new()),
            DetectorBackend::Vision => vision_or_heuristic(image, metrics),
            #[cfg(all(target_os = "linux", feature = "onnx-detector"))]
            DetectorBackend::Ml(detector) => match detector.detect(image) {
                Ok(Some(output)) => (Some(output), Vec::new()),
                Ok(None) => (
                    Some(heuristic_output(metrics)),
                    vec![
                        "The local ML model found no confident subject; used heuristic detector fallback."
                            .to_string(),
                    ],
                ),
                Err(first_failure) => (
                    Some(heuristic_output(metrics)),
                    first_failure
                        .then(|| {
                            vec![
                                "Local ML inference failed; disabled it for this scan and used heuristic detector fallback."
                                    .to_string(),
                            ]
                        })
                        .unwrap_or_default(),
                ),
            },
        }
    }

    pub fn timing_snapshot(&self) -> DetectorTimingSnapshot {
        #[cfg(all(target_os = "linux", feature = "onnx-detector"))]
        if let DetectorBackend::Ml(detector) = &self.backend {
            return detector.timing_snapshot();
        }
        DetectorTimingSnapshot::default()
    }
}

pub fn detector_report(requested: DetectorPreference) -> DetectorReport {
    let mut capabilities = vec!["heuristic_saliency".to_string()];
    let mut notes = vec![
        "The heuristic detector is always available and uses saliency, border contact, and object-like edge concentration.".to_string(),
    ];

    if cfg!(all(target_os = "macos", feature = "macos-vision")) {
        capabilities.push("macos_vision_saliency".to_string());
        notes.push("macOS Vision saliency is compiled in and runs locally through Apple system frameworks.".to_string());
    } else if cfg!(target_os = "macos") {
        notes.push(
            "macOS detected, but this build was compiled without the macos-vision feature."
                .to_string(),
        );
    }

    #[cfg(all(target_os = "linux", feature = "onnx-detector"))]
    {
        capabilities.push("onnxruntime_local_ml".to_string());
        capabilities.push("ml_light:u2netp".to_string());
        capabilities.push("ml_heavy:isnet_general_use".to_string());
    }

    let selected = match requested {
        DetectorPreference::Off => "off",
        DetectorPreference::Vision if cfg!(all(target_os = "macos", feature = "macos-vision")) => {
            "macos_vision_saliency"
        }
        DetectorPreference::Vision => "heuristic_saliency",
        DetectorPreference::Auto
        | DetectorPreference::Heuristic
        | DetectorPreference::MlLight
        | DetectorPreference::MlHeavy => "heuristic_saliency",
    };

    if requested == DetectorPreference::Vision && selected != "macos_vision_saliency" {
        notes.push(
            "Vision was requested but is unavailable; heuristic saliency will be used.".to_string(),
        );
    }
    if requested == DetectorPreference::Vision {
        notes.push(
            "Vision contributes advisory completeness and quality metrics; the stable compact saliency track owns near-duplicate comparison."
                .to_string(),
        );
    }
    if requested == DetectorPreference::Auto
        && cfg!(all(target_os = "macos", feature = "macos-vision"))
    {
        notes.push("Auto keeps the fast heuristic detector by default; pass --detector vision to use macOS Vision saliency.".to_string());
    }
    if matches!(
        requested,
        DetectorPreference::MlLight | DetectorPreference::MlHeavy
    ) {
        notes.push(
            "Local ML detection is advisory; the portable saliency descriptor remains responsible for near-duplicate comparison."
                .to_string(),
        );
    }

    DetectorReport {
        requested,
        selected: selected.to_string(),
        capabilities,
        notes,
        model: None,
    }
}

pub fn detect_subject(
    image: &RgbImage,
    metrics: &QualityMetrics,
    engine: &DetectorEngine,
) -> (Option<DetectorOutput>, Vec<String>) {
    engine.detect(image, metrics)
}

pub fn merge_detector_metrics(metrics: &mut QualityMetrics, detector: &DetectorOutput) {
    if detector.confidence <= 0.0 || detector.subject_count == 0 {
        return;
    }
    let detector_completeness = (1.0 - detector.truncation_risk).clamp(0.0, 1.0);
    metrics.object_confidence = metrics.object_confidence.max(detector.confidence);
    metrics.completeness =
        (0.62 * metrics.completeness + 0.38 * detector_completeness).clamp(0.0, 1.0);
    metrics.bbox_x1 = detector.bbox_x1;
    metrics.bbox_y1 = detector.bbox_y1;
    metrics.bbox_x2 = detector.bbox_x2;
    metrics.bbox_y2 = detector.bbox_y2;
}

fn vision_or_heuristic(
    image: &RgbImage,
    metrics: &QualityMetrics,
) -> (Option<DetectorOutput>, Vec<String>) {
    #[cfg(all(target_os = "macos", feature = "macos-vision"))]
    {
        match macos_vision::detect(image) {
            Ok(Some(output)) => (Some(output), Vec::new()),
            Ok(None) => (
                Some(heuristic_output(metrics)),
                vec![
                    "macOS Vision found no salient object; used heuristic detector fallback."
                        .to_string(),
                ],
            ),
            Err(err) => (
                Some(heuristic_output(metrics)),
                vec![format!(
                    "macOS Vision detector failed; used heuristic fallback: {err}"
                )],
            ),
        }
    }

    #[cfg(not(all(target_os = "macos", feature = "macos-vision")))]
    {
        let _ = image;
        (
            Some(heuristic_output(metrics)),
            vec![
                "macOS Vision detector is unavailable in this build; used heuristic fallback."
                    .to_string(),
            ],
        )
    }
}

fn heuristic_output(metrics: &QualityMetrics) -> DetectorOutput {
    DetectorOutput {
        backend: "heuristic_saliency".to_string(),
        confidence: metrics.object_confidence,
        subject_count: usize::from(metrics.object_confidence > 0.05),
        truncation_risk: (1.0 - metrics.completeness).clamp(0.0, 1.0),
        bbox_x1: metrics.bbox_x1,
        bbox_y1: metrics.bbox_y1,
        bbox_x2: metrics.bbox_x2,
        bbox_y2: metrics.bbox_y2,
        explanation: if metrics.completeness < 0.35 {
            "Likely subject detail is close to the frame edge.".to_string()
        } else {
            "Subject-like detail appears inside the frame.".to_string()
        },
    }
}

#[cfg(all(target_os = "macos", feature = "macos-vision"))]
#[allow(unexpected_cfgs)]
mod macos_vision {
    use std::ffi::CStr;
    use std::ptr;

    use anyhow::{Context, anyhow};
    use image::{ExtendedColorType, RgbImage, codecs::jpeg::JpegEncoder};
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    use crate::types::DetectorOutput;

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    pub fn detect(image: &RgbImage) -> anyhow::Result<Option<DetectorOutput>> {
        let mut encoded = Vec::new();
        JpegEncoder::new_with_quality(&mut encoded, 82)
            .encode(
                image.as_raw(),
                image.width(),
                image.height(),
                ExtendedColorType::Rgb8,
            )
            .context("encoding the analysis preview for Vision")?;

        unsafe {
            let data: *mut Object =
                msg_send![class("NSData")?, dataWithBytes: encoded.as_ptr() length: encoded.len()];
            let options: *mut Object = msg_send![class("NSDictionary")?, dictionary];

            let request_cls = class("VNGenerateObjectnessBasedSaliencyImageRequest")?;
            let request: *mut Object = msg_send![request_cls, new];
            let handler_alloc: *mut Object = msg_send![class("VNImageRequestHandler")?, alloc];
            let handler: *mut Object =
                msg_send![handler_alloc, initWithData: data options: options];
            let requests: *mut Object = msg_send![class("NSArray")?, arrayWithObject: request];

            let mut error: *mut Object = ptr::null_mut();
            let ok: bool = msg_send![handler, performRequests: requests error: &mut error];
            if !ok {
                let message =
                    ns_error_message(error).unwrap_or_else(|| "Vision request failed".to_string());
                let _: () = msg_send![request, release];
                let _: () = msg_send![handler, release];
                return Err(anyhow!(message));
            }

            let results: *mut Object = msg_send![request, results];
            let result_count: usize = msg_send![results, count];
            if result_count == 0 {
                let _: () = msg_send![request, release];
                let _: () = msg_send![handler, release];
                return Ok(None);
            }

            let observation: *mut Object = msg_send![results, objectAtIndex: 0usize];
            let objects: *mut Object = msg_send![observation, salientObjects];
            if objects.is_null() {
                let _: () = msg_send![request, release];
                let _: () = msg_send![handler, release];
                return Ok(None);
            }
            let object_count: usize = msg_send![objects, count];
            if object_count == 0 {
                let _: () = msg_send![request, release];
                let _: () = msg_send![handler, release];
                return Ok(None);
            }

            let mut best: Option<(CGRect, f64)> = None;
            for index in 0..object_count {
                let object: *mut Object = msg_send![objects, objectAtIndex: index];
                let confidence: f32 = msg_send![object, confidence];
                let rect: CGRect = msg_send![object, boundingBox];
                if best
                    .as_ref()
                    .is_none_or(|(_, best_conf)| f64::from(confidence) > *best_conf)
                {
                    best = Some((rect, f64::from(confidence)));
                }
            }

            let _: () = msg_send![request, release];
            let _: () = msg_send![handler, release];

            let Some((rect, confidence)) = best else {
                return Ok(None);
            };
            let x1 = rect.origin.x.clamp(0.0, 1.0);
            let x2 = (rect.origin.x + rect.size.width).clamp(0.0, 1.0);
            let y1 = (1.0 - rect.origin.y - rect.size.height).clamp(0.0, 1.0);
            let y2 = (1.0 - rect.origin.y).clamp(0.0, 1.0);
            let margin = x1.min(y1).min(1.0 - x2).min(1.0 - y2).max(0.0);
            let truncation_risk = (1.0 - (margin / 0.055).clamp(0.0, 1.0)).clamp(0.0, 1.0);

            Ok(Some(DetectorOutput {
                backend: "macos_vision_saliency".to_string(),
                confidence: confidence.clamp(0.0, 1.0),
                subject_count: object_count,
                truncation_risk,
                bbox_x1: x1,
                bbox_y1: y1,
                bbox_x2: x2,
                bbox_y2: y2,
                explanation: format!(
                    "macOS Vision found {object_count} salient object candidate(s); best confidence {confidence:.2}."
                ),
            }))
        }
    }

    unsafe fn class(name: &str) -> anyhow::Result<&'static Class> {
        Class::get(name).ok_or_else(|| anyhow!("Objective-C class {name} is unavailable"))
    }

    unsafe fn ns_error_message(error: *mut Object) -> Option<String> {
        if error.is_null() {
            return None;
        }
        let desc: *mut Object = msg_send![error, localizedDescription];
        if desc.is_null() {
            return None;
        }
        let bytes: *const std::os::raw::c_char = msg_send![desc, UTF8String];
        if bytes.is_null() {
            return None;
        }
        Some(
            unsafe { CStr::from_ptr(bytes) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(all(test, target_os = "linux", feature = "onnx-detector"))]
mod tests {
    use std::path::PathBuf;

    use super::DetectorEngine;
    use crate::types::{DetectorPreference, ScanOptions};

    #[test]
    fn missing_ml_pack_falls_back_without_exposing_its_path() {
        let secret = "/private/test-only/missing-model-pack";
        let options = ScanOptions {
            detector: DetectorPreference::MlLight,
            detector_model_pack: Some(PathBuf::from(secret)),
            ..ScanOptions::default()
        };
        let report = DetectorEngine::initialize(&options, 4).report();
        assert_eq!(report.requested, DetectorPreference::MlLight);
        assert_eq!(report.selected, "heuristic_saliency");
        assert!(report.model.is_none());
        assert!(report.notes.iter().any(|note| note.contains("model")));
        assert!(!report.notes.join(" ").contains(secret));
    }
}
