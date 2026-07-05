# Technical Notes

## Architecture

- CLI and local server: Rust.
- Review UI: static HTML/CSS/JavaScript embedded in the Rust server.
- Scan outputs: `manifest.json`, `review_state.json`, CSVs, thumbnails, optional move scripts.
- Asset model: same-basename RAW/JPEG files plus sidecars are reviewed as one asset.

## Feature Flags

```toml
default = ["macos-native"]
macos-native = ["metal-accel", "macos-vision"]
metal-accel = ["dep:metal"]
macos-vision = ["dep:objc"]
```

Use a portable CPU-only build with:

```bash
cargo build --release --no-default-features
```

## Scoring

The scorer combines:

- Laplacian variance and Tenengrad gradient focus metrics
- contrast percentile spread
- exposure and clipping penalty
- saliency-derived subject confidence
- border-energy and bounding-box completeness
- difference hash for visual split decisions

The default pipeline has two stages:

- First pass: decode every asset to a `1280px` long-edge preview for clustering and provisional scoring.
- Refinement pass: decode only keeper candidates and near ties to a `2048px` long-edge preview, then recompute quality metrics before final ranking.

Use `--preview-size`, `--refine-size`, `--refine-candidates-per-cluster`, or `--no-refine` to tune this tradeoff.

EXIF extraction is scan-time work. New manifests include compact per-asset metadata fields for ISO, aperture, shutter speed, focal length, and 35mm-equivalent focal length when the source file exposes them.

Metal currently accelerates focus metrics only. Percentiles, saliency, hashing, clustering, and ranking remain CPU-side.

## Detector Backends

| Detector | Behavior |
| --- | --- |
| `heuristic` | Uses local saliency, border contact, and object-like edge concentration. |
| `vision` | Uses macOS Vision objectness saliency when compiled and available, with heuristic fallback. |
| `off` | Disables detector output and keeps deterministic scoring metrics only. |

## Web RAW Preview

The review UI serves browser-displayable originals directly for JPEG/PNG/WebP/BMP/GIF assets.

For RAW-only assets, the browser first tries the local vendored `libraw-wasm` bundle under `web/vendor/libraw-wasm`. The RAW bytes are fetched from the local review server and decoded in a Web Worker. If browser-side RAW decoding fails or the bundle is unavailable, the server falls back to generating a JPEG preview from the original source file through the normal local decoder path.

Decoded RAW preview blobs are cached in the browser with a bounded LRU-style budget so reopening the same RAW-only image avoids another WASM decode while keeping memory usage capped.

This makes ordinary review light: opening a JPEG/PNG/WebP/BMP/GIF streams the existing file on demand, and opening RAW-only assets does extra work only for that image.

## Timing Fields

`manifest.json` includes:

- `discovery`: folder walk and asset grouping wall time
- `scoring_total`: parallel scoring wall time
- `decode_worker_sum`: sum of per-worker decode time
- `feature_scoring_worker_sum`: sum of per-worker feature time
- `refinement_total`: wall time for targeted high-resolution refinement
- `refinement_decode_worker_sum`: sum of per-worker refinement decode time
- `refinement_feature_worker_sum`: sum of per-worker refinement feature time
- `detector_worker_sum`: sum of per-worker detector time
- `thumbnail_generation_worker_sum`: sum of per-worker thumbnail time
- `clustering_and_ranking`: grouping, ranking, and suggestion time
- `manifest_write`, `review_export`, `scan_total`

Worker-sum rows are useful for understanding CPU/GPU work, but they are not wall-clock time because scoring is parallel.

## Safety

The scanner reads source files and writes run artifacts. It does not delete source files.

The web move operation asks for confirmation, copies each rejected source file into a local run-directory folder, verifies copied size, and then removes the original file. Move reports are written under the run directory and ignored by Git.
