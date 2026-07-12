# Frontend Path Benchmarks

Generated: 2026-07-12 04:15:25 UTC

Dataset: 120 metadata-stripped original-resolution frames from `benchmark/assets/original_burst_frames.zip`.

| Path | Engine/backend | Assets | Pair accuracy | Phase coverage | Total ms | Assets/sec | Relative to CLI |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Headless CLI | cpu_scalar_rayon + heuristic_saliency | 120 | 100.0% | 100.0% | 9396.16 | 12.77 | 1.00x |
| SwiftUI Rust FFI | cpu_scalar_rayon + heuristic_saliency | 120 | 100.0% | 100.0% | 8271.74 | 14.51 | 0.88x |
| Static WASM | CPU/WASM portable scorer | 120 | 95.5% | 100.0% | 15986.21 | 7.51 | 1.70x |

Swift bridge call overhead around the Rust scan was 2.25% (8458.12 ms wall time versus 8271.74 ms recorded by the shared engine).

The WASM path performs browser decode, preview scoring, and clustering. It does not run native high-resolution refinement, Rayon, Metal, or Vision, so its timing is not an accuracy-equivalent replacement for the native scan.

## WASM Stages

| Stage | Time ms |
| --- | ---: |
| Discovery | 13.54 |
| Wasm Initialization | 7.78 |
| Decode | 9951.17 |
| Scoring | 5905.18 |
| Clustering | 22.90 |
| Render | 5.80 |
