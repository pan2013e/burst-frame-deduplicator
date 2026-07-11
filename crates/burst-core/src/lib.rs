use std::collections::VecDeque;

use image::{GrayImage, RgbImage, imageops};
use serde::{Deserialize, Serialize};

const DETECTOR_LONG_EDGE: u32 = 512;
const DETECTOR_REFINEMENT_LONG_EDGE: u32 = 1024;
const GLOBAL_DESCRIPTOR_SIZE: u32 = 32;
const SUBJECT_DESCRIPTOR_SIZE: u32 = 64;

#[derive(Debug, Clone)]
pub struct FeatureScore {
    pub metrics: QualityMetrics,
    pub backend: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityMetrics {
    pub sharpness: f64,
    pub tenengrad: f64,
    pub subject_sharpness: f64,
    pub subject_tenengrad: f64,
    pub contrast: f64,
    pub exposure_score: f64,
    pub clipped_fraction: f64,
    pub completeness: f64,
    pub object_confidence: f64,
    pub border_energy_fraction: f64,
    pub bbox_x1: f64,
    pub bbox_y1: f64,
    pub bbox_x2: f64,
    pub bbox_y2: f64,
    pub dhash: String,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            sharpness: 0.0,
            tenengrad: 0.0,
            subject_sharpness: 0.0,
            subject_tenengrad: 0.0,
            contrast: 0.0,
            exposure_score: 0.0,
            clipped_fraction: 1.0,
            completeness: 0.0,
            object_confidence: 0.0,
            border_energy_fraction: 1.0,
            bbox_x1: 0.0,
            bbox_y1: 0.0,
            bbox_x2: 1.0,
            bbox_y2: 1.0,
            dhash: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SimilarityFeatures {
    pub global_luma: Vec<u8>,
    pub subject_luma: Vec<u8>,
    pub subject_edges: Vec<u8>,
    pub subject_mask: Vec<u8>,
    pub confidence: f64,
    pub area_fraction: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SimilarityComparison {
    pub distance: f64,
    pub confidence: f64,
    pub subject_distance: f64,
    pub global_distance: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct FocusMetrics {
    pub sharpness: f64,
    pub tenengrad: f64,
}

#[derive(Debug, Clone)]
pub struct FocusResult {
    pub metrics: FocusMetrics,
    pub backend: String,
    pub notes: Vec<String>,
}

pub fn score_image(image: &RgbImage) -> FeatureScore {
    score_image_with(image, |gray, width, height| FocusResult {
        metrics: cpu_focus_metrics(gray, width, height),
        backend: "cpu".to_string(),
        notes: Vec::new(),
    })
}

pub fn score_image_with(
    image: &RgbImage,
    focus_backend: impl FnOnce(&[u8], usize, usize) -> FocusResult,
) -> FeatureScore {
    let (width, height) = image.dimensions();
    if width < 8 || height < 8 {
        return FeatureScore {
            metrics: QualityMetrics::default(),
            backend: "cpu_small_image".to_string(),
            notes: vec!["Image is too small for full scoring.".to_string()],
        };
    }

    let gray = grayscale(image);
    let FocusResult {
        metrics: focus,
        backend,
        notes,
    } = focus_backend(gray.as_raw(), width as usize, height as usize);
    let histogram = histogram(gray.as_raw());
    let p5 = histogram_percentile(&histogram, gray.as_raw().len(), 5.0);
    let p95 = histogram_percentile(&histogram, gray.as_raw().len(), 95.0);
    let contrast = (f64::from(p95.saturating_sub(p5)) / 255.0).clamp(0.0, 1.0);
    let mut clipped = 0usize;
    let mut luma_sum = 0u64;
    for value in gray.as_raw() {
        clipped += usize::from(*value < 3 || *value > 252);
        luma_sum += u64::from(*value);
    }
    let clipped_fraction = clipped as f64 / gray.as_raw().len() as f64;
    let mean_luma = luma_sum as f64 / (gray.as_raw().len() as f64 * 255.0);
    let mean_penalty = ((mean_luma - 0.52).abs() / 0.52).min(1.0);
    let exposure_score = (1.0 - (clipped_fraction * 6.0 + mean_penalty * 0.35).min(1.0)).max(0.0);
    let bbox = robust_bbox(&gray);
    let subject_focus = focus_in_bbox(&gray, &bbox);
    let dhash = difference_hash(&gray);

    FeatureScore {
        metrics: QualityMetrics {
            sharpness: focus.sharpness,
            tenengrad: focus.tenengrad,
            subject_sharpness: subject_focus.sharpness,
            subject_tenengrad: subject_focus.tenengrad,
            contrast,
            exposure_score,
            clipped_fraction,
            completeness: bbox.completeness,
            object_confidence: bbox.object_confidence,
            border_energy_fraction: bbox.border_energy_fraction,
            bbox_x1: bbox.x1,
            bbox_y1: bbox.y1,
            bbox_x2: bbox.x2,
            bbox_y2: bbox.y2,
            dhash,
        },
        backend,
        notes,
    }
}

pub fn update_subject_focus(image: &RgbImage, metrics: &mut QualityMetrics) {
    let gray = grayscale(image);
    let bbox = BBoxScore {
        x1: metrics.bbox_x1,
        y1: metrics.bbox_y1,
        x2: metrics.bbox_x2,
        y2: metrics.bbox_y2,
        object_confidence: metrics.object_confidence,
        completeness: metrics.completeness,
        border_energy_fraction: metrics.border_energy_fraction,
    };
    let focus = focus_in_bbox(&gray, &bbox);
    metrics.subject_sharpness = focus.sharpness;
    metrics.subject_tenengrad = focus.tenengrad;
}

pub fn similarity_features(image: &RgbImage, metrics: &QualityMetrics) -> SimilarityFeatures {
    let gray = grayscale(image);
    let global = imageops::resize(
        &gray,
        GLOBAL_DESCRIPTOR_SIZE,
        GLOBAL_DESCRIPTOR_SIZE,
        imageops::FilterType::Triangle,
    );
    let (width, height) = image.dimensions();
    let x1 = (metrics.bbox_x1.clamp(0.0, 1.0) * f64::from(width)).floor();
    let y1 = (metrics.bbox_y1.clamp(0.0, 1.0) * f64::from(height)).floor();
    let x2 = (metrics.bbox_x2.clamp(0.0, 1.0) * f64::from(width)).ceil();
    let y2 = (metrics.bbox_y2.clamp(0.0, 1.0) * f64::from(height)).ceil();
    let bbox_width = (x2 - x1).max(1.0);
    let bbox_height = (y2 - y1).max(1.0);
    let area_fraction = (bbox_width * bbox_height / f64::from(width * height)).clamp(0.0, 1.0);
    let mut side = bbox_width.max(bbox_height) * 1.65;
    side = side.max(18.0).min(f64::from(width.min(height)));
    let center_x = (x1 + x2) * 0.5;
    let center_y = (y1 + y2) * 0.5;
    let crop_x1 = (center_x - side * 0.5)
        .round()
        .clamp(0.0, f64::from(width.saturating_sub(1))) as u32;
    let crop_y1 = (center_y - side * 0.5)
        .round()
        .clamp(0.0, f64::from(height.saturating_sub(1))) as u32;
    let crop_x2 = (center_x + side * 0.5)
        .round()
        .clamp(f64::from(crop_x1 + 1), f64::from(width)) as u32;
    let crop_y2 = (center_y + side * 0.5)
        .round()
        .clamp(f64::from(crop_y1 + 1), f64::from(height)) as u32;
    let crop = imageops::crop_imm(
        &gray,
        crop_x1,
        crop_y1,
        crop_x2 - crop_x1,
        crop_y2 - crop_y1,
    )
    .to_image();
    let subject = imageops::resize(
        &crop,
        SUBJECT_DESCRIPTOR_SIZE,
        SUBJECT_DESCRIPTOR_SIZE,
        imageops::FilterType::Lanczos3,
    );
    let edges = edge_descriptor(&subject);
    let mask = foreground_mask(&subject);
    let foreground_fraction =
        mask.iter().filter(|value| **value != 0).count() as f64 / mask.len().max(1) as f64;
    let descriptor_reliability = if (0.002..=0.65).contains(&foreground_fraction) {
        1.0
    } else {
        0.45
    };
    let bbox_reliability = if area_fraction <= 0.45 { 1.0 } else { 0.4 };

    SimilarityFeatures {
        global_luma: global.into_raw(),
        subject_luma: subject.into_raw(),
        subject_edges: edges,
        subject_mask: mask,
        confidence: (metrics.object_confidence * descriptor_reliability * bbox_reliability)
            .clamp(0.0, 1.0),
        area_fraction,
    }
}

pub fn compare_similarity(
    left: &SimilarityFeatures,
    right: &SimilarityFeatures,
) -> SimilarityComparison {
    let global_distance = correlation_distance(&left.global_luma, &right.global_luma);
    if left.subject_luma.is_empty() || right.subject_luma.is_empty() {
        return SimilarityComparison {
            distance: global_distance,
            confidence: 0.0,
            subject_distance: 1.0,
            global_distance,
        };
    }

    let luma_distance = correlation_distance(&left.subject_luma, &right.subject_luma);
    let edge_distance = cosine_distance(&left.subject_edges, &right.subject_edges);
    let (mask_distance, mask_reliability) = mask_distance(&left.subject_mask, &right.subject_mask);
    let subject_distance = 0.42 * mask_distance + 0.38 * edge_distance + 0.20 * luma_distance;
    let confidence = (left.confidence.min(right.confidence) * mask_reliability).clamp(0.0, 1.0);
    let distance = if confidence >= 0.25 {
        0.92 * subject_distance + 0.08 * global_distance
    } else {
        0.70 * subject_distance + 0.30 * global_distance
    };
    SimilarityComparison {
        distance: distance.clamp(0.0, 1.0),
        confidence,
        subject_distance,
        global_distance,
    }
}

pub fn cpu_focus_metrics(gray: &[u8], w: usize, h: usize) -> FocusMetrics {
    FocusMetrics {
        sharpness: laplacian_variance(gray, w, h),
        tenengrad: tenengrad(gray, w, h),
    }
}

pub fn hash_distance(a: &str, b: &str) -> u32 {
    if a.is_empty() || b.is_empty() {
        return 64;
    }
    let Ok(a) = u64::from_str_radix(a, 16) else {
        return 64;
    };
    let Ok(b) = u64::from_str_radix(b, 16) else {
        return 64;
    };
    (a ^ b).count_ones()
}

fn grayscale(image: &RgbImage) -> GrayImage {
    let mut pixels = Vec::with_capacity((image.width() * image.height()) as usize);
    for pixel in image.pixels() {
        let [r, g, b] = pixel.0;
        let luma = (54 * u32::from(r) + 183 * u32::from(g) + 19 * u32::from(b) + 128) >> 8;
        pixels.push(luma as u8);
    }
    GrayImage::from_raw(image.width(), image.height(), pixels)
        .expect("grayscale buffer matches image dimensions")
}

fn histogram(gray: &[u8]) -> [usize; 256] {
    let mut histogram = [0usize; 256];
    for value in gray {
        histogram[usize::from(*value)] += 1;
    }
    histogram
}

fn histogram_percentile(histogram: &[usize; 256], count: usize, pct: f64) -> u8 {
    if count == 0 {
        return 0;
    }
    let target = ((count - 1) as f64 * pct / 100.0).round() as usize;
    let mut seen = 0usize;
    for (value, occurrences) in histogram.iter().enumerate() {
        seen += occurrences;
        if seen > target {
            return value as u8;
        }
    }
    u8::MAX
}

fn laplacian_variance(gray: &[u8], w: usize, h: usize) -> f64 {
    if w < 3 || h < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut n = 0.0;
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let i = y * w + x;
            let lap = -4.0 * f64::from(gray[i])
                + f64::from(gray[i - 1])
                + f64::from(gray[i + 1])
                + f64::from(gray[i - w])
                + f64::from(gray[i + w]);
            sum += lap;
            sum_sq += lap * lap;
            n += 1.0;
        }
    }
    (sum_sq / n) - (sum / n).powi(2)
}

fn tenengrad(gray: &[u8], w: usize, h: usize) -> f64 {
    let mut dx_sum = 0.0;
    let mut dx_n: f64 = 0.0;
    for y in 0..h {
        for x in 0..(w - 1) {
            let d = f64::from(gray[y * w + x + 1]) - f64::from(gray[y * w + x]);
            dx_sum += d * d;
            dx_n += 1.0;
        }
    }
    let mut dy_sum = 0.0;
    let mut dy_n: f64 = 0.0;
    for y in 0..(h - 1) {
        for x in 0..w {
            let d = f64::from(gray[(y + 1) * w + x]) - f64::from(gray[y * w + x]);
            dy_sum += d * d;
            dy_n += 1.0;
        }
    }
    dx_sum / dx_n.max(1.0) + dy_sum / dy_n.max(1.0)
}

#[derive(Debug, Clone, Copy)]
struct BBoxScore {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    object_confidence: f64,
    completeness: f64,
    border_energy_fraction: f64,
}

#[derive(Debug, Clone, Copy)]
struct Component {
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
    pixels: usize,
    strong_pixels: usize,
    border_pixels: usize,
    energy: u64,
    score: f64,
}

fn robust_bbox(gray: &GrayImage) -> BBoxScore {
    let coarse = robust_bbox_at(gray, DETECTOR_LONG_EDGE);
    let coarse_area = bbox_area(&coarse);
    if gray.width().max(gray.height()) <= DETECTOR_LONG_EDGE
        || (coarse_area >= 0.025 && coarse.object_confidence >= 0.52)
    {
        return coarse;
    }

    let refined = robust_bbox_at(gray, DETECTOR_REFINEMENT_LONG_EDGE);
    let agreement = bbox_iou(&coarse, &refined);
    if refined.object_confidence >= coarse.object_confidence * 0.82
        && bbox_area(&refined) <= 0.45
        && (agreement >= 0.20 || coarse.object_confidence < 0.28)
    {
        refined
    } else {
        coarse
    }
}

fn robust_bbox_at(gray: &GrayImage, long_edge: u32) -> BBoxScore {
    let small = resize_long_edge(gray, long_edge);
    let w = small.width() as usize;
    let h = small.height() as usize;
    if w < 8 || h < 8 {
        return uncertain_bbox();
    }
    let pixels = small.as_raw();
    let integral = integral_image(pixels, w, h);
    let mut scores = vec![0u16; pixels.len()];
    let mut score_histogram = vec![0usize; 2048];
    let radius = ((w.min(h) as f64 * 0.015).round() as usize).clamp(3, 9);
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let i = y * w + x;
            let local = box_mean(&integral, w, h, x, y, radius);
            let value = i32::from(pixels[i]);
            let gradient = (i32::from(pixels[i + 1]) - i32::from(pixels[i - 1])).abs()
                + (i32::from(pixels[i + w]) - i32::from(pixels[i - w])).abs();
            let detail = (value - local).abs();
            let dark_contrast = (local - value).max(0);
            let score = (gradient * 2 + detail * 3 + dark_contrast).clamp(0, 2047) as u16;
            scores[i] = score;
            score_histogram[usize::from(score)] += 1;
        }
    }
    let scored_pixels = (w - 2) * (h - 2);
    let threshold = histogram_percentile_u16(&score_histogram, scored_pixels, 98.7).max(48);
    let mut strong = vec![false; scores.len()];
    for (index, score) in scores.iter().enumerate() {
        strong[index] = *score >= threshold;
    }
    let mask = dilate_mask(&strong, w, h);
    let mut visited = vec![false; mask.len()];
    let mut components = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }
        let component = collect_component(start, &mask, &strong, &scores, &mut visited, w, h);
        let bbox_area = (component.x2 - component.x1 + 1) * (component.y2 - component.y1 + 1);
        if component.strong_pixels < 2 || bbox_area > w * h * 3 / 5 || component.pixels < 4 {
            continue;
        }
        components.push(component);
    }
    components.sort_by(|left, right| right.score.total_cmp(&left.score));
    let Some(best) = components.first().copied() else {
        return uncertain_bbox();
    };
    let runner_up = components
        .get(1)
        .map(|component| component.score)
        .unwrap_or(0.0);
    let dominance = best.score / (best.score + runner_up).max(1e-9);
    let bbox_area = (best.x2 - best.x1 + 1) * (best.y2 - best.y1 + 1);
    let density = best.strong_pixels as f64 / bbox_area as f64;
    let average_energy = best.energy as f64 / best.strong_pixels.max(1) as f64;
    let energy_strength = ((average_energy / f64::from(threshold) - 1.0) / 2.0).clamp(0.0, 1.0);
    let size_reliability = (best.strong_pixels as f64 / 20.0).sqrt().clamp(0.25, 1.0);
    let area_fraction = bbox_area as f64 / (w * h) as f64;
    let area_reliability = if area_fraction <= 0.35 { 1.0 } else { 0.45 };
    let object_confidence = ((0.16
        + 0.34 * dominance
        + 0.22 * density.sqrt()
        + 0.16 * energy_strength
        + 0.12 * size_reliability)
        * area_reliability)
        .clamp(0.0, 0.98);
    let pad = 2usize;
    let x1 = best.x1.saturating_sub(pad);
    let y1 = best.y1.saturating_sub(pad);
    let x2 = (best.x2 + pad + 1).min(w);
    let y2 = (best.y2 + pad + 1).min(h);
    let margin = x1.min(y1).min(w - x2).min(h - y2);
    let margin_score = (margin as f64 / (w.min(h) as f64 * 0.035).max(1.0)).clamp(0.0, 1.0);
    let border_fraction = best.border_pixels as f64 / best.pixels.max(1) as f64;
    let completeness = if object_confidence < 0.3 {
        0.5
    } else {
        (0.85 * margin_score + 0.15 * (1.0 - border_fraction)).clamp(0.0, 1.0)
    };

    BBoxScore {
        x1: x1 as f64 / w as f64,
        y1: y1 as f64 / h as f64,
        x2: x2 as f64 / w as f64,
        y2: y2 as f64 / h as f64,
        object_confidence,
        completeness,
        border_energy_fraction: border_fraction,
    }
}

