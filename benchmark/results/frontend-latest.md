# Frontend Path Benchmarks

Generated: 2026-07-11 16:10:18 UTC

Dataset: 120 metadata-stripped original-resolution frames from `benchmark/assets/original_burst_frames.zip`.

| Path | Engine/backend | Assets | Total ms | Assets/sec | Relative to CLI |
| --- | --- | ---: | ---: | ---: | ---: |
| Headless CLI | cpu_rayon + heuristic_saliency | 120 | 13434.90 | 8.93 | 1.00x |
| SwiftUI Rust FFI | cpu_rayon + heuristic_saliency | 120 | 12642.31 | 9.49 | 0.94x |
| Static WASM | CPU/WASM portable scorer | 120 | 27290.93 | 4.40 | 2.03x |

Swift bridge call overhead around the Rust scan was 1.61% (12846.39 ms wall time versus 12642.31 ms recorded by the shared engine).

The WASM path performs browser decode, preview scoring, and clustering. It does not run native high-resolution refinement, Rayon, Metal, or Vision, so its timing is not an accuracy-equivalent replacement for the native scan.

## WASM Stages

| Stage | Time ms |
| --- | ---: |
| Discovery | 15.11 |
| Wasm Initialization | 12.20 |
| Decode | 23531.64 |
| Scoring | 3542.73 |
| Clustering | 22.94 |
| Render | 6.72 |
