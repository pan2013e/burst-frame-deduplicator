# Benchmark Results

Generated: 2026-07-05 21:05:36 UTC

Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).

| Case | Acceleration | Detector | Assets | Clusters | Total ms | Assets/sec | Scoring ms | Refined | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cpu_heuristic | cpu_rayon | heuristic_saliency | 120 | 1 | 13505.04 | 8.89 | 10801.05 | 14 | 2387.66 | 17483.97 | 6329.15 | 0.29 |
| metal_heuristic | metal_focus_cpu_rest | heuristic_saliency | 120 | 1 | 12897.28 | 9.30 | 10127.58 | 14 | 2455.67 | 16909.21 | 6823.38 | 0.09 |
| metal_vision | metal_focus_cpu_rest | macos_vision_saliency | 120 | 1 | 15489.71 | 7.75 | 12637.91 | 14 | 2534.23 | 19192.54 | 6236.66 | 10509.61 |

Raw run artifacts are intentionally ignored because manifests contain absolute local paths.