fn bbox_area(bbox: &BBoxScore) -> f64 {
    ((bbox.x2 - bbox.x1).max(0.0) * (bbox.y2 - bbox.y1).max(0.0)).clamp(0.0, 1.0)
}

fn bbox_iou(left: &BBoxScore, right: &BBoxScore) -> f64 {
    let width = (left.x2.min(right.x2) - left.x1.max(right.x1)).max(0.0);
    let height = (left.y2.min(right.y2) - left.y1.max(right.y1)).max(0.0);
    let intersection = width * height;
    let union = bbox_area(left) + bbox_area(right) - intersection;
    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

fn uncertain_bbox() -> BBoxScore {
    BBoxScore {
        x1: 0.0,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
        object_confidence: 0.0,
        completeness: 0.5,
        border_energy_fraction: 0.0,
    }
}

fn focus_in_bbox(gray: &GrayImage, bbox: &BBoxScore) -> FocusMetrics {
    let width = gray.width();
    let height = gray.height();
    let x1 = (bbox.x1.clamp(0.0, 1.0) * f64::from(width)).floor() as u32;
    let y1 = (bbox.y1.clamp(0.0, 1.0) * f64::from(height)).floor() as u32;
    let x2 = (bbox.x2.clamp(0.0, 1.0) * f64::from(width)).ceil() as u32;
    let y2 = (bbox.y2.clamp(0.0, 1.0) * f64::from(height)).ceil() as u32;
    if x2 <= x1 + 2 || y2 <= y1 + 2 {
        return FocusMetrics {
            sharpness: 0.0,
            tenengrad: 0.0,
        };
    }
    let crop = imageops::crop_imm(gray, x1, y1, x2.min(width) - x1, y2.min(height) - y1).to_image();
    cpu_focus_metrics(crop.as_raw(), crop.width() as usize, crop.height() as usize)
}

fn resize_long_edge(gray: &GrayImage, long_edge: u32) -> GrayImage {
    let max_edge = gray.width().max(gray.height());
    if max_edge <= long_edge {
        return gray.clone();
    }
    let scale = f64::from(long_edge) / f64::from(max_edge);
    let width = (f64::from(gray.width()) * scale).round().max(1.0) as u32;
    let height = (f64::from(gray.height()) * scale).round().max(1.0) as u32;
    imageops::resize(gray, width, height, imageops::FilterType::Triangle)
}

fn integral_image(pixels: &[u8], w: usize, h: usize) -> Vec<u64> {
    let stride = w + 1;
    let mut integral = vec![0u64; stride * (h + 1)];
    for y in 0..h {
        let mut row_sum = 0u64;
        for x in 0..w {
            row_sum += u64::from(pixels[y * w + x]);
            integral[(y + 1) * stride + x + 1] = integral[y * stride + x + 1] + row_sum;
        }
    }
    integral
}

fn box_mean(integral: &[u64], w: usize, h: usize, x: usize, y: usize, radius: usize) -> i32 {
    let stride = w + 1;
    let x1 = x.saturating_sub(radius);
    let y1 = y.saturating_sub(radius);
    let x2 = (x + radius + 1).min(w);
    let y2 = (y + radius + 1).min(h);
    let sum = integral[y2 * stride + x2] + integral[y1 * stride + x1]
        - integral[y1 * stride + x2]
        - integral[y2 * stride + x1];
    let area = (x2 - x1) * (y2 - y1);
    (sum / area.max(1) as u64) as i32
}

fn histogram_percentile_u16(histogram: &[usize], count: usize, pct: f64) -> u16 {
    let target = ((count.saturating_sub(1)) as f64 * pct / 100.0).round() as usize;
    let mut seen = 0usize;
    for (value, occurrences) in histogram.iter().enumerate() {
        seen += occurrences;
        if seen > target {
            return value as u16;
        }
    }
    (histogram.len().saturating_sub(1)) as u16
}

fn dilate_mask(mask: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut dilated = vec![false; mask.len()];
    for y in 0..h {
        for x in 0..w {
            if !mask[y * w + x] {
                continue;
            }
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    dilated[ny * w + nx] = true;
                }
            }
        }
    }
    dilated
}

