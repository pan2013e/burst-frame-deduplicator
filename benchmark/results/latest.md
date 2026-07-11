# Benchmark Results

Generated: 2026-07-11 14:31:34 UTC

Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).

Accuracy labels: `benchmark/accuracy_labels.json`.

| Case | Acceleration | Detector | Assets | Bursts | Stacks | Keep | Reject | Review | Pair accuracy | Phase coverage | Total ms | Assets/sec | Peak RSS MB | Refined |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cpu_heuristic | cpu_rayon | heuristic_saliency | 120 | 1 | 23 | 23 | 87 | 10 | 100.0% | 100.0% | 9194.48 | 13.05 | 973.1 | 44 |
| metal_heuristic | metal_focus_cpu_rest | heuristic_saliency | 120 | 1 | 23 | 23 | 87 | 10 | 100.0% | 100.0% | 9355.79 | 12.83 | 984.1 | 44 |
| metal_vision | metal_focus_cpu_rest | macos_vision_saliency | 120 | 1 | 23 | 23 | 90 | 7 | 100.0% | 100.0% | 10160.64 | 11.81 | 932.5 | 44 |

| Case | Per-frame detector usage | Scoring ms | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms | Must-link | Cannot-link |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cpu_heuristic | heuristic_saliency=120 | 5321.35 | 3337.07 | 4337.98 | 3105.92 | 0.09 | 100.0% | 100.0% |
| metal_heuristic | heuristic_saliency=120 | 5850.21 | 3077.70 | 4200.91 | 2562.82 | 0.07 | 100.0% | 100.0% |
| metal_vision | heuristic_saliency=24, macos_vision_saliency=96 | 5608.95 | 3961.31 | 4054.88 | 3870.65 | 5278.34 | 100.0% | 100.0% |

Raw run artifacts are intentionally ignored because manifests contain absolute local paths.
