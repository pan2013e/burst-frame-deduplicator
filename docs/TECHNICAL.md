# Technical Notes

## Architecture

- CLI, scan pipeline, local server, and source-file operations: Rust.
- Native macOS GUI: SwiftUI. `BurstFrameAppCore` calls the Rust dynamic library through the public C ABI in `include/burst_frame_deduplicator.h`.
- Native Linux GUI: GTK 4/libadwaita in `src/linux_gui`; it calls the same typed Rust application backend directly and remains behind the optional `linux-gui` feature.
- Local review UI: static HTML/CSS/JavaScript under `web/review`, embedded into the Rust server at compile time together with locale catalogs and the LibRaw-WASM worker.
- Portable scoring core: `crates/burst-core`, compiled for both native targets and `wasm32-unknown-unknown`.
- Static browser app: DOM UI plus the `web/wasm` Rust session, built into `web/dist` by `wasm-pack`.
- Scan outputs: `manifest.json`, `review_state.json`, burst/stack/asset CSVs, thumbnails, optional move scripts.
- Asset model: same-basename RAW/JPEG files plus sidecars are reviewed as one asset.
- Grouping model: a temporal burst contains one or more subject-aware near-duplicate stacks. Quality ranking and culling happen inside stacks.
- Locale resources: `locales/en.json` and `locales/zh-CN.json`; native and browser interfaces load the appropriate namespace at runtime.

## Feature Flags

```toml
default = ["macos-native", "linux-native"]
linux-native = ["avx2-accel", "neon-accel", "onnx-detector"]
avx2-accel = []
neon-accel = []
onnx-detector = ["dep:ort"]
cuda-accel = ["linux-native", "dep:cudarc", "dep:libloading"]
linux-gui = ["linux-native", "dep:adw", "dep:gdk-pixbuf", "dep:gtk"]
macos-native = ["metal-accel", "macos-vision"]
metal-accel = ["dep:metal"]
macos-vision = ["dep:objc"]
```

The Apple dependencies are declared only for macOS targets, and CUDA/ONNX dependencies only for Linux targets, so an ordinary `cargo build` does not try to link another platform's frameworks. ONNX Runtime itself is loaded dynamically only after an explicit ML detector selects an external pack. Use a portable scalar CPU build with:

```bash
cargo build --release --no-default-features
```

Build the Linux CLI with runtime-dispatched AVX2/NEON but no CUDA adapter, or with the dynamically loaded CUDA adapter, using:

```bash
cargo build --release --no-default-features --features linux-native
cargo build --release --no-default-features --features cuda-accel
```

The default Rust CLI has no windowing dependency. Build the Linux app explicitly with `cargo build --release --features linux-gui --bin burst-frame-deduplicator-gtk` or `scripts/build_linux_app.sh`. Build the Apple Silicon macOS application separately with `scripts/build_macos_app.sh`. CUDA is not available on macOS, and deprecated/limited OpenCL is not an Apple Silicon backend target.

## Native SwiftUI And FFI

The root Rust crate produces both `rlib` and `cdylib` artifacts. Its versioned C boundary accepts UTF-8 JSON requests and returns owned JSON envelopes for:

- default scan options
- synchronous scans with a progress callback
- loading a completed run
- persisting a decision
- preparing a full-image or cached RAW preview
- exporting reviewed artifacts
- executing the confirmed, verified reject move
- restoring moved asset groups to their journaled original paths
- planning, moving, and restoring basename-matched assets on a swapped RAW/JPEG card
- relocating a complete run with progress and restore-journal path repair

Every exported function catches Rust panics before crossing the ABI. Callers release returned strings with `bfd_free_string`. The Swift bridge centralizes JSON encoding, decoding, callback lifetime, and error conversion; views do not call C functions directly.

The SwiftUI app uses system folder panels, pickers, steppers, checkboxes, menus, split navigation, confirmation dialogs, and SF Symbols. Standard controls automatically adopt macOS 26 Liquid Glass, with native fallback on earlier supported releases. Scan work, relocation, and decision writes run off the main actor. The review grid is lazy, thumbnail images use a bounded `NSCache`, and full-image RAW previews use a progressive memory/disk cache.