#[allow(clippy::too_many_arguments)]
fn collect_component(
    start: usize,
    mask: &[bool],
    strong: &[bool],
    scores: &[u16],
    visited: &mut [bool],
    w: usize,
    h: usize,
) -> Component {
    let mut queue = VecDeque::from([start]);
    visited[start] = true;
    let mut x1 = w;
    let mut y1 = h;
    let mut x2 = 0usize;
    let mut y2 = 0usize;
    let mut pixels = 0usize;
    let mut strong_pixels = 0usize;
    let mut border_pixels = 0usize;
    let mut energy = 0u64;
    while let Some(index) = queue.pop_front() {
        let x = index % w;
        let y = index / w;
        x1 = x1.min(x);
        y1 = y1.min(y);
        x2 = x2.max(x);
        y2 = y2.max(y);
        pixels += 1;
        if strong[index] {
            strong_pixels += 1;
            energy += u64::from(scores[index]);
        }
        border_pixels += usize::from(x == 0 || y == 0 || x + 1 == w || y + 1 == h);
        for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
            for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                let neighbor = ny * w + nx;
                if mask[neighbor] && !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
    }
    let bbox_area = (x2 - x1 + 1) * (y2 - y1 + 1);
    let density = strong_pixels as f64 / bbox_area.max(1) as f64;
    let border_penalty = if border_pixels > 0 { 0.7 } else { 1.0 };
    let score = energy as f64 * (0.35 + 0.65 * density.sqrt()) * border_penalty
        / (bbox_area as f64).powf(0.12).max(1.0);
    Component {
        x1,
        y1,
        x2,
        y2,
        pixels,
        strong_pixels,
        border_pixels,
        energy,
        score,
    }
}

