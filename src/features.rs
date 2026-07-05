use image::{GrayImage, Luma, RgbImage, imageops};

use crate::types::{AccelerationPreference, QualityMetrics};

#[derive(Debug, Clone)]
pub struct FeatureScore {
    pub metrics: QualityMetrics,
    pub backend: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct NativeFocusMetrics {
    sharpness: f64,
    tenengrad: f64,
}

pub fn score_image(image: &RgbImage, acceleration: AccelerationPreference) -> FeatureScore {
    let (width, height) = image.dimensions();
    if width < 8 || height < 8 {
        return FeatureScore {
            metrics: QualityMetrics::default(),
            backend: "cpu_small_image".to_string(),
            notes: vec!["Image is too small for full scoring.".to_string()],
        };
    }

    let gray = grayscale(image);
    let w = width as usize;
    let h = height as usize;
    let (focus, backend, notes) = focus_metrics(&gray, w, h, acceleration);
    let p5 = percentile(gray.clone(), 5.0);
    let p95 = percentile(gray.clone(), 95.0);
    let contrast = ((p95 - p5) / 255.0).clamp(0.0, 1.0);
    let clipped = gray.iter().filter(|v| **v < 3.0 || **v > 252.0).count() as f64;
    let clipped_fraction = clipped / gray.len() as f64;
    let mean_luma = gray.iter().sum::<f64>() / (gray.len() as f64 * 255.0);
    let mean_penalty = ((mean_luma - 0.52).abs() / 0.52).min(1.0);
    let exposure_score = (1.0 - (clipped_fraction * 6.0 + mean_penalty * 0.35).min(1.0)).max(0.0);
    let bbox = robust_bbox(&gray, w, h);
    let dhash = difference_hash(&gray, w, h);

    FeatureScore {
        metrics: QualityMetrics {
            sharpness: focus.sharpness,
            tenengrad: focus.tenengrad,
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

fn focus_metrics(
    gray: &[f64],
    w: usize,
    h: usize,
    acceleration: AccelerationPreference,
) -> (NativeFocusMetrics, String, Vec<String>) {
    if matches!(
        acceleration,
        AccelerationPreference::Auto | AccelerationPreference::Metal
    ) {
        #[cfg(all(target_os = "macos", feature = "metal-accel"))]
        {
            match crate::metal_accel::focus_metrics(gray, w, h) {
                Ok(metrics) => {
                    return (
                        NativeFocusMetrics {
                            sharpness: metrics.sharpness,
                            tenengrad: metrics.tenengrad,
                        },
                        "metal".to_string(),
                        Vec::new(),
                    );
                }
                Err(err) if acceleration == AccelerationPreference::Metal => {
                    return (
                        cpu_focus_metrics(gray, w, h),
                        "cpu_rayon".to_string(),
                        vec![format!(
                            "Metal focus scoring failed; used CPU fallback: {err}"
                        )],
                    );
                }
                Err(_) => {}
            }
        }
    }

    (
        cpu_focus_metrics(gray, w, h),
        "cpu_rayon".to_string(),
        match acceleration {
            AccelerationPreference::Metal => {
                vec!["Metal was requested but this build has no working Metal scorer.".to_string()]
            }
            AccelerationPreference::Cuda => {
                vec![
                    "CUDA was requested but no CUDA scorer is implemented in this build."
                        .to_string(),
                ]
            }
            AccelerationPreference::OpenCl => {
                vec![
                    "OpenCL was requested but no OpenCL scorer is implemented in this build."
                        .to_string(),
                ]
            }
            _ => Vec::new(),
        },
    )
}

fn cpu_focus_metrics(gray: &[f64], w: usize, h: usize) -> NativeFocusMetrics {
    NativeFocusMetrics {
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

fn grayscale(image: &RgbImage) -> Vec<f64> {
    image
        .pixels()
        .map(|p| {
            let [r, g, b] = p.0;
            0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)
        })
        .collect()
}

fn laplacian_variance(gray: &[f64], w: usize, h: usize) -> f64 {
    if w < 3 || h < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut n = 0.0;
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let i = y * w + x;
            let lap = -4.0 * gray[i] + gray[i - 1] + gray[i + 1] + gray[i - w] + gray[i + w];
            sum += lap;
            sum_sq += lap * lap;
            n += 1.0;
        }
    }
    if n == 0.0 {
        0.0
    } else {
        (sum_sq / n) - (sum / n).powi(2)
    }
}

fn tenengrad(gray: &[f64], w: usize, h: usize) -> f64 {
    let mut dx_sum: f64 = 0.0;
    let mut dx_n: f64 = 0.0;
    for y in 0..h {
        for x in 0..(w - 1) {
            let d = gray[y * w + x + 1] - gray[y * w + x];
            dx_sum += d * d;
            dx_n += 1.0;
        }
    }
    let mut dy_sum: f64 = 0.0;
    let mut dy_n: f64 = 0.0;
    for y in 0..(h - 1) {
        for x in 0..w {
            let d = gray[(y + 1) * w + x] - gray[y * w + x];
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

fn robust_bbox(gray: &[f64], w: usize, h: usize) -> BBoxScore {
    let mut grad = vec![0.0; gray.len()];
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let i = y * w + x;
            grad[i] = (gray[i + 1] - gray[i - 1]).abs() + (gray[i + w] - gray[i - w]).abs();
        }
    }

    let border_width = ((w.min(h) as f64 * 0.035).round() as usize).max(2);
    let mut border = Vec::with_capacity((w + h) * border_width * 2);
    for y in 0..h {
        for x in 0..w {
            if x < border_width
                || x >= w - border_width
                || y < border_width
                || y >= h - border_width
            {
                border.push(gray[y * w + x]);
            }
        }
    }
    let bg = percentile(border.clone(), 50.0);
    let mut deviations: Vec<f64> = border.into_iter().map(|v| (v - bg).abs()).collect();
    let bg_mad = percentile(std::mem::take(&mut deviations), 50.0) + 1.0;
    let grad_p97 = percentile(grad.clone(), 97.0) + 1.0;

    let mut saliency = Vec::with_capacity(gray.len());
    for (i, value) in gray.iter().enumerate() {
        let difference = ((*value - bg).abs() / (bg_mad * 2.5).max(12.0)).min(4.0);
        saliency.push((grad[i] / grad_p97) + difference);
    }
    let threshold = percentile(saliency.clone(), 96.5).max(0.85);
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut total_energy = 0.0;
    let mut border_energy = 0.0;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let e = saliency[i];
            total_energy += e;
            if x < border_width
                || x >= w - border_width
                || y < border_width
                || y >= h - border_width
            {
                border_energy += e;
            }
            if e >= threshold {
                xs.push(x as f64);
                ys.push(y as f64);
            }
        }
    }

    let border_fraction = (border_energy / total_energy.max(1e-9)).min(1.0);
    let object_confidence = (xs.len() as f64 / (w * h) as f64 / 0.025).clamp(0.0, 1.0);
    if xs.len() < 16 {
        return BBoxScore {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
            object_confidence,
            completeness: 0.2,
            border_energy_fraction: border_fraction,
        };
    }

    let x1 = percentile(xs.clone(), 1.0);
    let x2 = percentile(xs, 99.0);
    let y1 = percentile(ys.clone(), 1.0);
    let y2 = percentile(ys, 99.0);
    let margin = x1.min(y1).min((w - 1) as f64 - x2).min((h - 1) as f64 - y2);
    let margin_score = (margin / ((w.min(h) as f64) * 0.045).max(1.0)).clamp(0.0, 1.0);
    let border_score = (1.0 - (border_fraction / 0.22).min(1.0)).max(0.0);
    let completeness = (0.72 * margin_score + 0.28 * border_score).clamp(0.0, 1.0);

    BBoxScore {
        x1: x1 / w as f64,
        y1: y1 / h as f64,
        x2: x2 / w as f64,
        y2: y2 / h as f64,
        object_confidence,
        completeness,
        border_energy_fraction: border_fraction,
    }
}

fn difference_hash(gray: &[f64], w: usize, h: usize) -> String {
    let mut image = GrayImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let value = gray[y * w + x].round().clamp(0.0, 255.0) as u8;
            image.put_pixel(x as u32, y as u32, Luma([value]));
        }
    }
    let small = imageops::resize(&image, 9, 8, imageops::FilterType::Lanczos3);
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

fn percentile(mut values: Vec<f64>, pct: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let idx = ((values.len() - 1) as f64 * (pct / 100.0)).round() as usize;
    values[idx.min(values.len() - 1)]
}
