# Frontend Path Benchmarks

Generated: 2026-07-12 00:16:31 UTC

Dataset: 120 metadata-stripped original-resolution frames from `benchmark/assets/original_burst_frames.zip`.

| Path | Engine/backend | Assets | Pair accuracy | Phase coverage | Total ms | Assets/sec | Relative to CLI |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Headless CLI | cpu_scalar_rayon + heuristic_saliency | 120 | 100.0% | 100.0% | 10568.07 | 11.35 | 1.00x |
| SwiftUI Rust FFI | cpu_scalar_rayon + heuristic_saliency | 120 | 100.0% | 100.0% | 9280.16 | 12.93 | 0.88x |
| Static WASM | CPU/WASM portable scorer | 120 | 95.5% | 100.0% | 15910.11 | 7.54 | 1.51x |

Swift bridge call overhead around the Rust scan was 2.15% (9479.99 ms wall time versus 9280.16 ms recorded by the shared engine).

The WASM path performs browser decode, preview scoring, and clustering. It does not run native high-resolution refinement, Rayon, Metal, or Vision, so its timing is not an accuracy-equivalent replacement for the native scan.

## WASM Stages

| Stage | Time ms |
| --- | ---: |
| Discovery | 12.71 |
| Wasm Initialization | 8.13 |
| Decode | 9891.45 |
| Scoring | 5864.21 |
| Clustering | 21.10 |
| Render | 5.98 |