fn difference_hash(gray: &GrayImage) -> String {
    let small = imageops::resize(gray, 9, 8, imageops::FilterType::Lanczos3);
    let mut value: u64 = 0;
    for y in 0..8 {
        for x in 0..8 {
            let left = small.get_pixel(x, y).0[0];
            let right = small.get_pixel(x + 1, y).0[0];
            value = (value << 1) | u64::from(right > left);
        }
    }
    format!("{value:016x}")
}

fn edge_descriptor(gray: &GrayImage) -> Vec<u8> {
    let w = gray.width() as usize;
    let h = gray.height() as usize;
    let pixels = gray.as_raw();
    let mut edges = vec![0u8; pixels.len()];
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let i = y * w + x;
            let dx = (i16::from(pixels[i + 1]) - i16::from(pixels[i - 1])).unsigned_abs();
            let dy = (i16::from(pixels[i + w]) - i16::from(pixels[i - w])).unsigned_abs();
            edges[i] = (dx + dy).min(u16::from(u8::MAX)) as u8;
        }
    }
    edges
}

fn foreground_mask(gray: &GrayImage) -> Vec<u8> {
    let w = gray.width() as usize;
    let h = gray.height() as usize;
    let pixels = gray.as_raw();
    let border = 6usize.min(w.min(h) / 4);
    let mut border_histogram = [0usize; 256];
    let mut border_count = 0usize;
    for y in 0..h {
        for x in 0..w {
            if x < border || y < border || x + border >= w || y + border >= h {
                border_histogram[usize::from(pixels[y * w + x])] += 1;
                border_count += 1;
            }
        }
    }
    let background = histogram_percentile(&border_histogram, border_count, 50.0);
    let mut deviation_histogram = [0usize; 256];
    for y in 0..h {
        for x in 0..w {
            if x < border || y < border || x + border >= w || y + border >= h {
                let deviation = pixels[y * w + x].abs_diff(background);
                deviation_histogram[usize::from(deviation)] += 1;
            }
        }
    }
    let mad = histogram_percentile(&deviation_histogram, border_count, 70.0);
    let threshold = (u16::from(mad) * 3).clamp(12, 72) as u8;
    pixels
        .iter()
        .map(|value| u8::from(value.abs_diff(background) >= threshold))
        .collect()
}

