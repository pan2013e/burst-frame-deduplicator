# Benchmark Results

Generated: 2026-07-12 02:56:22 UTC

Platform: Linux (aarch64)

Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).

Accuracy labels: `benchmark/accuracy_labels.json`.

| Case | Quality | Acceleration | Detector | Assets | Bursts | Stacks | Keep | Reject | Review | Pair accuracy | Phase coverage | Total ms | Assets/sec | Peak RSS MB | Refined |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_scalar | Balanced | cpu_scalar_rayon | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 9885.18 | 12.14 | 935.2 | 42 |
| balanced_neon | Balanced | cpu_neon_rayon | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 8716.35 | 13.77 | 936.2 | 42 |
| best_quality_neon | Best Quality | cpu_neon_rayon | heuristic_saliency | 120 | 1 | 23 | 23 | 88 | 9 | 100.0% | 100.0% | 41667.46 | 2.88 | 2306.9 | 82 |
| faster_neon | Faster | cpu_neon_rayon | heuristic_saliency | 120 | 1 | 23 | 23 | 82 | 15 | 95.5% | 100.0% | 6252.53 | 19.19 | 736.3 | 23 |

| Case | Per-frame detector usage | Scoring ms | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms | Must-link | Cannot-link |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_scalar | heuristic_saliency=120 | 5533.70 | 3837.24 | 7240.91 | 5545.31 | 0.16 | 100.0% | 100.0% |
| balanced_neon | heuristic_saliency=120 | 4798.87 | 3409.51 | 5820.27 | 4765.86 | 0.12 | 100.0% | 100.0% |
| best_quality_neon | heuristic_saliency=120 | 10356.06 | 30798.66 | 16952.45 | 23849.12 | 0.12 | 100.0% | 100.0% |
| faster_neon | heuristic_saliency=120 | 4394.68 | 1352.80 | 2793.75 | 1293.30 | 0.10 | 91.7% | 100.0% |

Raw run artifacts are intentionally ignored because manifests contain absolute local paths.
