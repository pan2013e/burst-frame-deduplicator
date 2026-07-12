# Benchmark Results

Generated: 2026-07-12 12:09:42 UTC

Platform: macOS (arm64)

Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).

Accuracy labels: `benchmark/accuracy_labels.json`.

| Case | Quality | Acceleration | Detector | Assets | Bursts | Stacks | Keep | Reject | Review | Pair accuracy | Phase coverage | Total ms | Assets/sec | Peak RSS MB | Refined |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_portable | Balanced | cpu_portable + rayon(8) | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 9351.94 | 12.83 | 1138.1 | 42 |
| balanced_cpu | Balanced | cpu_neon + rayon(8) | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 9082.82 | 13.21 | 1170.0 | 42 |
| balanced_gpu | Balanced | metal + rayon(8) | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 9401.77 | 12.76 | 1188.0 | 42 |
| balanced_ml | Balanced | metal + rayon(8) | mixed | 120 | 1 | 22 | 22 | 93 | 5 | 100.0% | 100.0% | 12053.91 | 9.96 | 1240.4 | 42 |
| best_quality | Best Quality | metal + rayon(8) | mixed | 120 | 1 | 27 | 27 | 89 | 4 | 100.0% | 100.0% | 36522.74 | 3.29 | 2193.1 | 88 |
| faster | Faster | cpu_neon + rayon(8) | heuristic_saliency | 120 | 1 | 23 | 23 | 82 | 15 | 95.5% | 100.0% | 6418.09 | 18.70 | 938.3 | 23 |

| Case | Per-frame detector usage | Scoring ms | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms | Must-link | Cannot-link |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_portable | heuristic_saliency=120 | 5481.15 | 3360.80 | 7009.18 | 5169.16 | 0.07 | 100.0% | 100.0% |
| balanced_cpu | heuristic_saliency=120 | 5376.69 | 3238.11 | 6631.85 | 4120.81 | 0.06 | 100.0% | 100.0% |
| balanced_gpu | heuristic_saliency=120 | 5502.80 | 3452.40 | 7105.14 | 4671.62 | 0.06 | 100.0% | 100.0% |
| balanced_ml | heuristic_saliency=24, macos_vision_saliency=96 | 8169.92 | 3472.91 | 6502.12 | 4509.42 | 26703.67 | 100.0% | 100.0% |
| best_quality | heuristic_saliency=20, macos_vision_saliency=100 | 10349.32 | 25764.65 | 13932.25 | 22388.34 | 12687.88 | 100.0% | 100.0% |
| faster | heuristic_saliency=120 | 4457.49 | 1500.94 | 2873.05 | 1286.61 | 0.05 | 91.7% | 100.0% |

Raw run artifacts are intentionally ignored because manifests contain absolute local paths.
