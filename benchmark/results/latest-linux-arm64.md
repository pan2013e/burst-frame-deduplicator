# Benchmark Results

Generated: 2026-07-12 12:07:37 UTC

Platform: Linux (aarch64)

Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).

Accuracy labels: `benchmark/accuracy_labels.json`.

| Case | Quality | Acceleration | Detector | Assets | Bursts | Stacks | Keep | Reject | Review | Pair accuracy | Phase coverage | Total ms | Assets/sec | Peak RSS MB | Refined |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_portable | Balanced | cpu_portable + rayon(8) | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 9559.14 | 12.55 | 930.2 | 42 |
| balanced_cpu | Balanced | cpu_neon + rayon(8) | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 8800.21 | 13.64 | 942.8 | 42 |
| best_quality_cpu | Best Quality | cpu_neon + rayon(8) | heuristic_saliency | 120 | 1 | 23 | 23 | 88 | 9 | 100.0% | 100.0% | 41949.09 | 2.86 | 2199.2 | 82 |
| faster_cpu | Faster | cpu_neon + rayon(8) | heuristic_saliency | 120 | 1 | 23 | 23 | 82 | 15 | 95.5% | 100.0% | 6627.71 | 18.11 | 739.5 | 23 |

| Case | Per-frame detector usage | Scoring ms | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms | Must-link | Cannot-link |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_portable | heuristic_saliency=120 | 5351.07 | 3687.54 | 6543.49 | 5302.66 | 0.14 | 100.0% | 100.0% |
| balanced_cpu | heuristic_saliency=120 | 4919.38 | 3370.97 | 5642.05 | 4262.54 | 0.10 | 100.0% | 100.0% |
| best_quality_cpu | heuristic_saliency=120 | 9838.79 | 31593.89 | 16077.08 | 23962.52 | 0.13 | 100.0% | 100.0% |
| faster_cpu | heuristic_saliency=120 | 4671.34 | 1447.18 | 2892.20 | 1353.19 | 0.11 | 91.7% | 100.0% |

Raw run artifacts are intentionally ignored because manifests contain absolute local paths.
