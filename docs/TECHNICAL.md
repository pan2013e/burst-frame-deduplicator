# Technical Notes

## Architecture

- CLI and local server: Rust.
- Native GUI: optional `eframe` feature; it calls the same Rust scan pipeline through a structured progress channel.
- Review UI: static HTML/CSS/JavaScript embedded in the Rust server.
- Portable scoring core: `crates/burst-core`, compiled for both native targets and `wasm32-unknown-unknown`.
- Static browser app: DOM UI plus the `web/wasm` Rust session, built into `web/dist` by `wasm-pack`.
- Scan outputs: `manifest.json`, `review_state.json`, burst/stack/asset CSVs, thumbnails, optional move scripts.
- Asset model: same-basename RAW/JPEG files plus sidecars are reviewed as one asset.
- Grouping model: a temporal burst contains one or more subject-aware near-duplicate stacks. Quality ranking and culling happen inside stacks.

## Feature Flags

```toml
default = ["macos-native"]
macos-native = ["metal-accel", "macos-vision"]
metal-accel = ["dep:metal"]
macos-vision = ["dep:objc"]
gui = ["dep:eframe", "dep:rfd"]
```

Use a portable CPU-only build with:

```bash
cargo build --release --no-default-features
```

The default build remains headless. Compile the desktop command explicitly with `--features gui`.

## Progress Reporting

`ProgressReporter` emits serializable updates with a stable stage enum, stage item counts, optional current-file detail, stage fraction, and weighted overall fraction. The stages are preparation, discovery, preview analysis, burst/stack grouping, high-resolution refinement, ranking, manifest writing, review export, and completion.

The CLI installs a throttled terminal renderer. The native GUI receives the same events over a channel, so progress accounting remains in the backend rather than being reconstructed by each interface.

## Scoring

The scorer combines:

- whole-frame and detected-subject Laplacian/Tenengrad focus metrics
- contrast percentile spread
- exposure and clipping penalty
- saliency-derived subject confidence
- border-energy and bounding-box completeness
- normalized subject luminance, edge, and foreground-mask descriptors
- whole-frame difference hash as a fast scene-change guard only

The default pipeline has two stages:

- First pass: decode every asset to a `1280px` long-edge preview, locate a compact salient subject, build descriptors, and score provisional quality.
- Temporal grouping: combine EXIF `DateTimeOriginal` (including subseconds/offset when present), filename counters, and filesystem-time fallback into parent bursts.
- Visual grouping: apply complete-link subject comparison inside each burst. A frame must remain within the configured visual radius of every member of its stack, preventing gradual pose changes from chaining.
- Refinement pass: decode only the top candidates in each stack to a `2048px` long-edge preview, then recompute quality metrics before final ranking.
- Suggestion pass: keep the best frame in each stack; reject only when duplicate confidence clears the configured threshold. Otherwise mark it for review.

Use `--preview-size`, `--refine-size`, `--refine-candidates-per-cluster`, or `--no-refine` to tune this tradeoff.

EXIF extraction is scan-time work. New manifests include capture time plus compact per-asset metadata fields for ISO, aperture, shutter speed, focal length, and 35mm-equivalent focal length when the source file exposes them.

JPEG files use scaled-DCT decoding when supported, with `image-rs` as a compatibility fallback. Feature extraction uses an 8-bit grayscale buffer and histogram quantiles rather than cloned `f64` buffers and repeated sorts.

Metal currently accelerates focus metrics only. The kernel reduces within GPU threadgroups and returns compact partial sums. Saliency, descriptors, clustering, and ranking remain CPU-side.

## Detector Backends

| Detector | Behavior |
| --- | --- |
| `heuristic` | Uses local saliency, border contact, and object-like edge concentration. |
| `vision` | Uses macOS Vision objectness saliency for advisory completeness/quality metrics, with per-frame heuristic fallback. Stable compact saliency remains responsible for duplicate descriptors. |
| `off` | Disables detector output and keeps deterministic scoring metrics only. |

## Web RAW Preview

The review UI serves browser-displayable originals directly for JPEG/PNG/WebP/BMP/GIF assets.

For RAW-only assets, the browser first tries the local vendored `libraw-wasm` bundle under `web/vendor/libraw-wasm`. The RAW bytes are fetched from the local review server and decoded in a Web Worker. If browser-side RAW decoding fails or the bundle is unavailable, the server falls back to generating a JPEG preview from the original source file through the normal local decoder path.

Decoded RAW preview blobs are cached in the browser with a bounded LRU-style budget so reopening the same RAW-only image avoids another WASM decode while keeping memory usage capped.

This makes ordinary review light: opening a JPEG/PNG/WebP/BMP/GIF streams the existing file on demand, and opening RAW-only assets does extra work only for that image.

## Static WASM Application

`web/wasm` keeps decoded descriptors inside a `BrowserSession`. Browser JavaScript supplies a downscaled RGBA preview and file metadata; Rust computes the shared CPU quality metrics and descriptors, then performs temporal burst grouping, complete-link near-duplicate grouping, ranking, and confidence-gated suggestions.

Browser formats are decoded with `createImageBitmap`. RAW-only assets are decoded by the vendored LibRaw-WASM worker, converted to a bounded 1280px preview, and then passed to Rust. Same-basename files are grouped before analysis.

The Pages build includes a same-origin isolation service worker because the current LibRaw-WASM binary uses shared WebAssembly memory. The app remains usable for browser-decodable formats when RAW support is unavailable.

The static application cannot safely reproduce the native move operation because browser-selected files are read handles without a portable verified rename/remove API. It exports decisions as JSON and never mutates source files. Native acceleration and the second high-resolution refinement pass are also unavailable in the browser edition.

`web/wasm/build.sh` creates an ignored `web/dist` directory. `.github/workflows/pages.yml` builds that directory and deploys it with the official GitHub Pages actions.

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
- `burst_and_stack_grouping`: temporal burst and subject-aware complete-link grouping
- `ranking_and_suggestions`: within-stack quality ranking and confidence-gated decisions
- `clustering_and_ranking`: sum of the preceding grouping/ranking stages
- `manifest_write`, `review_export`, `scan_total`

Worker-sum rows are useful for understanding CPU/GPU work, but they are not wall-clock time because scoring is parallel.

## Manifest Grouping Fields

- `bursts`: temporal parent sequences in capture order.
- `clusters`: near-duplicate stacks; each has `burst_id`, similarity confidence, and maximum in-stack visual distance.
- `asset.burst_id` and `asset.cluster_id`: parent burst and culling stack assignments.
- `asset.similarity`: subject confidence, nearest visual distances, duplicate confidence, and pose novelty.

## Safety

The scanner reads source files and writes run artifacts. It does not delete source files.

The web move operation asks for confirmation, copies each rejected source file into a local run-directory folder, verifies copied size, and then removes the original file. Move reports are written under the run directory and ignored by Git.
