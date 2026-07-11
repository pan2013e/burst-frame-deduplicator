use image::RgbImage;

#[cfg(all(target_os = "macos", feature = "metal-accel"))]
use burst_core::FocusMetrics;
pub use burst_core::{
    FeatureScore, SimilarityComparison, SimilarityFeatures, compare_similarity, hash_distance,
    similarity_features, update_subject_focus,
};
use burst_core::{FocusResult, cpu_focus_metrics, score_image_with};

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
                    return FocusResult {
                        metrics: cpu_focus_metrics(gray, width, height),
                        backend: "cpu_rayon".to_string(),
                        notes: vec![format!(
                            "Metal focus scoring failed; used CPU fallback: {err}"
                        )],
                    };
                }
                Err(_) => {}
            }
        }
    }

    let notes = match acceleration {
        AccelerationPreference::Metal => {
            vec!["Metal was requested but this build has no working Metal scorer.".to_string()]
        }
        AccelerationPreference::Cuda => {
            vec!["CUDA was requested but no CUDA scorer is implemented in this build.".to_string()]
        }
        AccelerationPreference::OpenCl => {
            vec![
                "OpenCL was requested but no OpenCL scorer is implemented in this build."
                    .to_string(),
            ]
        }
        _ => Vec::new(),
    };
    FocusResult {
        metrics: cpu_focus_metrics(gray, width, height),
        backend: "cpu_rayon".to_string(),
        notes,
    }
}