The full-image viewer uses `NSScrollView` rather than a gesture-only SwiftUI transform. Native magnification, two-finger pan, scroll elasticity, fitted centering, keyboard navigation, off-main ImageIO downsampling, and a bounded decoded-image cache keep interaction responsive. RAW opens from an embedded ImageIO thumbnail first. A device-pixel demand check observes Fit, window resizing, toolbar zoom, and completed trackpad magnification; it requests a `4096px` render only when the embedded bitmap would be upscaled and the render can provide a meaningful resolution gain. A 350 ms demand dwell avoids work during rapid navigation and is canceled if the view returns below the threshold. The backend writes directly into an atomically published `native_previews/` cache through `sips`; Rust no longer expands and re-encodes that JPEG. The embedded bitmap remains visible until the replacement is fully decoded, then AppKit applies its bitmap, document dimensions, equivalent magnification, and normalized center in one non-animated transaction. Fit mode stays fitted and manual zoom does not flash or jump. Settings live in a separate SwiftUI `Settings` scene and group locale/appearance, result and reject destinations, quality/acceleration controls, workload estimates, and selective run storage management.

Appearance selection is applied at `NSApplication` and existing-window level, so auxiliary Settings/About scenes follow a dark-to-system transition instead of retaining a stale view override. System mode resolves the current Aqua/high-contrast appearance explicitly and observes macOS theme-change notifications, avoiding a partially repainted window while continuing to follow later OS changes. Settings tabs publish content-specific window heights capped by `NSScreen.visibleFrame`; native `Form` scrolling remains available only when the screen is shorter than the requested content.

Known run paths are persisted separately from scan manifests. The Get Started view merges that registry with discovered children of the configured and legacy result roots, validates `manifest.json`, and computes directory usage off the main actor. Cleanup only accepts non-symlink directories with a manifest and excludes the currently open run.

Changing the result root for an open run is debounced in Swift and executed by Rust. Same-filesystem relocation uses `rename`; cross-filesystem relocation copies into a destination-volume staging folder, verifies every regular-file size, moves the old run to a cleanup tombstone, then atomically publishes the staged folder. Internal moved-reject destinations are rewritten before the new run is exposed. Existing destination names receive a suffix rather than being overwritten.

Counterpart-card matching is a metadata-only discovery pass over the selected card. It normalizes filename stems case-insensitively and ignores relative directories, but requires the opposite image kind before admitting a candidate. A duplicate normalized stem in the original manifest or more than one relevant candidate is excluded from the move plan. Move records carry a `source_set` plus source-root and relative-path fields; old journals deserialize as `primary`. Counterpart restore joins the saved relative path to the currently selected card root, which permits a different mount path while retaining traversal sanitization, occupied-path checks, verified transfer, rollback, and durable journal semantics.

`scripts/build_macos_app.sh` builds Rust and Swift in release mode, rewrites the dylib install name to `@rpath`, embeds it under `Contents/Frameworks`, copies locale resources, records commit/toolchain metadata, and signs every nested code object. It defaults to ad-hoc signing for local testing. With `CODE_SIGN_IDENTITY`, it enables hardened runtime and secure timestamps.

`scripts/build_macos_dmg.sh` stages the app beside an `/Applications` alias and creates a compressed UDZO disk image. With `NOTARY_PROFILE`, it submits through `notarytool`, waits, and staples the notarization ticket. ImageMagick is not embedded: RAW on macOS uses the installed Camera RAW/ImageIO stack through `/usr/bin/sips` first, with an optional external ImageMagick fallback.

`scripts/test_macos_app.sh` builds the dylib and runs the Swift package tests. Standalone Xcode Command Line Tools installs keep `Testing.framework` outside SwiftPM's default search path, so the script adds that path conditionally; full Xcode installations use normal framework discovery.

## Native GTK Application

