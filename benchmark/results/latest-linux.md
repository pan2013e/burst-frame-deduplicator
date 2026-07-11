# Benchmark Results

Generated: 2026-07-11 22:41:04 UTC

Platform: Linux (x86_64)

Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).

Accuracy labels: `benchmark/accuracy_labels.json`.

| Case | Quality | Acceleration | Detector | Assets | Bursts | Stacks | Keep | Reject | Review | Pair accuracy | Phase coverage | Total ms | Assets/sec | Peak RSS MB | Refined |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_scalar | Balanced | cpu_scalar_rayon | heuristic_saliency | 120 | 1 | 22 | 22 | 89 | 9 | 100.0% | 100.0% | 8479.73 | 14.15 | 927.9 | 42 |
| balanced_avx2 | Balanced | cpu_avx2_rayon | heuristic_saliency | 120 | 1 | 22 | 22 | 89 | 9 | 100.0% | 100.0% | 8090.88 | 14.83 | 929.8 | 42 |
| best_quality_avx2 | Best Quality | cpu_avx2_rayon | heuristic_saliency | 120 | 1 | 23 | 23 | 89 | 8 | 100.0% | 100.0% | 44551.25 | 2.69 | 2296.5 | 83 |
| faster_avx2 | Faster | cpu_avx2_rayon | heuristic_saliency | 120 | 1 | 23 | 23 | 82 | 15 | 95.5% | 100.0% | 5749.45 | 20.87 | 744.0 | 23 |

| Case | Per-frame detector usage | Scoring ms | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms | Must-link | Cannot-link |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced_scalar | heuristic_saliency=120 | 4427.36 | 3218.60 | 6207.99 | 4594.35 | 0.11 | 100.0% | 100.0% |
| balanced_avx2 | heuristic_saliency=120 | 4267.42 | 3049.26 | 5326.56 | 4013.46 | 0.10 | 100.0% | 100.0% |
| best_quality_avx2 | heuristic_saliency=120 | 8253.39 | 35518.38 | 12945.45 | 35960.34 | 0.14 | 100.0% | 100.0% |
| faster_avx2 | heuristic_saliency=120 | 3680.64 | 1298.37 | 2556.99 | 1448.71 | 0.11 | 91.7% | 100.0% |

Raw run artifacts are intentionally ignored because manifests contain absolute local paths.
