use burst_core::{FocusMetrics, FocusResult, cpu_focus_metrics};

pub fn focus_metrics(gray: &[u8], width: usize, height: usize) -> FocusResult {
    let Some(expected_len) = width.checked_mul(height) else {
        return invalid_buffer_result("grayscale dimensions overflow usize");
    };
    if width == 0 || height == 0 || gray.len() != expected_len {
        return invalid_buffer_result("grayscale buffer does not match its dimensions");
    }

    #[cfg(all(
        feature = "avx2-accel",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    let metrics = if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: Runtime feature detection above guarantees AVX2 support, and the
        // buffer length and dimensions were validated before entering the kernel.
        unsafe { x86::focus_metrics_avx2(gray, width, height) }
    } else {
        cpu_focus_metrics(gray, width, height)
    };

    #[cfg(all(feature = "neon-accel", target_arch = "aarch64"))]
    let metrics = if std::arch::is_aarch64_feature_detected!("neon") {
        // SAFETY: Runtime feature detection above guarantees NEON support, and the
        // buffer length and dimensions were validated before entering the kernel.
        unsafe { aarch64::focus_metrics_neon(gray, width, height) }
    } else {
        cpu_focus_metrics(gray, width, height)
    };

    #[cfg(not(any(
        all(
            feature = "avx2-accel",
            any(target_arch = "x86", target_arch = "x86_64")
        ),
        all(feature = "neon-accel", target_arch = "aarch64")
    )))]
    let metrics = cpu_focus_metrics(gray, width, height);

    FocusResult {
        metrics,
        backend: backend_name().to_string(),
        notes: Vec::new(),
    }
}

pub fn backend_name() -> &'static str {
    #[cfg(all(
        feature = "avx2-accel",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    if std::arch::is_x86_feature_detected!("avx2") {
        return "cpu_avx2";
    }

    #[cfg(all(feature = "neon-accel", target_arch = "aarch64"))]
    if std::arch::is_aarch64_feature_detected!("neon") {
        return "cpu_neon";
    }

    "cpu_scalar"
}

fn invalid_buffer_result(note: &str) -> FocusResult {
    FocusResult {
        metrics: FocusMetrics {
            sharpness: 0.0,
            tenengrad: 0.0,
        },
        backend: backend_name().to_string(),
        notes: vec![format!("Invalid grayscale buffer: {note}.")],
    }
}

