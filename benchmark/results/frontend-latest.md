# Frontend Path Benchmarks

Generated: 2026-07-12 12:12:42 UTC

Dataset: 120 metadata-stripped original-resolution frames from `benchmark/assets/original_burst_frames.zip`.

| Path | Engine/backend | Assets | Pair accuracy | Phase coverage | Total ms | Assets/sec | Relative to CLI |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Headless CLI | cpu_neon + rayon(8) + heuristic_saliency | 120 | 100.0% | 100.0% | 9297.44 | 12.91 | 1.00x |
| SwiftUI Rust FFI | cpu_neon + rayon(8) + heuristic_saliency | 120 | 100.0% | 100.0% | 8632.53 | 13.90 | 0.93x |
| Static WASM (portable) | wasm_cpu_portable=120 + heuristic_saliency=120 | 120 | 95.5% | 100.0% | 15700.68 | 7.64 | 1.69x |
| Static WASM (webgpu) | webgpu_wgpu=120 (browser WebGPU adapter) + heuristic_saliency=120 | 120 | 95.5% | 100.0% | 15874.75 | 7.56 | 1.71x |
| Static WASM (auto) | webgpu_wgpu=120 (browser WebGPU adapter) + heuristic_saliency=120 | 120 | 95.5% | 100.0% | 15581.77 | 7.70 | 1.68x |
| Static WASM (webgpu_ml) | webgpu_wgpu=120 (browser WebGPU adapter) + burn_u2netp_webgpu=120 | 120 | 95.5% | 100.0% | 60856.00 | 1.97 | 6.55x |

Swift bridge call overhead around the Rust scan was 3.65% (8947.38 ms wall time versus 8632.53 ms recorded by the shared engine).

The WASM path performs browser decode, preview scoring, and clustering. WebGPU accelerates focus metrics only; descriptors and ranking remain portable WASM CPU work. The browser path does not run native high-resolution refinement, Rayon, Metal, or platform ML, so its timing is not an accuracy-equivalent replacement for the native scan.

## WASM Stages

| Mode | Stage | Time ms |
| --- | --- | ---: |
| portable | Discovery | 13.48 |
| portable | Wasm Initialization | 8.75 |
| portable | Detector Initialization | 0.00 |
| portable | Decode | 9645.76 |
| portable | Detector Preprocessing | 0.00 |
| portable | Detector Inference | 0.00 |
| portable | Scoring | 5931.08 |
| portable | Clustering | 22.87 |
| portable | Render | 5.59 |
| webgpu | Discovery | 8.21 |
| webgpu | Wasm Initialization | 21.12 |
| webgpu | Detector Initialization | 0.00 |
| webgpu | Decode | 9856.88 |
| webgpu | Detector Preprocessing | 0.00 |
| webgpu | Detector Inference | 0.00 |
| webgpu | Scoring | 5915.58 |
| webgpu | Clustering | 20.55 |
| webgpu | Render | 4.35 |
| auto | Discovery | 9.34 |
| auto | Wasm Initialization | 11.47 |
| auto | Detector Initialization | 0.00 |
| auto | Decode | 9578.61 |
| auto | Detector Preprocessing | 0.00 |
| auto | Detector Inference | 0.00 |
| auto | Scoring | 5910.50 |
| auto | Clustering | 20.26 |
| auto | Render | 4.32 |
| webgpu_ml | Discovery | 8.04 |
| webgpu_ml | Wasm Initialization | 11.05 |
| webgpu_ml | Detector Initialization | 92.84 |
| webgpu_ml | Decode | 10238.48 |
| webgpu_ml | Detector Preprocessing | 737.37 |
| webgpu_ml | Detector Inference | 43562.90 |
| webgpu_ml | Scoring | 6085.58 |
| webgpu_ml | Clustering | 19.72 |
| webgpu_ml | Render | 8.41 |
