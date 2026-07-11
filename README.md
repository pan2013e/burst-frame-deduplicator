# Burst Frame Deduplicator

Cull burst photos without losing control.

Burst Frame Deduplicator scans a camera card or local photo folder, separates temporal bursts into posture-aware similarity stacks, and recommends which frames to keep, reject, or inspect. Decisions are preselected and remain fully editable. Source files are never changed during a scan.

![Native macOS review of a sanitized aircraft burst](docs/assets/usage-native-review.jpg)

## Highlights

- Scores whole-frame and subject sharpness, exposure, contrast, completeness, and out-of-frame risk.
- Preserves changes in posture, angle, or composition instead of treating an entire burst as one duplicate group.
- Refines likely keepers and close calls at higher resolution after a fast preview pass.
- Treats matching RAW/JPEG files and sidecars as one review asset.
- Uses Metal focus scoring and macOS Vision saliency when selected and available, with recorded CPU fallbacks.
- Includes a native SwiftUI macOS scan and review app, a headless CLI, a local review server, and a static WASM edition.
- Supports English and Simplified Chinese through editable JSON locale catalogs.

## Choose An Interface

| Interface | Best for | Scan engine | Review experience |
| --- | --- | --- | --- |
| Native macOS app | Normal interactive use | Shared Rust native backend through C FFI | Native SwiftUI grid and image viewer |
| Headless CLI | Automation and large cards | Rust, Rayon, optional Metal/Vision | Artifacts only, or serve later |
| CLI `app` command | Terminal users who want immediate review | Rust, Rayon, optional Metal/Vision | Local browser UI |
| Static WASM app | GitHub Pages and installation-free use | Portable Rust scorer in-browser | Browser UI; JSON export only |

## Native macOS App

Requires macOS 14 or newer, Rust, and the Swift toolchain from Xcode Command Line Tools.

```bash
./scripts/build_macos_app.sh
open "target/macos/Burst Frame Deduplicator.app"
```

The app chooses source/output folders, shows weighted stage progress, runs the same Rust pipeline as the CLI, and opens the review grid inside the app. Review decisions are persisted immediately. RAW previews are decoded on demand and cached under the run directory.

## Command Line

Scan and immediately start the local review server:

```bash
cargo run --release -- app /Volumes/CARD/DCIM --open --acceleration metal --detector heuristic
```

Keep scan and review separate:

```bash
cargo run --release -- scan /Volumes/CARD/DCIM --acceleration metal --detector heuristic
cargo run --release -- serve --run runs/run_YYYYMMDD_HHMMSS --open
```

Default scoring uses a `1280px` long-edge preview and refines up to two candidates per stack at `2048px`. Long runs report discovery, analysis, grouping, refinement, ranking, writing, and export progress with current item counts.

## Static WASM App

```bash
cargo install wasm-pack --version 0.15.0 --locked
./web/wasm/build.sh
python3 -m http.server 4173 --directory web/dist
```

Open [http://127.0.0.1:4173](http://127.0.0.1:4173). Photos stay in the browser process. Browser formats use `createImageBitmap`; RAW-only assets use the bundled LibRaw-WASM worker. The static edition cannot perform verified source-file moves, use Metal/Vision, or run native high-resolution refinement, so it exports a JSON review instead.

The GitHub Pages workflow builds the same static directory.

## Prerequisites

| Requirement | macOS native | Linux/Windows CLI | Static WASM build |
| --- | --- | --- | --- |
| Rust/Cargo | Required | Required | Required |
| Swift 6 / Xcode Command Line Tools | Required for SwiftUI app | Not required | Not required |
| ImageMagick | Recommended for RAW/HEIC fallback | Recommended for RAW | Not used |
| Git LFS | Required for benchmark fixture | Required for benchmark fixture | Required for benchmark fixture |
| `wasm-pack` | Optional | Optional | Required |
| Modern browser | Optional local review | Optional local review | Required |

macOS setup:

```bash
xcode-select --install
brew install imagemagick git-lfs
git lfs install
rustup toolchain install stable
```

## Platform Support

| Feature / backend | macOS Apple Silicon | macOS Intel | Linux CPU | Linux NVIDIA | Windows CPU |
| --- | --- | --- | --- | --- | --- |
| Headless CLI | Supported | Supported | Supported | Supported | Supported |
| Native SwiftUI GUI | Supported (macOS 14+) | Supported (macOS 14+) | Planned | Planned | Planned |
| Static WASM scan/review | Supported | Supported | Supported | Supported | Supported |
| JPEG/PNG/TIFF/WebP decode | Supported | Supported | Supported | Supported | Supported |
| RAW via ImageMagick | Supported | Supported | Supported | Supported | Supported |
| RAW via macOS `sips` | Supported | Supported | Not available | Not available | Not available |
| Browser RAW via LibRaw-WASM | Supported | Supported | Supported | Supported | Supported |
| CPU/Rayon scoring | Supported | Supported | Supported | Supported | Supported |
| Metal focus scoring | Supported | Supported when a Metal device exists | Not available | Not available | Not available |
| macOS Vision detector | Supported | Supported | Not available | Not available | Not available |
| CUDA / TensorRT | Planned | Planned | Planned | Planned | Planned |
| OpenCL / OpenVINO | Planned | Planned | Planned | Planned | Planned |
| English / Simplified Chinese | Supported | Supported | Supported | Supported | Supported |

Requested and selected backends, capabilities, and fallback notes are recorded in every `manifest.json`.

## Locale Configuration

User-facing strings live in [`locales/en.json`](locales/en.json) and [`locales/zh-CN.json`](locales/zh-CN.json), outside Rust and Swift source. The app bundle and static build copy these files as resources. For development or custom wording, point native components at another synchronized locale directory:

```bash
BURST_DEDUP_LOCALES_DIR=/path/to/locales ./target/release/burst-frame-deduplicator serve --run runs/example
```

## Safety

Scanning is read-only for source photos. A reject move is a separate confirmed action: it copies files to `moved_rejects/` inside the run directory, verifies copied sizes, and only then removes originals. The app exposes no permanent-delete control.

## Benchmarks

The Git LFS fixture contains 120 metadata-stripped original-resolution aircraft/sky frames. It includes reviewed must-link, cannot-link, and posture-coverage labels.

```bash
git lfs pull
python3 benchmark/run_benchmarks.py
npm install --prefix benchmark
python3 benchmark/run_frontend_benchmarks.py
```

See [accuracy/backend results](benchmark/results/latest.md) and [CLI/SwiftUI/WASM path results](benchmark/results/frontend-latest.md).

Detailed workflows are in [docs/USAGE.md](docs/USAGE.md). Architecture, FFI, acceleration, and timing details are in [docs/TECHNICAL.md](docs/TECHNICAL.md).