#[cfg(all(
    feature = "avx2-accel",
    any(target_arch = "x86", target_arch = "x86_64")
))]
mod x86 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    use burst_core::FocusMetrics;

    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn focus_metrics_avx2(
        gray: &[u8],
        width: usize,
        height: usize,
    ) -> FocusMetrics {
        // SAFETY: The caller verifies AVX2 support and validates that the slice
        // contains exactly width * height pixels. Every vector load below is
        // bounded by its corresponding loop condition; the remaining pixels use
        // scalar tails.
        unsafe {
            let (lap_sum, lap_sq_sum, lap_count) = laplacian_sums(gray, width, height);
            let (dx_sum, dx_count) = horizontal_gradient_sum(gray, width, height);
            let (dy_sum, dy_count) = vertical_gradient_sum(gray, width, height);

            let sharpness = if lap_count == 0 {
                0.0
            } else {
                let count = lap_count as f64;
                let mean = lap_sum as f64 / count;
                lap_sq_sum as f64 / count - mean * mean
            };
            let tenengrad = dx_sum as f64 / (dx_count as f64).max(1.0)
                + dy_sum as f64 / (dy_count as f64).max(1.0);

            FocusMetrics {
                sharpness,
                tenengrad,
            }
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn laplacian_sums(gray: &[u8], width: usize, height: usize) -> (i64, u64, usize) {
        if width < 3 || height < 3 {
            return (0, 0, 0);
        }

        unsafe {
            let mut vector_sum_low = _mm256_setzero_si256();
            let mut vector_sum_high = _mm256_setzero_si256();
            let mut vector_sq_low = _mm256_setzero_si256();
            let mut vector_sq_high = _mm256_setzero_si256();
            let ones = _mm256_set1_epi16(1);
            let mut scalar_sum = 0i64;
            let mut scalar_sq_sum = 0u64;

            for y in 1..(height - 1) {
                let row = y * width;
                let mut x = 1usize;
                while x + 16 < width {
                    let index = row + x;
                    let center = load_u8x16_as_i16(gray.as_ptr().add(index));
                    let left = load_u8x16_as_i16(gray.as_ptr().add(index - 1));
                    let right = load_u8x16_as_i16(gray.as_ptr().add(index + 1));
                    let up = load_u8x16_as_i16(gray.as_ptr().add(index - width));
                    let down = load_u8x16_as_i16(gray.as_ptr().add(index + width));
                    let neighbors =
                        _mm256_add_epi16(_mm256_add_epi16(left, right), _mm256_add_epi16(up, down));
                    let laplacian = _mm256_sub_epi16(neighbors, _mm256_slli_epi16(center, 2));

                    let pair_sums = _mm256_madd_epi16(laplacian, ones);
                    accumulate_i32_as_i64(pair_sums, &mut vector_sum_low, &mut vector_sum_high);
                    let pair_squares = _mm256_madd_epi16(laplacian, laplacian);
                    accumulate_i32_as_i64(pair_squares, &mut vector_sq_low, &mut vector_sq_high);
                    x += 16;
                }

                for x in x..(width - 1) {
                    let index = row + x;
                    let laplacian = -4 * i64::from(gray[index])
                        + i64::from(gray[index - 1])
                        + i64::from(gray[index + 1])
                        + i64::from(gray[index - width])
                        + i64::from(gray[index + width]);
                    scalar_sum += laplacian;
                    scalar_sq_sum += (laplacian * laplacian) as u64;
                }
            }

            let sum = scalar_sum
                + horizontal_sum_i64(vector_sum_low)
                + horizontal_sum_i64(vector_sum_high);
            let square_sum = scalar_sq_sum
                + horizontal_sum_i64(vector_sq_low) as u64
                + horizontal_sum_i64(vector_sq_high) as u64;
            (sum, square_sum, (width - 2) * (height - 2))
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn horizontal_gradient_sum(gray: &[u8], width: usize, height: usize) -> (u64, usize) {
        if width < 2 {
            return (0, 0);
        }

        unsafe {
            let mut vector_low = _mm256_setzero_si256();
            let mut vector_high = _mm256_setzero_si256();
            let mut scalar_sum = 0u64;

            for y in 0..height {
                let row = y * width;
                let mut x = 0usize;
                while x + 16 < width {
                    let index = row + x;
                    let current = load_u8x16_as_i16(gray.as_ptr().add(index));
                    let right = load_u8x16_as_i16(gray.as_ptr().add(index + 1));
                    let difference = _mm256_sub_epi16(right, current);
                    let pair_squares = _mm256_madd_epi16(difference, difference);
                    accumulate_i32_as_i64(pair_squares, &mut vector_low, &mut vector_high);
                    x += 16;
                }

                for x in x..(width - 1) {
                    let difference = i64::from(gray[row + x + 1]) - i64::from(gray[row + x]);
                    scalar_sum += (difference * difference) as u64;
                }
            }

            let sum = scalar_sum
                + horizontal_sum_i64(vector_low) as u64
                + horizontal_sum_i64(vector_high) as u64;
            (sum, height * (width - 1))
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn vertical_gradient_sum(gray: &[u8], width: usize, height: usize) -> (u64, usize) {
        if height < 2 {
            return (0, 0);
        }

        unsafe {
            let mut vector_low = _mm256_setzero_si256();
            let mut vector_high = _mm256_setzero_si256();
            let mut scalar_sum = 0u64;

            for y in 0..(height - 1) {
                let row = y * width;
                let next_row = row + width;
                let mut x = 0usize;
                while x + 16 <= width {
                    let current = load_u8x16_as_i16(gray.as_ptr().add(row + x));
                    let down = load_u8x16_as_i16(gray.as_ptr().add(next_row + x));
                    let difference = _mm256_sub_epi16(down, current);
                    let pair_squares = _mm256_madd_epi16(difference, difference);
                    accumulate_i32_as_i64(pair_squares, &mut vector_low, &mut vector_high);
                    x += 16;
                }

                for x in x..width {
                    let difference = i64::from(gray[next_row + x]) - i64::from(gray[row + x]);
                    scalar_sum += (difference * difference) as u64;
                }
            }

            let sum = scalar_sum
                + horizontal_sum_i64(vector_low) as u64
                + horizontal_sum_i64(vector_high) as u64;
            (sum, (height - 1) * width)
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn load_u8x16_as_i16(pointer: *const u8) -> __m256i {
        unsafe {
            let bytes = _mm_loadu_si128(pointer.cast::<__m128i>());
            _mm256_cvtepu8_epi16(bytes)
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn accumulate_i32_as_i64(
        values: __m256i,
        low_accumulator: &mut __m256i,
        high_accumulator: &mut __m256i,
    ) {
        let low = _mm256_castsi256_si128(values);
        let high = _mm256_extracti128_si256::<1>(values);
        *low_accumulator = _mm256_add_epi64(*low_accumulator, _mm256_cvtepi32_epi64(low));
        *high_accumulator = _mm256_add_epi64(*high_accumulator, _mm256_cvtepi32_epi64(high));
    }

    #[target_feature(enable = "avx2")]
    unsafe fn horizontal_sum_i64(values: __m256i) -> i64 {
        unsafe {
            let mut lanes = [0i64; 4];
            _mm256_storeu_si256(lanes.as_mut_ptr().cast::<__m256i>(), values);
            lanes.into_iter().sum()
        }
    }
}

#[cfg(all(feature = "neon-accel", target_arch = "aarch64"))]
mod aarch64 {
    use std::arch::aarch64::*;

    use burst_core::FocusMetrics;

    #[target_feature(enable = "neon")]
    pub(super) unsafe fn focus_metrics_neon(
        gray: &[u8],
        width: usize,
        height: usize,
    ) -> FocusMetrics {
        // SAFETY: The caller verifies NEON support and validates the buffer length.
        // Vector loads are bounded by their loop conditions and scalar tails handle
        // every remaining pixel.
        unsafe {
            let (lap_sum, lap_sq_sum, lap_count) = laplacian_sums(gray, width, height);
            let (dx_sum, dx_count) = horizontal_gradient_sum(gray, width, height);
            let (dy_sum, dy_count) = vertical_gradient_sum(gray, width, height);

            let sharpness = if lap_count == 0 {
                0.0
            } else {
                let count = lap_count as f64;
                let mean = lap_sum as f64 / count;
                lap_sq_sum as f64 / count - mean * mean
            };
            let tenengrad = dx_sum as f64 / (dx_count as f64).max(1.0)
                + dy_sum as f64 / (dy_count as f64).max(1.0);
            FocusMetrics {
                sharpness,
                tenengrad,
            }
        }
    }

    #[target_feature(enable = "neon")]
    unsafe fn laplacian_sums(gray: &[u8], width: usize, height: usize) -> (i64, u64, usize) {
        if width < 3 || height < 3 {
            return (0, 0, 0);
        }

        unsafe {
            let mut sum = 0i64;
            let mut square_sum = 0u64;
            for y in 1..(height - 1) {
                let row = y * width;
                let mut x = 1usize;
                while x + 16 < width {
                    let index = row + x;
                    let (center_low, center_high) = load_u8x16_as_i16(gray.as_ptr().add(index));
                    let (left_low, left_high) = load_u8x16_as_i16(gray.as_ptr().add(index - 1));
                    let (right_low, right_high) = load_u8x16_as_i16(gray.as_ptr().add(index + 1));
                    let (up_low, up_high) = load_u8x16_as_i16(gray.as_ptr().add(index - width));
                    let (down_low, down_high) = load_u8x16_as_i16(gray.as_ptr().add(index + width));
                    let lap_low = laplacian(center_low, left_low, right_low, up_low, down_low);
                    let lap_high =
                        laplacian(center_high, left_high, right_high, up_high, down_high);
                    sum += i64::from(vaddlvq_s16(lap_low)) + i64::from(vaddlvq_s16(lap_high));
                    square_sum += sum_squares_i16(lap_low) + sum_squares_i16(lap_high);
                    x += 16;
                }
                for x in x..(width - 1) {
                    let index = row + x;
                    let value = -4 * i64::from(gray[index])
                        + i64::from(gray[index - 1])
                        + i64::from(gray[index + 1])
                        + i64::from(gray[index - width])
                        + i64::from(gray[index + width]);
                    sum += value;
                    square_sum += (value * value) as u64;
                }
            }
            (sum, square_sum, (width - 2) * (height - 2))
        }
    }

    #[target_feature(enable = "neon")]
    unsafe fn horizontal_gradient_sum(gray: &[u8], width: usize, height: usize) -> (u64, usize) {
        if width < 2 {
            return (0, 0);
        }

        unsafe {
            let mut sum = 0u64;
            for y in 0..height {
                let row = y * width;
                let mut x = 0usize;
                while x + 16 < width {
                    let (current_low, current_high) = load_u8x16_as_i16(gray.as_ptr().add(row + x));
                    let (right_low, right_high) = load_u8x16_as_i16(gray.as_ptr().add(row + x + 1));
                    sum += sum_squares_i16(vsubq_s16(right_low, current_low));
                    sum += sum_squares_i16(vsubq_s16(right_high, current_high));
                    x += 16;
                }
                for x in x..(width - 1) {
                    let difference = i64::from(gray[row + x + 1]) - i64::from(gray[row + x]);
                    sum += (difference * difference) as u64;
                }
            }
            (sum, height * (width - 1))
        }
    }

    #[target_feature(enable = "neon")]
    unsafe fn vertical_gradient_sum(gray: &[u8], width: usize, height: usize) -> (u64, usize) {
        if height < 2 {
            return (0, 0);
        }

        unsafe {
            let mut sum = 0u64;
            for y in 0..(height - 1) {
                let row = y * width;
                let next_row = row + width;
                let mut x = 0usize;
                while x + 16 <= width {
                    let (current_low, current_high) = load_u8x16_as_i16(gray.as_ptr().add(row + x));
                    let (down_low, down_high) = load_u8x16_as_i16(gray.as_ptr().add(next_row + x));
                    sum += sum_squares_i16(vsubq_s16(down_low, current_low));
                    sum += sum_squares_i16(vsubq_s16(down_high, current_high));
                    x += 16;
                }
                for x in x..width {
                    let difference = i64::from(gray[next_row + x]) - i64::from(gray[row + x]);
                    sum += (difference * difference) as u64;
                }
            }
            (sum, (height - 1) * width)
        }
    }

    #[target_feature(enable = "neon")]
    unsafe fn load_u8x16_as_i16(pointer: *const u8) -> (int16x8_t, int16x8_t) {
        unsafe {
            let bytes = vld1q_u8(pointer);
            (
                vreinterpretq_s16_u16(vmovl_u8(vget_low_u8(bytes))),
                vreinterpretq_s16_u16(vmovl_high_u8(bytes)),
            )
        }
    }

    #[target_feature(enable = "neon")]
    unsafe fn laplacian(
        center: int16x8_t,
        left: int16x8_t,
        right: int16x8_t,
        up: int16x8_t,
        down: int16x8_t,
    ) -> int16x8_t {
        let horizontal = vaddq_s16(left, right);
        let vertical = vaddq_s16(up, down);
        let twice_center = vaddq_s16(center, center);
        let four_center = vaddq_s16(twice_center, twice_center);
        vsubq_s16(vaddq_s16(horizontal, vertical), four_center)
    }

    #[target_feature(enable = "neon")]
    unsafe fn sum_squares_i16(values: int16x8_t) -> u64 {
        let low = vmull_s16(vget_low_s16(values), vget_low_s16(values));
        let high = vmull_high_s16(values, values);
        vaddlvq_u32(vreinterpretq_u32_s32(low)) + vaddlvq_u32(vreinterpretq_u32_s32(high))
    }
}

#[cfg(test)]
mod tests {
    use burst_core::cpu_focus_metrics;

    use super::{backend_name, focus_metrics};

    #[test]
    fn matches_scalar_reference_for_varied_dimensions_and_data() {
        let dimensions = [
            (1, 1),
            (1, 19),
            (19, 1),
            (2, 2),
            (3, 3),
            (5, 7),
            (15, 9),
            (16, 3),
            (17, 11),
            (31, 17),
            (32, 32),
            (33, 65),
            (127, 91),
        ];

        for (width, height) in dimensions {
            for seed in [0u64, 1, 0x1234_5678, u32::MAX as u64] {
                let pixels = generated_pixels(width * height, seed);
                let expected = cpu_focus_metrics(&pixels, width, height);
                let actual = focus_metrics(&pixels, width, height);
                assert_close(
                    actual.metrics.sharpness,
                    expected.sharpness,
                    width,
                    height,
                    "sharpness",
                );
                assert_close(
                    actual.metrics.tenengrad,
                    expected.tenengrad,
                    width,
                    height,
                    "tenengrad",
                );
                assert!(actual.notes.is_empty());
            }
        }
    }

    #[test]
    fn matches_scalar_reference_for_extreme_patterns() {
        let width = 67;
        let height = 35;
        let patterns = [
            vec![0; width * height],
            vec![u8::MAX; width * height],
            (0..width * height)
                .map(|index| if index.is_multiple_of(2) { 0 } else { u8::MAX })
                .collect(),
            (0..width * height)
                .map(|index| {
                    if (index / width).is_multiple_of(2) {
                        0
                    } else {
                        u8::MAX
                    }
                })
                .collect(),
        ];

        for pixels in patterns {
            let expected = cpu_focus_metrics(&pixels, width, height);
            let actual = focus_metrics(&pixels, width, height);
            assert_close(
                actual.metrics.sharpness,
                expected.sharpness,
                width,
                height,
                "sharpness",
            );
            assert_close(
                actual.metrics.tenengrad,
                expected.tenengrad,
                width,
                height,
                "tenengrad",
            );
        }
    }

    #[test]
    fn rejects_invalid_or_empty_buffers_without_panicking() {
        let cases: &[(&[u8], usize, usize)] = &[
            (&[], 0, 0),
            (&[], 1, 1),
            (&[0], 2, 1),
            (&[0, 1], 1, 1),
            (&[], usize::MAX, 2),
        ];

        for (pixels, width, height) in cases {
            let result = focus_metrics(pixels, *width, *height);
            assert_eq!(result.metrics.sharpness, 0.0);
            assert_eq!(result.metrics.tenengrad, 0.0);
            assert_eq!(result.backend, backend_name());
            assert_eq!(result.notes.len(), 1);
        }
    }

    #[test]
    fn reports_the_runtime_selected_backend() {
        #[cfg(all(
            feature = "avx2-accel",
            any(target_arch = "x86", target_arch = "x86_64")
        ))]
        let expected = if std::arch::is_x86_feature_detected!("avx2") {
            "cpu_avx2"
        } else {
            "cpu_scalar"
        };
        #[cfg(all(feature = "neon-accel", target_arch = "aarch64"))]
        let expected = if std::arch::is_aarch64_feature_detected!("neon") {
            "cpu_neon"
        } else {
            "cpu_scalar"
        };
        #[cfg(not(any(
            all(
                feature = "avx2-accel",
                any(target_arch = "x86", target_arch = "x86_64")
            ),
            all(feature = "neon-accel", target_arch = "aarch64")
        )))]
        let expected = "cpu_scalar";

        assert_eq!(backend_name(), expected);
        assert_eq!(focus_metrics(&[0], 1, 1).backend, expected);
    }

    fn generated_pixels(count: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        (0..count)
            .map(|index| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407)
                    ^ index as u64;
                (state >> 24) as u8
            })
            .collect()
    }

    fn assert_close(actual: f64, expected: f64, width: usize, height: usize, metric: &str) {
        let tolerance = 1e-10 * expected.abs().max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance,
            "{metric} differed for {width}x{height}: actual {actual}, expected {expected}, tolerance {tolerance}"
        );
    }
}
