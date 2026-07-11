# Benchmark Results

Generated: 2026-07-11 18:27:48 UTC

Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).

Accuracy labels: `benchmark/accuracy_labels.json`.

| Case | Quality | Acceleration | Detector | Assets | Bursts | Stacks | Keep | Reject | Review | Pair accuracy | Phase coverage | Total ms | Assets/sec | Peak RSS MB | Refined |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_cpu | Balanced | cpu_rayon | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 9748.59 | 12.31 | 1130.1 | 42 |
| balanced_metal | Balanced | metal_focus_cpu_rest | heuristic_saliency | 120 | 1 | 22 | 22 | 90 | 8 | 100.0% | 100.0% | 9015.91 | 13.31 | 1151.6 | 42 |
| balanced_vision | Balanced | metal_focus_cpu_rest | macos_vision_saliency | 120 | 1 | 22 | 22 | 93 | 5 | 100.0% | 100.0% | 9783.36 | 12.27 | 1155.8 | 42 |
| best_quality | Best Quality | metal_focus_cpu_rest | macos_vision_saliency | 120 | 1 | 27 | 27 | 89 | 4 | 100.0% | 100.0% | 36484.72 | 3.29 | 1890.0 | 88 |
| faster | Faster | cpu_rayon | heuristic_saliency | 120 | 1 | 23 | 23 | 82 | 15 | 95.5% | 100.0% | 6711.82 | 17.88 | 938.7 | 23 |

| Case | Per-frame detector usage | Scoring ms | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms | Must-link | Cannot-link |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_cpu | heuristic_saliency=120 | 5691.81 | 3532.32 | 6638.79 | 4796.38 | 0.16 | 100.0% | 100.0% |
| balanced_metal | heuristic_saliency=120 | 5452.54 | 3151.99 | 6526.85 | 4143.41 | 0.07 | 100.0% | 100.0% |
| balanced_vision | heuristic_saliency=24, macos_vision_saliency=96 | 5987.65 | 3390.20 | 6950.06 | 4363.84 | 4925.45 | 100.0% | 100.0% |
| best_quality | heuristic_saliency=21, macos_vision_saliency=99 | 9786.54 | 26293.79 | 14365.19 | 23353.77 | 6748.91 | 100.0% | 100.0% |
| faster | heuristic_saliency=120 | 4713.02 | 1539.50 | 3207.92 | 1484.02 | 0.14 | 91.7% | 100.0% |

Raw run artifacts are intentionally ignored because manifests contain absolute local paths.