The Linux app is a non-unique `adw::Application`, allowing independent concurrent processes. Main and preview controllers are retained in per-thread registries until their windows close; signal handlers use weak references to avoid window/controller cycles. Scan, decision, export, move/restore, counterpart, relocation, and cache work run off the GTK main loop and publish typed events through a bounded UI poll.

The review surface flattens expanded clusters into a `gio::ListStore` consumed by `GtkListView`. Only visible rows allocate thumbnails and controls. Cluster headers are ordered expanded-first, suggested decisions initialize tri-state checkboxes, and EXIF differences/quality/details are computed from the same manifest used by the web and macOS interfaces.

The Linux image viewer decodes off the UI thread and maintains a 384 MiB process-wide LRU-style decoded cache. JPEG uses the shared scaled decoder. RAW dynamically loads LibRaw's reentrant C API and extracts its in-memory thumbnail into an atomically published JPEG or PNM cache file; the adapter validates the ABI payload and retains an ImageMagick fallback. A demand-gated 350 ms refinement uses ImageMagick at `4096px` only when display scale and zoom would upscale the embedded bitmap. Replacements preserve Fit or equivalent displayed size and viewport, while `GtkScrolledWindow`, `GestureZoom`, and `GestureDrag` provide zoom/pan. Preview windows are registered with the application so global `Ctrl+Q` remains active.

Settings and tutorial state use the XDG config directory. Tutorial completion/skipping records schema, outcome, and timestamp. The `.deb` builder installs both binaries, localized desktop/AppStream metadata, the shared icon, model-pack installer, and RAW runtime dependencies. `scripts/test_linux_gui.sh` performs a real scan then uses Xvfb, AT-SPI/Dogtail, and Metacity to load review, open preview, navigate, and quit; it caught and now guards against reentrant checkbox-state borrows.

## Progress Reporting

`ProgressReporter` emits serializable updates with a stable stage enum, stage item counts, optional current-file detail, stage fraction, and weighted overall fraction. The stages are preparation, discovery, preview analysis, burst/stack grouping, high-resolution refinement, ranking, manifest writing, review export, and completion.

The CLI installs a throttled terminal renderer. Swift receives the same serialized updates through the FFI callback, while GTK consumes the typed Rust update directly, so neither reconstructs progress accounting. Run relocation emits the same JSON field shape with a `relocating` stage, allowing native and CLI paths to reuse their progress plumbing.

## Locale Loading

Locale files contain separate `macos`, `linux`, `reviewWeb`, and `staticWeb` namespaces. Rust validates locale identifiers before reading files and the local server exposes only supported catalog names. Native loaders use embedded catalogs with controlled development overrides; the static build copies the same files into `web/dist/locales`.

The CLI first honors a valid external locale directory and otherwise serves compile-time embedded catalogs. Review HTML/CSS/JavaScript and the vendored LibRaw worker/WASM are also compiled into the binary. Release smoke tests copy only the executable into a temporary directory, run a scan/server, and verify locale, diagnostics, and RAW-WASM responses there.

Adding a user-facing key requires updating both catalogs. Locale load failures are surfaced rather than silently reading arbitrary paths.

## Tutorials And Diagnostics

All interfaces use the same four conceptual tutorial steps but native controls and browser dialogs appropriate to each surface. Tutorial visuals are synthetic and never call scan/move APIs. Both completion and skip write a schema-versioned record containing the outcome and timestamp. macOS uses `UserDefaults`; Linux uses its XDG JSON config and migrates the former Boolean flag. Both browser editions share `web/shared/tutorial-progress.mjs`, migrate former local-storage keys, and use local storage for normal persistence. The local CLI review also writes a same-host cookie because local storage is port-specific; this preserves the record when the review server moves to another port. Help/`?` always reopens the tour without clearing the record.

The local review server exposes `/api/diagnostics` with compile-time commit, Rust/Cargo versions, target/profile, runtime OS/architecture, CPU/memory, and the run manifest's actual acceleration/detector/RAW selections. Browser code appends user-agent, platform, locale, logical-CPU/memory hints, and cross-origin isolation. The static build writes `build-info.json` with Rust, Cargo, `wasm-pack`, target, app version, and commit, then adds browser diagnostics at display time. Diagnostics intentionally omit source/run paths and file names.

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

