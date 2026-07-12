use burst_core::score_image_with;
pub use burst_core::{
    FeatureScore, SimilarityComparison, SimilarityFeatures, compare_similarity, hash_distance,
    similarity_features, update_subject_focus,
};
use image::RgbImage;

use crate::acceleration::AccelerationPlan;

pub fn score_image(image: &RgbImage, acceleration: AccelerationPlan) -> FeatureScore {
    score_image_with(image, |gray, width, height| {
        acceleration.score(gray, width, height)
    })
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::score_image;
    use crate::acceleration::{AccelerationPlan, FocusBackend, best_cpu_backend};
    use crate::types::AccelerationPreference;

    fn test_image() -> RgbImage {
        RgbImage::from_fn(32, 24, |x, y| {
            let value = ((x * 17 + y * 29) % 256) as u8;
            Rgb([value, value.wrapping_add(31), value.wrapping_mul(3)])
        })
    }

    #[test]
    fn portable_preference_is_the_explicit_reference() {
        let score = score_image(
            &test_image(),
            AccelerationPlan::resolve(AccelerationPreference::Portable),
        );
        assert_eq!(score.backend, "cpu_portable");
    }

    #[test]
    fn cpu_preference_uses_the_resolved_architecture_backend() {
        let score = score_image(
            &test_image(),
            AccelerationPlan::resolve(AccelerationPreference::Cpu),
        );
        assert_eq!(score.backend, best_cpu_backend().id());
        assert_ne!(best_cpu_backend(), FocusBackend::Metal);
        assert_ne!(best_cpu_backend(), FocusBackend::Cuda);
    }
}
