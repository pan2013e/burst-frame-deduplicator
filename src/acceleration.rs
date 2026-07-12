use std::collections::BTreeMap;

#[cfg(all(target_os = "macos", feature = "metal-accel"))]
use burst_core::FocusMetrics;
use burst_core::{FocusResult, cpu_focus_metrics};

use crate::types::{AccelerationPreference, AccelerationReport, AssetRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusBackend {
    PortableCpu,
    Avx2,
    Neon,
    Metal,
    Cuda,
}

impl FocusBackend {
    pub fn id(self) -> &'static str {
        match self {
            Self::PortableCpu => "cpu_portable",
            Self::Avx2 => "cpu_avx2",
            Self::Neon => "cpu_neon",
            Self::Metal => "metal",
            Self::Cuda => "cuda",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AccelerationPlan {
    original_request: AccelerationPreference,
    requested: AccelerationPreference,
    primary: FocusBackend,
}

impl AccelerationPlan {
    pub fn resolve(requested: AccelerationPreference) -> Self {
        let canonical = requested.canonical();
        let primary = match canonical {
            AccelerationPreference::Auto => automatic_backend(),
            AccelerationPreference::Cpu => best_cpu_backend(),
            AccelerationPreference::Gpu => gpu_backend().unwrap_or_else(best_cpu_backend),
            AccelerationPreference::Portable => FocusBackend::PortableCpu,
            _ => unreachable!("legacy acceleration preferences canonicalize above"),
        };
        Self {
            original_request: requested,
            requested: canonical,
            primary,
        }
    }

    pub fn requested(self) -> AccelerationPreference {
        self.requested
    }

    pub fn primary(self) -> FocusBackend {
        self.primary
    }

    pub fn score(self, gray: &[u8], width: usize, height: usize) -> FocusResult {
        match self.primary {
            FocusBackend::PortableCpu | FocusBackend::Avx2 | FocusBackend::Neon => {
                score_cpu(self.primary, gray, width, height)
            }
            FocusBackend::Metal => score_metal_or_cpu(self.requested, gray, width, height),
            FocusBackend::Cuda => score_cuda_or_cpu(gray, width, height),
        }
    }

    pub fn report(self, worker_count: usize, assets: &[AssetRecord]) -> AccelerationReport {
        let mut capabilities = vec![
            "cpu_portable_focus_scoring".to_string(),
            format!("rayon_cpu_workers:{worker_count}"),
        ];
        let mut notes = vec![
            "Portable CPU focus scoring and Rayon asset parallelism are independent and remain available on every native platform."
                .to_string(),
        ];

        if self.original_request != self.requested {
            notes.push(format!(
                "Legacy acceleration preference {:?} was normalized to {:?}.",
                self.original_request, self.requested
            ));
        }
        append_cpu_capabilities(&mut capabilities, &mut notes);
        append_gpu_capabilities(self, &mut capabilities, &mut notes);

        let mut usage = BTreeMap::new();
        for asset in assets {
            let backend = asset
                .feature_backend
                .split("+refined_")
                .next()
                .unwrap_or_default();
            if !backend.is_empty() {
                *usage.entry(backend.to_string()).or_insert(0usize) += 1;
            }
        }
        notes.push(format!(
            "Final per-asset focus backend usage: {}.",
            if usage.is_empty() {
                "none".to_string()
            } else {
                usage
                    .iter()
                    .map(|(backend, count)| format!("{backend}={count}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));

        let cuda_count = usage.get("cuda").copied().unwrap_or(0);
        let metal_count = usage.get("metal").copied().unwrap_or(0);
        let cpu_count = usage
            .iter()
            .filter(|(backend, _)| backend.starts_with("cpu_"))
            .map(|(_, count)| *count)
            .sum::<usize>();
        let focus_backend = if cuda_count > 0 {
            if cpu_count > 0 {
                "cuda_with_cpu_fallback"
            } else {
                "cuda"
            }
        } else if metal_count > 0 {
            if cpu_count > 0 {
                "metal_with_cpu_fallback"
            } else {
                "metal"
            }
        } else if usage.contains_key("cpu_avx2") {
            "cpu_avx2"
        } else if usage.contains_key("cpu_neon") {
            "cpu_neon"
        } else if usage.contains_key("cpu_portable") || usage.contains_key("cpu_small_image") {
            "cpu_portable"
        } else {
            self.primary.id()
        };

        // Keep the pre-v0.6 combined value for consumers that have not migrated yet.
        let selected = match focus_backend {
            "cuda" => "cuda_focus_cpu_rest",
            "cuda_with_cpu_fallback" => "cuda_focus_with_cpu_fallback",
            "metal" => "metal_focus_cpu_rest",
            "metal_with_cpu_fallback" => "metal_focus_with_cpu_fallback",
            "cpu_avx2" => "cpu_avx2_rayon",
            "cpu_neon" => "cpu_neon_rayon",
            _ => "cpu_scalar_rayon",
        };

        AccelerationReport {
            requested: self.requested,
            selected: selected.to_string(),
            focus_backend: focus_backend.to_string(),
            parallelism_backend: "rayon".to_string(),
            parallelism_workers: worker_count,
            capabilities,
            notes,
        }
    }
}

fn automatic_backend() -> FocusBackend {
    #[cfg(all(target_os = "macos", feature = "metal-accel"))]
    {
        return FocusBackend::Metal;
    }

    #[allow(unreachable_code)]
    best_cpu_backend()
}

fn gpu_backend() -> Option<FocusBackend> {
    #[cfg(all(target_os = "macos", feature = "metal-accel"))]
    {
        return Some(FocusBackend::Metal);
    }
    #[cfg(all(target_os = "linux", feature = "cuda-accel"))]
    {
        return Some(FocusBackend::Cuda);
    }

    #[allow(unreachable_code)]
    None
}

pub fn best_cpu_backend() -> FocusBackend {
    #[cfg(all(feature = "cpu-simd", any(target_arch = "x86", target_arch = "x86_64")))]
    if std::arch::is_x86_feature_detected!("avx2") {
        return FocusBackend::Avx2;
    }

    #[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
    {
        return FocusBackend::Neon;
    }

    #[allow(unreachable_code)]
    FocusBackend::PortableCpu
}

fn score_cpu(backend: FocusBackend, gray: &[u8], width: usize, height: usize) -> FocusResult {
    if backend == FocusBackend::PortableCpu {
        return portable_cpu_focus_metrics(gray, width, height);
    }

    #[cfg(all(
        feature = "cpu-simd",
        any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        return crate::cpu_accel::focus_metrics(gray, width, height);
    }

    #[allow(unreachable_code)]
    portable_cpu_focus_metrics(gray, width, height)
}

fn score_metal_or_cpu(
    _requested: AccelerationPreference,
    gray: &[u8],
    width: usize,
    height: usize,
) -> FocusResult {
    #[cfg(all(target_os = "macos", feature = "metal-accel"))]
    {
        match crate::metal_accel::focus_metrics(gray, width, height) {
            Ok(metrics) => FocusResult {
                metrics: FocusMetrics {
                    sharpness: metrics.sharpness,
                    tenengrad: metrics.tenengrad,
                },
                backend: FocusBackend::Metal.id().to_string(),
                notes: Vec::new(),
            },
            Err(error) => {
                let mut fallback = score_cpu(best_cpu_backend(), gray, width, height);
                if _requested == AccelerationPreference::Gpu {
                    fallback.notes.push(format!(
                        "Metal focus scoring failed; used CPU fallback: {error}"
                    ));
                }
                fallback
            }
        }
    }

    #[cfg(not(all(target_os = "macos", feature = "metal-accel")))]
    {
        let mut fallback = score_cpu(best_cpu_backend(), gray, width, height);
        fallback
            .notes
            .push("GPU focus scoring is unavailable in this build; used CPU fallback.".to_string());
        fallback
    }
}

fn score_cuda_or_cpu(gray: &[u8], width: usize, height: usize) -> FocusResult {
    #[cfg(all(target_os = "linux", feature = "cuda-accel"))]
    {
        match crate::cuda_accel::focus_metrics(gray, width, height) {
            Ok(metrics) => {
                return FocusResult {
                    metrics,
                    backend: FocusBackend::Cuda.id().to_string(),
                    notes: Vec::new(),
                };
            }
            Err(error) => {
                let mut fallback = score_cpu(best_cpu_backend(), gray, width, height);
                fallback.notes.push(format!(
                    "CUDA focus scoring failed; used CPU fallback: {error}"
                ));
                return fallback;
            }
        }
    }

    #[cfg(not(all(target_os = "linux", feature = "cuda-accel")))]
    {
        let mut fallback = score_cpu(best_cpu_backend(), gray, width, height);
        fallback.notes.push(
            "CUDA focus scoring is unavailable in this build; used CPU fallback.".to_string(),
        );
        fallback
    }
}

fn portable_cpu_focus_metrics(gray: &[u8], width: usize, height: usize) -> FocusResult {
    FocusResult {
        metrics: cpu_focus_metrics(gray, width, height),
        backend: FocusBackend::PortableCpu.id().to_string(),
        notes: Vec::new(),
    }
}

fn append_cpu_capabilities(_capabilities: &mut Vec<String>, _notes: &mut Vec<String>) {
    #[cfg(all(feature = "cpu-simd", any(target_arch = "x86", target_arch = "x86_64")))]
    {
        _capabilities.push("cpu_avx2_focus_scoring_compiled".to_string());
        if best_cpu_backend() == FocusBackend::Avx2 {
            _capabilities.push("cpu_avx2_focus_scoring".to_string());
        } else {
            _notes.push(
                "The AVX2 scorer is compiled in, but this CPU does not advertise AVX2; portable CPU scoring will be used."
                    .to_string(),
            );
        }
    }

    #[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
    {
        _capabilities.push("cpu_neon_focus_scoring".to_string());
        _notes.push(
            "NEON is part of the supported AArch64 target baseline; no runtime ISA probe is required."
                .to_string(),
        );
    }
}

fn append_gpu_capabilities(
    plan: AccelerationPlan,
    _capabilities: &mut Vec<String>,
    notes: &mut Vec<String>,
) {
    #[cfg(all(target_os = "macos", feature = "metal-accel"))]
    {
        _capabilities.push("metal_focus_scoring_compiled".to_string());
        if crate::metal_accel::is_available() {
            _capabilities.push("metal_focus_scoring".to_string());
        } else {
            notes.push(
                "Metal focus scoring is compiled in but no usable device initialized; CPU scoring will be used."
                    .to_string(),
            );
        }
    }

    #[cfg(all(target_os = "linux", feature = "cuda-accel"))]
    {
        _capabilities.push("cuda_focus_scoring_compiled".to_string());
        if plan.primary == FocusBackend::Cuda {
            let status = crate::cuda_accel::status();
            if status.available {
                _capabilities.push("cuda_focus_scoring".to_string());
                if let Some(device_name) = status.device_name {
                    _capabilities.push(format!("cuda_device:{device_name}"));
                }
            }
            if let Some(note) = status.note {
                notes.push(note);
            }
        }
    }

    if plan.requested == AccelerationPreference::Gpu && gpu_backend().is_none() {
        notes.push(
            "GPU acceleration was requested, but this build has no supported GPU focus adapter; CPU scoring was used."
                .to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{AccelerationPlan, FocusBackend, best_cpu_backend};
    use crate::types::AccelerationPreference;

    #[test]
    fn legacy_preferences_normalize_before_resolution() {
        assert_eq!(
            AccelerationPlan::resolve(AccelerationPreference::Neon).requested(),
            AccelerationPreference::Cpu
        );
        assert_eq!(
            AccelerationPlan::resolve(AccelerationPreference::Cuda).requested(),
            AccelerationPreference::Gpu
        );
        assert_eq!(
            AccelerationPlan::resolve(AccelerationPreference::OpenCl).primary(),
            FocusBackend::PortableCpu
        );
    }

    #[test]
    fn cpu_uses_the_best_compiled_architecture_backend() {
        assert_eq!(
            AccelerationPlan::resolve(AccelerationPreference::Cpu).primary(),
            best_cpu_backend()
        );
    }

    #[test]
    fn portable_cpu_never_resolves_to_explicit_simd() {
        assert_eq!(
            AccelerationPlan::resolve(AccelerationPreference::Portable).primary(),
            FocusBackend::PortableCpu
        );
    }
}