- First pass: decode every asset to a `1280px` long-edge preview, locate a compact salient subject, build descriptors, and score provisional quality. Small or uncertain subject boxes receive an adaptive second localization pass at `1024px`; accepted refinements must satisfy confidence, area, and coarse/refined overlap checks.
- Temporal grouping: combine EXIF `DateTimeOriginal` (including subseconds/offset when present), filename counters, and filesystem-time fallback into parent bursts.
- Visual grouping: apply complete-link subject comparison inside each burst. A frame must remain within the configured visual radius of every member of its stack, preventing gradual pose changes from chaining. Pair comparisons are cached per burst so complete-link admission and final nearest-neighbor reporting reuse identical descriptor results.
- Refinement pass: decode only the top candidates in each stack to a `2048px` long-edge preview, then recompute quality metrics before final ranking.
- Suggestion pass: keep the best frame in each stack; reject only when duplicate confidence clears the configured threshold. Otherwise mark it for review.

Use `--preview-size`, `--refine-size`, `--refine-candidates-per-cluster`, or `--no-refine` to tune this tradeoff.

The native **Best Quality** preset uses a `2048px` first pass, `4096px` refinement for up to four candidates per stack, `0.18` maximum duplicate distance, `0.60` minimum duplicate confidence, Metal, and Vision. Refinement concurrency is bounded by a `2 GiB` estimated working-set budget. On the persisted fixture this reduced peak RSS from the earlier uncapped `3.46 GB` run to `1.89 GB` while preserving all reviewed pair and posture labels.

EXIF extraction is scan-time work. New manifests include capture time plus compact per-asset metadata fields for ISO, aperture, shutter speed, focal length, and 35mm-equivalent focal length when the source file exposes them.

JPEG files use scaled-DCT decoding when supported, with `image-rs` as a compatibility fallback. Feature extraction uses an 8-bit grayscale buffer and histogram quantiles rather than cloned `f64` buffers and repeated sorts.

Linux exposes the CPU choice explicitly. `--acceleration cpu` runs the portable scalar reference. `--acceleration avx2` requests the runtime-checked x86_64 AVX2 Laplacian/Tenengrad kernel, `--acceleration neon` requests the AArch64 NEON kernel, and `auto` selects the available SIMD path. Both implementations remain in the native root crate and are exact-parity tested against `burst-core`; unsupported requests use the best compatible native CPU scorer, otherwise scalar, and record the selected fallback. On the 120-frame ARM64 fixture, NEON preserved every reviewed label and improved Balanced throughput from `12.14` to `13.77` assets/sec.

Metal and CUDA accelerate whole-frame focus metrics only. Both kernels reduce Laplacian and gradient sums into compact partial results; CUDA uses per-call streams, `f64` partials, a process-cached driver/CUDA 12 NVRTC module, and dynamic library loading. CUDA is selected only by explicit `--acceleration cuda` while device parity and throughput testing is pending. Missing libraries, unavailable devices, initialization failures, or per-frame failures disable CUDA for the process and fall back to the best available CPU scorer. Saliency, descriptors, subject focus, clustering, and ranking remain CPU-side.

The optional Linux ML detector uses a dynamically loaded, pinned ONNX Runtime pack. One session is created before the Rayon scoring loop and protected by a mutex; image preprocessing and mask postprocessing stay outside the session lock. `ml-light` uses U²-Net-P at 320×320, while `ml-heavy` uses IS-Net General Use at 1024×1024. Inputs follow the projects' per-image maximum normalization, with a guarded all-zero case. Raw sigmoid outputs are thresholded without output min/max normalization. Connected components produce a subject count, confidence, union box, and border-contact risk. CUDA registration uses an ordered CUDA→CPU provider path with heuristic cuDNN convolution selection and a bounded search workspace. A post-initialization CUDA error rebuilds the session on the CPU provider and retries once; a subsequent CPU error disables ML for the scan. No-GPU CPU scans and provider fallbacks are tested; CUDA inference remains unexecuted while the available devices are occupied.