fn correlation_distance(left: &[u8], right: &[u8]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return 1.0;
    }
    let n = left.len() as f64;
    let left_mean = left.iter().map(|value| f64::from(*value)).sum::<f64>() / n;
    let right_mean = right.iter().map(|value| f64::from(*value)).sum::<f64>() / n;
    let mut covariance = 0.0;
    let mut left_variance = 0.0;
    let mut right_variance = 0.0;
    for (left, right) in left.iter().zip(right) {
        let left = f64::from(*left) - left_mean;
        let right = f64::from(*right) - right_mean;
        covariance += left * right;
        left_variance += left * left;
        right_variance += right * right;
    }
    if left_variance < 1e-9 || right_variance < 1e-9 {
        return left
            .iter()
            .zip(right)
            .map(|(left, right)| f64::from(left.abs_diff(*right)) / 255.0)
            .sum::<f64>()
            / n;
    }
    let correlation = covariance / (left_variance.sqrt() * right_variance.sqrt());
    ((1.0 - correlation.clamp(-1.0, 1.0)) * 0.5).clamp(0.0, 1.0)
}

fn cosine_distance(left: &[u8], right: &[u8]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return 1.0;
    }
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left, right) in left.iter().zip(right) {
        let left = f64::from(*left);
        let right = f64::from(*right);
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm < 1e-9 || right_norm < 1e-9 {
        return 1.0;
    }
    (1.0 - dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(0.0, 1.0)
}

fn mask_distance(left: &[u8], right: &[u8]) -> (f64, f64) {
    if left.len() != right.len() || left.is_empty() {
        return (1.0, 0.0);
    }
    let mut intersection = 0usize;
    let mut union = 0usize;
    for (left, right) in left.iter().zip(right) {
        intersection += usize::from(*left != 0 && *right != 0);
        union += usize::from(*left != 0 || *right != 0);
    }
    if union < 8 {
        return (1.0, 0.35);
    }
    (
        1.0 - intersection as f64 / union as f64,
        (union as f64 / 80.0).sqrt().clamp(0.35, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::{compare_similarity, score_image, similarity_features};

    fn synthetic_frame(x: u32, vertical: bool) -> RgbImage {
        let mut image = RgbImage::from_pixel(320, 240, Rgb([165, 190, 210]));
        if vertical {
            for y in 98..142 {
                for px in x.saturating_sub(4)..=(x + 4).min(319) {
                    image.put_pixel(px, y, Rgb([20, 24, 29]));
                }
            }
        } else {
            for y in 116..124 {
                for px in x.saturating_sub(22)..=(x + 22).min(319) {
                    image.put_pixel(px, y, Rgb([20, 24, 29]));
                }
            }
        }
        image
    }

    fn features(image: &RgbImage) -> super::SimilarityFeatures {
        let score = score_image(image);
        similarity_features(image, &score.metrics)
    }

    #[test]
    fn subject_descriptor_ignores_position_but_detects_pose() {
        let horizontal_left = features(&synthetic_frame(90, false));
        let horizontal_right = features(&synthetic_frame(220, false));
        let vertical = features(&synthetic_frame(220, true));
        let moved = compare_similarity(&horizontal_left, &horizontal_right);
        let changed = compare_similarity(&horizontal_right, &vertical);
        assert!(
            moved.distance < changed.distance,
            "{moved:?} vs {changed:?}"
        );
        assert!(horizontal_left.confidence > 0.25);
    }

    #[test]
    fn identical_subject_descriptors_have_zero_distance() {
        let features = features(&synthetic_frame(160, false));
        let comparison = compare_similarity(&features, &features);
        assert!(comparison.distance < 1e-9, "{comparison:?}");
    }
}
