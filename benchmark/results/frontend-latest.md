# Frontend Path Benchmarks

Generated: 2026-07-12 13:30:59 UTC

Dataset: 120 metadata-stripped original-resolution frames from `benchmark/assets/original_burst_frames.zip`.

| Path | Engine/backend | Assets | Pair accuracy | Phase coverage | Total ms | Assets/sec | Relative to CLI |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Headless CLI | cpu_neon + rayon(8) + heuristic_saliency | 120 | 100.0% | 100.0% | 10038.46 | 11.95 | 1.00x |
| SwiftUI Rust FFI | cpu_neon + rayon(8) + heuristic_saliency | 120 | 100.0% | 100.0% | 9817.47 | 12.22 | 0.98x |
| Static WASM (portable) | wasm_cpu_portable=120 + heuristic_saliency=120 | 120 | 95.5% | 100.0% | 27713.87 | 4.33 | 2.76x |
| Static WASM (webgpu) | webgpu_wgpu=120 (browser WebGPU adapter) + heuristic_saliency=120 | 120 | 95.5% | 100.0% | 27693.64 | 4.33 | 2.76x |
| Static WASM (auto) | webgpu_wgpu=120 (browser WebGPU adapter) + heuristic_saliency=120 | 120 | 95.5% | 100.0% | 28545.73 | 4.20 | 2.84x |
| Static WASM (webgpu_ml) | webgpu_wgpu=120 (browser WebGPU adapter) + burn_u2netp_webgpu=120 | 120 | 95.5% | 100.0% | 75866.26 | 1.58 | 7.56x |

Swift bridge call overhead around the Rust scan was 2.06% (10019.46 ms wall time versus 9817.47 ms recorded by the shared engine).

The WASM path performs browser decode, first-pass scoring, targeted candidate refinement, and clustering. WebGPU accelerates focus metrics; descriptors and ranking remain portable WASM CPU work. The browser path does not run Rayon, Metal, platform Vision/native ONNX, or the native RAW stack, so its timing is not an accuracy-equivalent replacement for the native scan.

## WASM Stages

| Mode | Stage | Time ms |
| --- | --- | ---: |
| portable | Discovery | 13.12 |
| portable | Wasm Initialization | 9.02 |
| portable | Detector Initialization | 0.00 |
| portable | Decode | 10654.05 |
| portable | Detector Preprocessing | 0.00 |
| portable | Detector Inference | 0.00 |
| portable | Scoring | 5963.00 |
| portable | Refinement Decode | 5358.81 |
| portable | Refinement Scoring | 5563.19 |
| portable | Clustering | 21.03 |
| portable | Render | 9.52 |
| webgpu | Discovery | 8.65 |
| webgpu | Wasm Initialization | 19.35 |
| webgpu | Detector Initialization | 0.00 |
| webgpu | Decode | 10648.76 |
| webgpu | Detector Preprocessing | 0.00 |
| webgpu | Detector Inference | 0.00 |
| webgpu | Scoring | 6021.40 |
| webgpu | Refinement Decode | 5353.70 |
| webgpu | Refinement Scoring | 5522.88 |
| webgpu | Clustering | 19.46 |
| webgpu | Render | 4.12 |
| auto | Discovery | 8.42 |
| auto | Wasm Initialization | 11.28 |
| auto | Detector Initialization | 0.00 |
| auto | Decode | 11444.84 |
| auto | Detector Preprocessing | 0.00 |
| auto | Detector Inference | 0.00 |
| auto | Scoring | 6025.60 |
| auto | Refinement Decode | 5650.58 |
| auto | Refinement Scoring | 5271.05 |
| auto | Clustering | 19.52 |
| auto | Render | 4.19 |
| webgpu_ml | Discovery | 8.70 |
| webgpu_ml | Wasm Initialization | 12.22 |
| webgpu_ml | Detector Initialization | 98.38 |
| webgpu_ml | Decode | 11115.30 |
| webgpu_ml | Detector Preprocessing | 803.05 |
| webgpu_ml | Detector Inference | 45803.65 |
| webgpu_ml | Scoring | 6456.56 |
| webgpu_ml | Refinement Decode | 5684.66 |
| webgpu_ml | Refinement Scoring | 5725.06 |
| webgpu_ml | Clustering | 18.11 |
| webgpu_ml | Render | 4.17 |