ML metrics are advisory. `pipeline::score_asset` clones the heuristic metrics before invoking any native detector, and the stable portable snapshot remains the input to similarity descriptors. A missing runtime/model, checksum mismatch, bad tensor contract, provider failure, or inference failure is recorded once and falls back to heuristic saliency. Reports include only model ID, SHA-256, byte size, runtime, provider, and generic fallback notes; model-pack paths are never serialized.

The native settings workload bar is intentionally an estimate, not telemetry. It combines preview/refinement pixel area, candidates per stack, detector cost, and acceleration choice, normalized against logical CPU count and physical memory, plus Metal availability on macOS. About windows report runtime platform diagnostics and available build metadata without photo/run paths.

Platform acceleration remains isolated behind Rust feature gates and target `cfg` checks. The final manifest derives acceleration selection from per-asset backend usage and records the actual Rayon worker count, compiled/runtime capabilities, and fallback notes. Future backends must preserve that contract and must not introduce CUDA or OpenCL claims for Apple Silicon macOS.

## Detector Backends

| Detector | Behavior |
| --- | --- |
| `heuristic` | Uses the portable two-resolution local-saliency algorithm, border contact, and object-like edge concentration. This is the self-contained Linux detector and the fallback on every platform. |
| `vision` | Uses macOS Vision objectness saliency for advisory completeness/quality metrics, with per-frame heuristic fallback. Stable compact saliency remains responsible for duplicate descriptors. |
| `ml-light` | Uses the external 4.57 MB U²-Net-P ONNX model on Linux. Explicit CPU/CUDA provider selection and checksum validation precede one serialized session. |
| `ml-heavy` | Uses the external 178.65 MB IS-Net General Use ONNX model at 1024×1024 on Linux, with the same provider/fallback contract. |
| `off` | Disables detector output and keeps deterministic scoring metrics only. |

## Web RAW Preview

The review UI serves browser-displayable originals directly for JPEG/PNG/WebP/BMP/GIF assets.

For RAW-only assets, the browser first tries the local vendored `libraw-wasm` bundle under `web/vendor/libraw-wasm`. The RAW bytes are fetched from the local review server and decoded in a Web Worker. If browser-side RAW decoding fails or the bundle is unavailable, the server falls back to generating a JPEG preview from the original source file through the normal local decoder path.

Decoded RAW preview blobs are cached in the browser with a bounded LRU-style budget so reopening the same RAW-only image avoids another WASM decode while keeping memory usage capped.

This makes ordinary review light: opening a JPEG/PNG/WebP/BMP/GIF streams the existing file on demand, and opening RAW-only assets does extra work only for that image.

## Static WASM Application

`web/wasm` keeps decoded descriptors inside a `BrowserSession`. Browser JavaScript supplies a downscaled RGBA preview and file metadata; Rust computes the shared CPU quality metrics and descriptors, then performs temporal burst grouping, complete-link near-duplicate grouping, ranking, and confidence-gated suggestions.

Browser formats are decoded in deterministic batches with bounded parallelism (four jobs by default, configurable for benchmarks). The app prefers `ImageDecoder` scaled output where WebCodecs is available, then falls back to `createImageBitmap` and `OffscreenCanvas`. RAW-only assets are decoded serially by the stateful vendored LibRaw-WASM worker, converted to a bounded 1280px preview, and cached for repeat viewing. Same-basename files are grouped before analysis.

The Pages build includes a same-origin isolation service worker because the current LibRaw-WASM binary uses shared WebAssembly memory. The app remains usable for browser-decodable formats when RAW support is unavailable.

