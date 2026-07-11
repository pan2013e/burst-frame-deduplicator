#[cfg(all(target_os = "macos", feature = "metal-accel"))]
use burst_core::FocusMetrics;
use burst_core::cpu_focus_metrics;
pub use burst_core::{
    FeatureScore, SimilarityComparison, SimilarityFeatures, compare_similarity, hash_distance,
    similarity_features, update_subject_focus,
};
use burst_core::{FocusResult, score_image_with};
use image::RgbImage;

use crate::types::AccelerationPreference;

pub fn score_image(image: &RgbImage, acceleration: AccelerationPreference) -> FeatureScore {
    score_image_with(image, |gray, width, height| {
        focus_metrics(gray, width, height, acceleration)
    })
}

fn focus_metrics(
    gray: &[u8],
    width: usize,
    height: usize,
    acceleration: AccelerationPreference,
) -> FocusResult {
    if acceleration == AccelerationPreference::Cpu {
        return scalar_cpu_focus_metrics(gray, width, height);
    }

    if acceleration == AccelerationPreference::Avx2 {
        let mut result = best_cpu_focus_metrics(gray, width, height);
        if result.backend != "cpu_avx2" {
            result.notes.push(
                "AVX2 was requested but is unavailable on this CPU or in this build; used the portable scalar scorer."
                    .to_string(),
            );
        }
        return result;
    }

    if acceleration == AccelerationPreference::Cuda {
        #[cfg(all(target_os = "linux", feature = "cuda-accel"))]
        {
            match crate::cuda_accel::focus_metrics(gray, width, height) {
                Ok(metrics) => {
                    return FocusResult {
                        metrics,
                        backend: "cuda".to_string(),
                        notes: Vec::new(),
                    };
                }
                Err(err) => {
                    let mut fallback = best_cpu_focus_metrics(gray, width, height);
                    fallback.notes.push(format!(
                        "CUDA focus scoring failed; used CPU fallback: {err}"
                    ));
                    return fallback;
                }
            }
        }

        #[cfg(not(all(target_os = "linux", feature = "cuda-accel")))]
        {
            let mut fallback = best_cpu_focus_metrics(gray, width, height);
            fallback.notes.push(
                "CUDA was requested but this build does not include the cuda-accel feature."
                    .to_string(),
            );
            return fallback;
        }
    }

    if matches!(
        acceleration,
        AccelerationPreference::Auto | AccelerationPreference::Metal
    ) {
        #[cfg(all(target_os = "macos", feature = "metal-accel"))]
        {
            match crate::metal_accel::focus_metrics(gray, width, height) {
                Ok(metrics) => {
                    return FocusResult {
                        metrics: FocusMetrics {
                            sharpness: metrics.sharpness,
                            tenengrad: metrics.tenengrad,
                        },
                        backend: "metal".to_string(),
                        notes: Vec::new(),
                    };
                }
                Err(err) if acceleration == AccelerationPreference::Metal => {
                    let mut fallback = best_cpu_focus_metrics(gray, width, height);
                    fallback.notes.push(format!(
                        "Metal focus scoring failed; used CPU fallback: {err}"
                    ));
                    return fallback;
                }
                Err(_) => {}
            }
        }
    }

    let mut fallback = best_cpu_focus_metrics(gray, width, height);
    fallback.notes.extend(match acceleration {
        AccelerationPreference::Metal => {
            vec!["Metal was requested but this build has no working Metal scorer.".to_string()]
        }
        AccelerationPreference::OpenCl => {
            vec![
                "OpenCL was requested but no OpenCL scorer is implemented in this build."
                    .to_string(),
            ]
        }
        _ => Vec::new(),
    });
    fallback
}

fn best_cpu_focus_metrics(gray: &[u8], width: usize, height: usize) -> FocusResult {
    #[cfg(all(target_os = "linux", feature = "avx2-accel"))]
    {
        crate::cpu_accel::focus_metrics(gray, width, height)
    }

    #[cfg(not(all(target_os = "linux", feature = "avx2-accel")))]
    {
        scalar_cpu_focus_metrics(gray, width, height)
    }
}

fn scalar_cpu_focus_metrics(gray: &[u8], width: usize, height: usize) -> FocusResult {
    FocusResult {
        metrics: cpu_focus_metrics(gray, width, height),
        backend: "cpu_scalar".to_string(),
        notes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::score_image;
    use crate::types::AccelerationPreference;

    fn test_image() -> RgbImage {
        RgbImage::from_fn(32, 24, |x, y| {
            let value = ((x * 17 + y * 29) % 256) as u8;
            Rgb([value, value.wrapping_add(31), value.wrapping_mul(3)])
        })
    }

    #[test]
    fn cpu_preference_is_the_explicit_scalar_reference() {
        let score = score_image(&test_image(), AccelerationPreference::Cpu);
        assert_eq!(score.backend, "cpu_scalar");
    }

    #[cfg(all(target_os = "linux", feature = "avx2-accel"))]
    #[test]
    fn avx2_preference_reports_the_runtime_dispatched_backend() {
        let score = score_image(&test_image(), AccelerationPreference::Avx2);
        assert_eq!(score.backend, crate::cpu_accel::backend_name());
        assert_eq!(score.backend == "cpu_avx2", score.notes.is_empty());
    }
}
