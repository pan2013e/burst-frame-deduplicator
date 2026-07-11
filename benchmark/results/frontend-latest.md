# Frontend Path Benchmarks

Generated: 2026-07-11 18:28:58 UTC

Dataset: 120 metadata-stripped original-resolution frames from `benchmark/assets/original_burst_frames.zip`.

| Path | Engine/backend | Assets | Pair accuracy | Phase coverage | Total ms | Assets/sec | Relative to CLI |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Headless CLI | cpu_rayon + heuristic_saliency | 120 | 100.0% | 100.0% | 12881.54 | 9.32 | 1.00x |
| SwiftUI Rust FFI | cpu_rayon + heuristic_saliency | 120 | 100.0% | 100.0% | 9365.76 | 12.81 | 0.73x |
| Static WASM | CPU/WASM portable scorer | 120 | 95.5% | 100.0% | 15772.96 | 7.61 | 1.22x |

Swift bridge call overhead around the Rust scan was 1.96% (9549.19 ms wall time versus 9365.76 ms recorded by the shared engine).

The WASM path performs browser decode, preview scoring, and clustering. It does not run native high-resolution refinement, Rayon, Metal, or Vision, so its timing is not an accuracy-equivalent replacement for the native scan.

## WASM Stages

| Stage | Time ms |
| --- | ---: |
| Discovery | 13.25 |
| Wasm Initialization | 7.14 |
| Decode | 9767.63 |
| Scoring | 5879.55 |
| Clustering | 20.91 |
| Render | 6.22 |