The static application performs verified copy/remove/restore only when the source was opened with a read-write File System Access directory handle and the user confirms the operation. This API is not portable: normal folder uploads and unsupported browsers remain read-only and expose review JSON plus generated POSIX/PowerShell scripts. In-browser restore state lasts for the current session, unlike the durable native `move_state.json` journal. Native acceleration, reliable scan-time EXIF fallback, Rayon, Vision, and the second high-resolution refinement pass are unavailable in the browser edition.

`web/wasm/build.sh` creates an ignored `web/dist` directory. `.github/workflows/pages.yml` builds that directory and deploys it with the official GitHub Pages actions. Its path allow-list covers only static-app, shared-browser, shared-core, locale, and workflow inputs, so documentation-only commits do not start a Pages deployment.

## Binary CI And Releases

`.github/workflows/binaries.yml` builds Linux x86_64 and ARM64 CLI/app artifacts on native Ubuntu 24.04 runners. x86_64 includes scalar, AVX2, and dynamically loaded CUDA focus paths; ARM64 validates NEON and executes the AArch64 ONNX Runtime model pack. Both run the GTK accessibility smoke test and publish checksummed CLI archives plus `.deb` packages. The Apple Silicon job builds the CLI/app/DMG on macOS 26; that SDK is required for availability-gated Liquid Glass and Metal 4 code while deployment remains macOS 14.

Pushes and pull requests that contain non-documentation changes, plus manual runs, upload short-lived Actions artifacts. The binary workflow ignores changes confined to `docs/**` and Markdown files. Tag pushes are deliberately not path-filtered: a `v*` tag runs both package jobs, downloads their artifacts into `publish-release`, and creates or updates a GitHub Release. The release job's `startsWith(github.ref, 'refs/tags/v')` condition means GitHub displays it as **skipped** on branch, pull-request, and branch-based manual runs; that is expected.

CI has no Developer ID or notarization credentials: its DMG is deliberately ad-hoc signed and must be described as such. A maintainer can produce a hardened-runtime Developer ID build by supplying `CODE_SIGN_IDENTITY` and `NOTARY_PROFILE` to `scripts/build_macos_dmg.sh` outside that workflow.

The static scanner snapshots the selected `FileList` before clearing the input, then publishes a local `burst-benchmark-complete` event containing discovery, WASM initialization, browser decode, Rust scoring, clustering, rendering, total time, and throughput. `benchmark/wasm_benchmark.mjs` consumes this event in local headless Chrome.

## Timing Fields

`manifest.json` includes:

- `discovery`: folder walk and asset grouping wall time
- `scoring_total`: parallel scoring wall time
- `decode_worker_sum`: sum of per-worker decode time
- `feature_scoring_worker_sum`: sum of per-worker feature time
- `detector_initialization`: model verification plus ONNX Runtime/session initialization wall time
- `refinement_total`: wall time for targeted high-resolution refinement
- `refinement_decode_worker_sum`: sum of per-worker refinement decode time
- `refinement_feature_worker_sum`: sum of per-worker refinement feature time
- `detector_worker_sum`: sum of per-worker detector time
- `detector_preprocessing_worker_sum`: local ML input resize/normalization time
- `detector_session_queue_wait_worker_sum`: time workers spent waiting for the serialized ONNX session
- `detector_inference_worker_sum`: time inside successful ONNX Runtime calls
- `detector_postprocessing_worker_sum`: local ML connected-component and box extraction time
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

The shared native move operation asks for confirmation and works at asset-group granularity. It preflights all group members, copies and size-verifies the complete group, removes originals, and atomically persists `move_state.json`; partial failures trigger rollback. The default target is under the run directory, while explicit custom targets must be outside both temporary storage and the source root. Restore requires the original parent directories to exist and refuses occupied paths. Move and restore reports are written under the run directory and ignored by Git.

Native apps invoke the same shared Rust move function only after platform-native destructive confirmation dialogs. Neither native nor web review interface exposes permanent deletion.

Run-folder relocation is distinct from moving source rejects and does not require the source card. Selective cache cleanup removes complete user-selected run directories only. It can therefore remove generated previews and recoverable rejects stored inside those directories, but it does not traverse source roots or external custom move destinations.
