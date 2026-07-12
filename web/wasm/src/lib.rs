use burst_core::{
    FocusMetrics, FocusResult, QualityMetrics, SimilarityFeatures, SubjectDetection,
    compare_similarity, hash_distance, merge_subject_detection, score_image, score_image_with,
    similarity_features, update_subject_focus,
};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod webgpu;
#[cfg(target_arch = "wasm32")]
pub use webgpu::WebGpuFocusScorer;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct BrowserOptions {
    max_seq_gap: i64,
    max_time_gap_ms: i64,
    max_cluster_span_ms: i64,
    max_hash_gap: u32,
    max_duplicate_distance: f64,
    min_duplicate_confidence: f64,
    keepers_per_cluster: Option<usize>,
}

impl Default for BrowserOptions {
    fn default() -> Self {
        Self {
            max_seq_gap: 12,
            max_time_gap_ms: 1250,
            max_cluster_span_ms: 1800,
            max_hash_gap: 30,
            max_duplicate_distance: 0.20,
            min_duplicate_confidence: 0.52,
            keepers_per_cluster: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct BrowserInput {
    id: String,
    rel_path: String,
    modified_ms: i64,
    capture_ms: Option<i64>,
    source_width: u32,
    source_height: u32,
    files: Vec<String>,
    metadata: BrowserMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct BrowserMetadata {
    iso: Option<u32>,
    aperture: Option<f64>,
    shutter: Option<String>,
    focal_length_mm: Option<f64>,
    focal_length_35mm: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct BrowserDetectorInput {
    backend: String,
    confidence: f64,
    subject_count: usize,
    truncation_risk: f64,
    bbox_x1: f64,
    bbox_y1: f64,
    bbox_x2: f64,
    bbox_y2: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct BrowserAnalysisInput {
    sharpness: f64,
    tenengrad: f64,
    focus_backend: String,
    detector: BrowserDetectorInput,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct BrowserFocusInput {
    sharpness: f64,
    tenengrad: f64,
    backend: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
struct BrowserSimilarity {
    subject_confidence: f64,
    subject_area_fraction: f64,
    nearest_distance: f64,
    nearest_subject_distance: f64,
    nearest_global_distance: f64,
    duplicate_confidence: f64,
    pose_novelty: f64,
}

#[derive(Serialize)]
struct BrowserFocusResult {
    sharpness: f64,
    tenengrad: f64,
    backend: &'static str,
}

#[derive(Debug)]
struct BrowserAsset {
    id: String,
    rel_path: String,
    directory: String,
    stem: String,
    prefix: String,
    seq: Option<i64>,
    time_ms: i64,
    source_width: u32,
    source_height: u32,
    files: Vec<String>,
    metadata: BrowserMetadata,
    metrics: QualityMetrics,
    detector_backend: String,
    subject_detection: Option<SubjectDetection>,
    features: SimilarityFeatures,
    burst_id: usize,
    stack_id: usize,
    similarity: BrowserSimilarity,
    action: BrowserAction,
    rank: usize,
    score: f64,
    reason_key: &'static str,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "snake_case")]
enum BrowserAction {
    Keep,
    Reject,
    Review,
    #[default]
    Error,
}

#[derive(Debug)]
struct IndexGroup {
    burst_id: usize,
    indices: Vec<usize>,
    similarity_confidence: f64,
    max_distance: f64,
}

#[derive(Serialize)]
struct BrowserScanResult {
    summary: BrowserSummary,
    bursts: Vec<BrowserBurst>,
    stacks: Vec<BrowserStack>,
    assets: Vec<BrowserResultAsset>,
}

#[derive(Default, Serialize)]
struct BrowserSummary {
    assets: usize,
    bursts: usize,
    stacks: usize,
    keep: usize,
    reject: usize,
    review: usize,
}

#[derive(Serialize)]
struct BrowserBurst {
    id: usize,
    asset_ids: Vec<String>,
    stack_ids: Vec<usize>,
}

#[derive(Serialize)]
struct BrowserStack {
    id: usize,
    burst_id: usize,
    asset_ids: Vec<String>,
    keep_count: usize,
    similarity_confidence: f64,
    max_distance: f64,
}

#[derive(Serialize)]
struct BrowserResultAsset {
    id: String,
    rel_path: String,
    files: Vec<String>,
    source_width: u32,
    source_height: u32,
    metadata: BrowserMetadata,
    metrics: QualityMetrics,
    detector_backend: String,
    burst_id: usize,
    stack_id: usize,
    similarity: BrowserSimilarity,
    action: BrowserAction,
    rank: usize,
    score: f64,
    reason_key: &'static str,
}

#[wasm_bindgen]
pub struct BrowserSession {
    assets: Vec<BrowserAsset>,
}

impl Default for BrowserSession {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl BrowserSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self { assets: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.assets.clear();
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn add_rgba(
        &mut self,
        input: JsValue,
        preview_width: u32,
        preview_height: u32,
        rgba: &[u8],
    ) -> Result<(), JsValue> {
        let input = browser_input(input)?;
        let image = rgba_image(preview_width, preview_height, rgba)?;
        self.push_image(input, image);
        Ok(())
    }

    pub fn add_rgba_with_focus(
        &mut self,
        input: JsValue,
        preview_width: u32,
        preview_height: u32,
        rgba: &[u8],
        sharpness: f64,
        tenengrad: f64,
    ) -> Result<(), JsValue> {
        if !sharpness.is_finite() || !tenengrad.is_finite() {
            return Err(JsValue::from_str("invalid WebGPU focus metrics"));
        }
        let input = browser_input(input)?;
        let image = rgba_image(preview_width, preview_height, rgba)?;
        let score = score_image_with(&image, |_gray, _width, _height| FocusResult {
            metrics: FocusMetrics {
                sharpness,
                tenengrad,
            },
            backend: "webgpu_wgpu".to_string(),
            notes: Vec::new(),
        });
        self.push_scored_image(
            input,
            image,
            score.metrics,
            "heuristic_saliency".to_string(),
            None,
        );
        Ok(())
    }

    pub fn add_rgba_with_analysis(
        &mut self,
        input: JsValue,
        preview_width: u32,
        preview_height: u32,
        rgba: &[u8],
        analysis: JsValue,
    ) -> Result<(), JsValue> {
        let analysis: BrowserAnalysisInput = serde_wasm_bindgen::from_value(analysis)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        if !analysis.sharpness.is_finite() || !analysis.tenengrad.is_finite() {
            return Err(JsValue::from_str("invalid WebGPU focus metrics"));
        }
        let input = browser_input(input)?;
        let image = rgba_image(preview_width, preview_height, rgba)?;
        let score = score_image_with(&image, |_gray, _width, _height| FocusResult {
            metrics: FocusMetrics {
                sharpness: analysis.sharpness,
                tenengrad: analysis.tenengrad,
            },
            backend: analysis.focus_backend,
            notes: Vec::new(),
        });
        let mut metrics = score.metrics;
        let detection = analysis.detector;
        let detector_backend_name = detection.backend.clone();
        let subject_detection = if detection.confidence > 0.0 && detection.subject_count > 0 {
            Some(SubjectDetection {
                confidence: detection.confidence,
                subject_count: detection.subject_count,
                truncation_risk: detection.truncation_risk,
                bbox_x1: detection.bbox_x1,
                bbox_y1: detection.bbox_y1,
                bbox_x2: detection.bbox_x2,
                bbox_y2: detection.bbox_y2,
            })
        } else {
            None
        };
        let detector_backend = if let Some(detection) = subject_detection {
            merge_subject_detection(&mut metrics, &detection);
            update_subject_focus(&image, &mut metrics);
            detector_backend_name
        } else {
            "heuristic_saliency".to_string()
        };
        self.push_scored_image(input, image, metrics, detector_backend, subject_detection);
        Ok(())
    }

    pub fn refinement_candidates(
        &mut self,
        options: JsValue,
        max_candidates_per_stack: usize,
    ) -> Result<JsValue, JsValue> {
        let options = parse_browser_options(options)?;
        let bursts = build_bursts(&self.assets, &options);
        let groups = build_stacks(&mut self.assets, &bursts, &options);
        let mut candidates = Vec::new();
        for group in groups {
            if group.indices.len() <= 1 {
                continue;
            }
            let keep_count = keep_count_for_stack(group.indices.len(), options.keepers_per_cluster);
            let budget = max_candidates_per_stack
                .max(keep_count)
                .min(group.indices.len());
            candidates.extend(
                ranked_scores(&self.assets, &group.indices)
                    .into_iter()
                    .take(budget)
                    .map(|(index, _)| self.assets[index].id.clone()),
            );
        }
        serde_wasm_bindgen::to_value(&candidates)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub fn refine_rgba_with_focus(
        &mut self,
        asset_id: &str,
        preview_width: u32,
        preview_height: u32,
        rgba: &[u8],
        focus: JsValue,
    ) -> Result<(), JsValue> {
        let focus: BrowserFocusInput = serde_wasm_bindgen::from_value(focus)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        if !focus.sharpness.is_finite() || !focus.tenengrad.is_finite() {
            return Err(JsValue::from_str("invalid refinement focus metrics"));
        }
        let index = self
            .assets
            .iter()
            .position(|asset| asset.id == asset_id)
            .ok_or_else(|| JsValue::from_str("refinement asset was not found"))?;
        let image = rgba_image(preview_width, preview_height, rgba)?;
        let score = score_image_with(&image, |_gray, _width, _height| FocusResult {
            metrics: FocusMetrics {
                sharpness: focus.sharpness,
                tenengrad: focus.tenengrad,
            },
            backend: focus.backend,
            notes: Vec::new(),
        });
        let mut metrics = score.metrics;
        if let Some(detection) = self.assets[index].subject_detection {
            merge_subject_detection(&mut metrics, &detection);
            update_subject_focus(&image, &mut metrics);
        }
        self.assets[index].metrics = metrics;
        Ok(())
    }

    pub fn finish(&mut self, options: JsValue) -> Result<JsValue, JsValue> {
        let options = parse_browser_options(options)?;
        let result = build_result(&mut self.assets, &options);
        serde_wasm_bindgen::to_value(&result).map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

fn parse_browser_options(options: JsValue) -> Result<BrowserOptions, JsValue> {
    if options.is_null() || options.is_undefined() {
        Ok(BrowserOptions::default())
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

#[wasm_bindgen]
pub fn portable_focus_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<JsValue, JsValue> {
    let expected = width as usize * height as usize * 4;
    if width < 3 || height < 3 || rgba.len() != expected {
        return Err(JsValue::from_str("invalid RGBA focus buffer"));
    }
    let mut gray = Vec::with_capacity(width as usize * height as usize);
    for pixel in rgba.chunks_exact(4) {
        let luma =
            (54 * u32::from(pixel[0]) + 183 * u32::from(pixel[1]) + 19 * u32::from(pixel[2]) + 128)
                >> 8;
        gray.push(luma as u8);
    }
    let metrics = burst_core::cpu_focus_metrics(&gray, width as usize, height as usize);
    serde_wasm_bindgen::to_value(&BrowserFocusResult {
        sharpness: metrics.sharpness,
        tenengrad: metrics.tenengrad,
        backend: "wasm_cpu_portable",
    })
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

impl BrowserSession {
    fn push_image(&mut self, input: BrowserInput, image: RgbImage) {
        let score = score_image(&image);
        self.push_scored_image(
            input,
            image,
            score.metrics,
            "heuristic_saliency".to_string(),
            None,
        );
    }

    fn push_scored_image(
        &mut self,
        input: BrowserInput,
        image: RgbImage,
        metrics: QualityMetrics,
        detector_backend: String,
        subject_detection: Option<SubjectDetection>,
    ) {
        let features = similarity_features(&image, &metrics);
        let (directory, stem) = split_path(&input.rel_path);
        let (prefix, seq) = split_sequence(&stem);
        self.assets.push(BrowserAsset {
            id: input.id,
            rel_path: input.rel_path,
            directory,
            stem,
            prefix,
            seq,
            time_ms: input.capture_ms.unwrap_or(input.modified_ms),
            source_width: input.source_width,
            source_height: input.source_height,
            files: input.files,
            metadata: input.metadata,
            metrics,
            detector_backend,
            subject_detection,
            features,
            burst_id: 0,
            stack_id: 0,
            similarity: BrowserSimilarity::default(),
            action: BrowserAction::Error,
            rank: 0,
            score: 0.0,
            reason_key: "decode_error",
        });
    }
}

fn browser_input(input: JsValue) -> Result<BrowserInput, JsValue> {
    serde_wasm_bindgen::from_value(input).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn rgba_image(width: u32, height: u32, rgba: &[u8]) -> Result<RgbImage, JsValue> {
    let expected = width as usize * height as usize * 4;
    if width < 8 || height < 8 || rgba.len() != expected {
        return Err(JsValue::from_str("invalid RGBA preview buffer"));
    }
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for pixel in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&pixel[..3]);
    }
    RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| JsValue::from_str("could not construct RGB preview"))
}

#[cfg(any(target_arch = "wasm32", test))]
fn reduce_focus_partials(
    partials: &[i32],
    width: usize,
    height: usize,
) -> Result<FocusMetrics, &'static str> {
    if width < 3 || height < 3 || partials.is_empty() || !partials.len().is_multiple_of(4) {
        return Err("invalid WebGPU focus partials");
    }
    let mut lap_sum = 0i64;
    let mut lap_sq_sum = 0u64;
    let mut dx_sum = 0u64;
    let mut dy_sum = 0u64;
    for partial in partials.chunks_exact(4) {
        if partial[1] < 0 || partial[2] < 0 || partial[3] < 0 {
            return Err("WebGPU focus partial overflow");
        }
        lap_sum += i64::from(partial[0]);
        lap_sq_sum += partial[1] as u64;
        dx_sum += partial[2] as u64;
        dy_sum += partial[3] as u64;
    }
    let lap_count = (width - 2)
        .checked_mul(height - 2)
        .ok_or("WebGPU Laplacian count overflow")? as f64;
    let dx_count = height
        .checked_mul(width - 1)
        .ok_or("WebGPU horizontal-gradient count overflow")? as f64;
    let dy_count = (height - 1)
        .checked_mul(width)
        .ok_or("WebGPU vertical-gradient count overflow")? as f64;
    let mean = lap_sum as f64 / lap_count;
    Ok(FocusMetrics {
        sharpness: lap_sq_sum as f64 / lap_count - mean * mean,
        tenengrad: dx_sum as f64 / dx_count + dy_sum as f64 / dy_count,
    })
}

fn build_result(assets: &mut [BrowserAsset], options: &BrowserOptions) -> BrowserScanResult {
    let bursts = build_bursts(assets, options);
    let groups = build_stacks(assets, &bursts, options);
    let mut stack_ids_by_burst = vec![Vec::new(); bursts.len()];
    let mut stacks = Vec::with_capacity(groups.len());
    for (stack_zero, group) in groups.into_iter().enumerate() {
        let stack_id = stack_zero + 1;
        let keep_count = keep_count_for_stack(group.indices.len(), options.keepers_per_cluster);
        stack_ids_by_burst[group.burst_id - 1].push(stack_id);
        rank_stack(assets, &group.indices, stack_id, group.burst_id, options);
        stacks.push(BrowserStack {
            id: stack_id,
            burst_id: group.burst_id,
            asset_ids: group
                .indices
                .iter()
                .map(|index| assets[*index].id.clone())
                .collect(),
            keep_count,
            similarity_confidence: group.similarity_confidence,
            max_distance: group.max_distance,
        });
    }
    let browser_bursts = bursts
        .iter()
        .enumerate()
        .map(|(burst_zero, indices)| BrowserBurst {
            id: burst_zero + 1,
            asset_ids: indices
                .iter()
                .map(|index| assets[*index].id.clone())
                .collect(),
            stack_ids: stack_ids_by_burst[burst_zero].clone(),
        })
        .collect::<Vec<_>>();

    let mut summary = BrowserSummary {
        assets: assets.len(),
        bursts: browser_bursts.len(),
        stacks: stacks.len(),
        ..BrowserSummary::default()
    };
    let output_assets = assets
        .iter()
        .map(|asset| {
            match asset.action {
                BrowserAction::Keep => summary.keep += 1,
                BrowserAction::Reject => summary.reject += 1,
                BrowserAction::Review | BrowserAction::Error => summary.review += 1,
            }
            BrowserResultAsset {
                id: asset.id.clone(),
                rel_path: asset.rel_path.clone(),
                files: asset.files.clone(),
                source_width: asset.source_width,
                source_height: asset.source_height,
                metadata: asset.metadata.clone(),
                metrics: asset.metrics.clone(),
                detector_backend: asset.detector_backend.clone(),
                burst_id: asset.burst_id,
                stack_id: asset.stack_id,
                similarity: asset.similarity,
                action: asset.action,
                rank: asset.rank,
                score: asset.score,
                reason_key: asset.reason_key,
            }
        })
        .collect();
    BrowserScanResult {
        summary,
        bursts: browser_bursts,
        stacks,
        assets: output_assets,
    }
}

fn build_bursts(assets: &[BrowserAsset], options: &BrowserOptions) -> Vec<Vec<usize>> {
    let mut indices = (0..assets.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        let left = &assets[*left];
        let right = &assets[*right];
        (
            &left.directory,
            &left.prefix,
            left.seq.unwrap_or(-1),
            left.time_ms,
            &left.stem,
        )
            .cmp(&(
                &right.directory,
                &right.prefix,
                right.seq.unwrap_or(-1),
                right.time_ms,
                &right.stem,
            ))
    });
    let mut bursts = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut start_ms = 0i64;
    for index in indices {
        let split = current.last().is_some_and(|previous| {
            should_split_burst(&assets[*previous], &assets[index], start_ms, options)
        });
        if split {
            bursts.push(std::mem::take(&mut current));
        }
        if current.is_empty() {
            start_ms = assets[index].time_ms;
        }
        current.push(index);
    }
    if !current.is_empty() {
        bursts.push(current);
    }
    bursts
}

fn should_split_burst(
    previous: &BrowserAsset,
    current: &BrowserAsset,
    start_ms: i64,
    options: &BrowserOptions,
) -> bool {
    if previous.directory != current.directory || previous.prefix != current.prefix {
        return true;
    }
    if (current.time_ms - previous.time_ms).abs() > options.max_time_gap_ms
        || (current.time_ms - start_ms).abs() > options.max_cluster_span_ms
    {
        return true;
    }
    matches!((previous.seq, current.seq), (Some(previous), Some(current)) if current - previous > options.max_seq_gap)
}

fn build_stacks(
    assets: &mut [BrowserAsset],
    bursts: &[Vec<usize>],
    options: &BrowserOptions,
) -> Vec<IndexGroup> {
    let mut groups = Vec::new();
    for (burst_zero, burst) in bursts.iter().enumerate() {
        let burst_id = burst_zero + 1;
        let mut current: Vec<usize> = Vec::new();
        for index in burst {
            assets[*index].burst_id = burst_id;
            assets[*index].similarity.subject_confidence = assets[*index].features.confidence;
            assets[*index].similarity.subject_area_fraction = assets[*index].features.area_fraction;
            let split = current.last().is_some_and(|previous| {
                hash_distance(
                    &assets[*previous].metrics.dhash,
                    &assets[*index].metrics.dhash,
                ) > options.max_hash_gap
                    || !current.iter().all(|member| {
                        compare_similarity(&assets[*member].features, &assets[*index].features)
                            .distance
                            <= options.max_duplicate_distance
                    })
            });
            if split {
                groups.push(finalize_stack(
                    assets,
                    burst_id,
                    std::mem::take(&mut current),
                    options.max_duplicate_distance,
                ));
            }
            current.push(*index);
        }
        if !current.is_empty() {
            groups.push(finalize_stack(
                assets,
                burst_id,
                current,
                options.max_duplicate_distance,
            ));
        }
    }
    groups
}

fn finalize_stack(
    assets: &mut [BrowserAsset],
    burst_id: usize,
    indices: Vec<usize>,
    threshold: f64,
) -> IndexGroup {
    let mut updates = Vec::with_capacity(indices.len());
    let mut confidence_sum = 0.0;
    let mut max_distance: f64 = 0.0;
    for (position, index) in indices.iter().enumerate() {
        if indices.len() == 1 {
            updates.push((
                *index,
                BrowserSimilarity {
                    subject_confidence: assets[*index].features.confidence,
                    subject_area_fraction: assets[*index].features.area_fraction,
                    nearest_distance: 1.0,
                    pose_novelty: 1.0,
                    ..BrowserSimilarity::default()
                },
            ));
            continue;
        }
        let mut nearest = None;
        for (other_position, other) in indices.iter().enumerate() {
            if position == other_position {
                continue;
            }
            let comparison = compare_similarity(&assets[*index].features, &assets[*other].features);
            max_distance = max_distance.max(comparison.distance);
            if nearest
                .as_ref()
                .is_none_or(|current: &burst_core::SimilarityComparison| {
                    comparison.distance < current.distance
                })
            {
                nearest = Some(comparison);
            }
        }
        if let Some(nearest) = nearest {
            let ratio = (nearest.distance / threshold.max(1e-9)).clamp(0.0, 1.0);
            let duplicate_confidence = nearest.confidence * (1.0 - 0.35 * ratio);
            confidence_sum += duplicate_confidence;
            updates.push((
                *index,
                BrowserSimilarity {
                    subject_confidence: assets[*index].features.confidence,
                    subject_area_fraction: assets[*index].features.area_fraction,
                    nearest_distance: nearest.distance,
                    nearest_subject_distance: nearest.subject_distance,
                    nearest_global_distance: nearest.global_distance,
                    duplicate_confidence,
                    pose_novelty: ratio,
                },
            ));
        }
    }
    for (index, similarity) in updates {
        assets[index].similarity = similarity;
    }
    let group_len = indices.len();
    IndexGroup {
        burst_id,
        indices,
        similarity_confidence: if confidence_sum > 0.0 {
            confidence_sum / group_len.max(1) as f64
        } else {
            0.0
        },
        max_distance,
    }
}

fn rank_stack(
    assets: &mut [BrowserAsset],
    indices: &[usize],
    stack_id: usize,
    burst_id: usize,
    options: &BrowserOptions,
) {
    let ranked = ranked_scores(assets, indices);
    let keep_count = keep_count_for_stack(indices.len(), options.keepers_per_cluster);
    let keep_threshold = ranked
        .get(keep_count.saturating_sub(1))
        .map(|(_, score)| *score)
        .unwrap_or(0.0);
    for (rank_zero, (index, score)) in ranked.into_iter().enumerate() {
        let asset = &mut assets[index];
        asset.burst_id = burst_id;
        asset.stack_id = stack_id;
        asset.rank = rank_zero + 1;
        asset.score = score;
        if indices.len() == 1 {
            asset.action = BrowserAction::Keep;
            asset.reason_key = "distinct_frame";
        } else if rank_zero < keep_count {
            asset.action = BrowserAction::Keep;
            asset.reason_key = "best_quality";
        } else if asset.similarity.duplicate_confidence < options.min_duplicate_confidence {
            asset.action = BrowserAction::Review;
            asset.reason_key = "uncertain_similarity";
        } else if score >= keep_threshold - 0.035 {
            asset.action = BrowserAction::Review;
            asset.reason_key = "quality_tie";
        } else {
            asset.action = BrowserAction::Reject;
            asset.reason_key = "high_confidence_duplicate";
        }
    }
}

fn keep_count_for_stack(size: usize, requested: Option<usize>) -> usize {
    requested.unwrap_or(1).max(1).min(size.max(1))
}

fn ranked_scores(assets: &[BrowserAsset], indices: &[usize]) -> Vec<(usize, f64)> {
    let sharp = normalize(
        indices
            .iter()
            .map(|index| assets[*index].metrics.sharpness.max(0.0).ln_1p())
            .collect(),
    );
    let tenengrad = normalize(
        indices
            .iter()
            .map(|index| assets[*index].metrics.tenengrad.max(0.0).ln_1p())
            .collect(),
    );
    let subject_sharp = normalize(
        indices
            .iter()
            .map(|index| assets[*index].metrics.subject_sharpness.max(0.0).ln_1p())
            .collect(),
    );
    let subject_tenengrad = normalize(
        indices
            .iter()
            .map(|index| assets[*index].metrics.subject_tenengrad.max(0.0).ln_1p())
            .collect(),
    );
    let mut ranked = indices
        .iter()
        .enumerate()
        .map(|(position, index)| {
            let asset = &assets[*index];
            let global_focus = 0.70 * sharp[position] + 0.30 * tenengrad[position];
            let subject_focus = 0.70 * subject_sharp[position] + 0.30 * subject_tenengrad[position];
            let focus = if asset.features.confidence >= 0.3 {
                0.72 * subject_focus + 0.28 * global_focus
            } else {
                global_focus
            };
            let score = 0.60 * focus
                + 0.18 * asset.metrics.completeness
                + 0.10 * asset.metrics.contrast
                + 0.07 * asset.metrics.object_confidence
                + 0.05 * asset.metrics.exposure_score
                - 0.08 * (asset.metrics.border_energy_fraction / 0.35).min(1.0);
            (*index, score)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    ranked
}

fn normalize(values: Vec<f64>) -> Vec<f64> {
    let low = values
        .iter()
        .copied()
        .fold(f64::INFINITY, |current, value| current.min(value));
    let high = values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |current, value| current.max(value));
    if values.is_empty() {
        Vec::new()
    } else if (high - low).abs() < 1e-9 {
        vec![0.5; values.len()]
    } else {
        values
            .into_iter()
            .map(|value| (value - low) / (high - low))
            .collect()
    }
}

fn split_path(rel_path: &str) -> (String, String) {
    let normalized = rel_path.replace('\\', "/");
    let (directory, name) = normalized
        .rsplit_once('/')
        .map_or((String::new(), normalized.as_str()), |(directory, name)| {
            (directory.to_string(), name)
        });
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    (directory, stem.to_string())
}

fn split_sequence(stem: &str) -> (String, Option<i64>) {
    let digit_start = stem
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_ascii_digit())
        .map_or(0, |(index, character)| index + character.len_utf8());
    if digit_start == stem.len() {
        return (stem.to_string(), None);
    }
    let sequence = stem[digit_start..].parse().ok();
    (stem[..digit_start].to_string(), sequence)
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::{
        BrowserInput, BrowserOptions, BrowserSession, build_result, reduce_focus_partials,
        split_sequence,
    };

    #[test]
    fn filename_counter_split_matches_native_expectation() {
        assert_eq!(split_sequence("DSC_0042"), ("DSC_".to_string(), Some(42)));
        assert_eq!(split_sequence("image"), ("image".to_string(), None));
    }

    #[test]
    fn webgpu_partial_reduction_preserves_integer_focus_sums() {
        let metrics = reduce_focus_partials(&[0, 100, 80, 60], 3, 3).unwrap();
        assert_eq!(metrics.sharpness, 100.0);
        assert!((metrics.tenengrad - (80.0 / 6.0 + 60.0 / 6.0)).abs() < 1e-12);
        assert!(reduce_focus_partials(&[0, -1, 0, 0], 3, 3).is_err());
    }

    fn synthetic_frame(x: u32, vertical: bool) -> RgbImage {
        let mut image = RgbImage::from_pixel(320, 240, Rgb([165, 190, 210]));
        if vertical {
            for y in 98..142 {
                for pixel_x in x.saturating_sub(4)..=(x + 4).min(319) {
                    image.put_pixel(pixel_x, y, Rgb([20, 24, 29]));
                }
            }
        } else {
            for y in 116..124 {
                for pixel_x in x.saturating_sub(22)..=(x + 22).min(319) {
                    image.put_pixel(pixel_x, y, Rgb([20, 24, 29]));
                }
            }
        }
        image
    }

    #[test]
    fn browser_session_preserves_pose_changes() {
        let mut session = BrowserSession::new();
        for (index, image) in [
            synthetic_frame(90, false),
            synthetic_frame(220, false),
            synthetic_frame(220, true),
        ]
        .into_iter()
        .enumerate()
        {
            session.push_image(
                BrowserInput {
                    id: format!("asset-{index}"),
                    rel_path: format!("burst/frame_{:04}.jpg", index + 1),
                    modified_ms: index as i64 * 100,
                    source_width: image.width(),
                    source_height: image.height(),
                    files: vec![format!("frame_{:04}.jpg", index + 1)],
                    ..BrowserInput::default()
                },
                image,
            );
        }
        let result = build_result(&mut session.assets, &BrowserOptions::default());
        assert_eq!(result.summary.bursts, 1);
        assert!(
            result.summary.stacks >= 2,
            "stacks={}",
            result.summary.stacks
        );
        assert!(result.summary.keep >= 2);
    }
}
