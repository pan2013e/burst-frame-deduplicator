# Burst Frame Deduplicator

Cull burst photos without losing control.

Burst Frame Deduplicator scans a camera card or local photo folder, separates temporal bursts into posture-aware similarity stacks, and recommends which frames to keep, reject, or inspect. Decisions are preselected and remain fully editable. Source files are never changed during a scan.

![Native macOS review of a sanitized aircraft burst](docs/assets/usage-native-review.jpg)

## Highlights

- Scores whole-frame and subject sharpness, exposure, contrast, completeness, and out-of-frame risk.
- Preserves changes in posture, angle, or composition instead of treating an entire burst as one duplicate group.
- Refines likely keepers and close calls at higher resolution after a fast preview pass.
- Treats matching RAW/JPEG files and sidecars as one review asset.
- Reapplies reviewed rejects to a second RAW/JPEG card by filename stem, even when folder and mount paths differ, with ambiguity checks and restore support.
- Exposes stable `auto`, `cpu`, `gpu`, and `portable` processing policies while selecting Metal, CUDA, AVX2, or ARM NEON only where that capability actually exists.
- Exposes one optional ML detector: GPU-backed Vision on macOS, offline U²-Net-P or IS-Net through ONNX Runtime on Linux, and lazy U²-Net-P through WebGPU in the static browser app.
- Includes native SwiftUI macOS and GTK/libadwaita Linux scan/review apps, a headless CLI, a local review server, and a static WASM edition.
- Opens large completed runs with visible staged progress and cancels active scans safely from every interface, including `Ctrl+C` in the CLI.
- Supports English and Simplified Chinese through editable JSON locale catalogs.
- Opens with a skippable interactive tour, remembers completion on every interface, and exposes build/runtime diagnostics without reading a photo folder.

## Choose An Interface

| Interface | Best for | Scan engine | Review experience |
| --- | --- | --- | --- |
| Native macOS app | Normal interactive use | Shared Rust native backend through C FFI | Native SwiftUI grid, settings, and responsive image viewer |
| Native Linux app | GNOME and GTK desktops | Shared Rust backend in-process | Virtualized GTK review list, settings, and zoomable image viewer |
| Headless CLI | Automation and large cards | Rust, Rayon, native CPU SIMD, optional GPU/ML | Artifacts only, or serve later |
| CLI `app` command | Terminal users who want immediate review | Same native Rust engine | Local browser UI |
| Static WASM app | GitHub Pages and installation-free use | Rust/WASM with optional WebGPU focus and ML | Browser UI; JSON/script export and conditional local moves |

## Native macOS App

The native app targets Apple Silicon and requires macOS 14 or newer. It uses current SwiftUI controls, with macOS 26 Liquid Glass styling supplied by the system.

```bash
./scripts/build_macos_app.sh
open "target/macos/Burst Frame Deduplicator.app"
```

The Get Started view presents equal-size new/open actions, uses the full width for recent runs, and keeps result storage in Settings. Scans and large-run loading show staged progress in the same window; manifest loading reports byte-level progress through a buffered parser. Cancel waits for in-flight frame work, stops later stages, and removes a newly created partial run. `Command-N` launches another app process so multiple scans can run concurrently; collision-resistant run names keep their outputs separate.

RAW preview opens from the camera's embedded ImageIO preview first. The viewer requests a reusable `4096px` JPEG through the system `sips` tool only when the embedded image cannot cover the current Retina viewport or zoom level, and skips rendering when the possible resolution gain is marginal. The decoder writes directly into the run cache without a second Rust decode/re-encode pass. ImageMagick is not bundled and is only an optional compatibility fallback for formats the installed macOS release cannot decode.

Build a drag-to-Applications disk image for local testing:

```bash
./scripts/build_macos_dmg.sh
```

The default build is ad-hoc signed. Public distribution requires a Developer ID Application identity and notarization; see [Distribution](docs/USAGE.md#distributing-the-macos-app).

Tagged releases and ordinary CI runs also produce an Apple Silicon DMG. The CI app is intentionally ad-hoc signed, so Gatekeeper cannot verify its developer or notarization status. Follow the trusted-artifact steps in the usage guide, or build and sign it locally.

## Native Linux App

The Linux app uses GTK 4 and libadwaita, follows the GNOME interaction model, and shares the same Rust scan, review, move/restore, counterpart-card, locale, and run-storage backend as the CLI. Its review list is virtualized so large cards do not create every row widget at once.

```bash
sudo apt-get install libgtk-4-dev libadwaita-1-dev libgdk-pixbuf-2.0-dev \
  libraw23t64 imagemagick
cargo run --release --features linux-gui --bin burst-frame-deduplicator-gtk
```

![Native Linux review of the sanitized aircraft burst](docs/assets/usage-linux-review.png)

Ubuntu 24.04 users can build a desktop-integrated Debian package with `scripts/build_linux_app.sh`. It contains the CLI and GUI, installs the app icon/launcher, and declares LibRaw plus ImageMagick so embedded and refined RAW previews work. The same app runs on other GTK/libadwaita desktops when compatible libraries are available.

## Command Line

Scan and immediately start the local review server:

```bash
# Linux: use the best compatible CPU scorer (AVX2 or NEON when available)
cargo run --release -- app /path/to/photos --open --acceleration cpu --detector heuristic

# macOS Apple Silicon
cargo run --release -- app /Volumes/CARD/DCIM --open --acceleration gpu --detector ml
```

Keep scan and review separate:

```bash
cargo run --release -- scan /path/to/photos --acceleration auto --detector heuristic
cargo run --release -- serve --run runs/run_YYYYMMDD_HHMMSS --open
```

Apply reviewed decisions after swapping from a JPEG card to its RAW card, or vice versa:

```bash
cargo run --release -- counterpart-plan --run /path/to/run --card /Volumes/SECOND_CARD/DCIM
cargo run --release -- counterpart-apply --run /path/to/run --card /Volumes/SECOND_CARD/DCIM --confirm
cargo run --release -- counterpart-restore --run /path/to/run --card /Volumes/SECOND_CARD/DCIM --confirm
```

Matching uses only the case-insensitive filename stem: `CARD_A/DCIM/YYY.jpg` can match `CARD_B/PRIVATE/YYY.rw2`. Duplicate stems are reported and never guessed.

Acceleration choices describe intent rather than an implementation name. `auto` uses Metal on macOS and the best compatible CPU scorer elsewhere; `cpu` uses AVX2 on supported x86 processors, the AArch64 NEON baseline on ARM64, or the portable fallback; `portable` always uses the scalar reference. `gpu` explicitly requests Metal on macOS or CUDA in a CUDA-enabled Linux build:

```bash
cargo run --release --features cuda-accel -- scan /path/to/photos --acceleration gpu
```

The CUDA feature loads the NVIDIA driver and CUDA 12 NVRTC dynamically. A CUDA-enabled binary still runs on CPU-only Linux when CUDA is not requested, and an unavailable or failed CUDA scorer falls back to the best available CPU path.

Linux can also use one of two explicit local ML subject detectors. Models and ONNX Runtime live in a separately installed, checksum-verified pack; scans never download them:

```bash
pack="$HOME/.local/share/burst-frame-deduplicator/ml-model-pack"
scripts/install_linux_ml_models.sh --dest "$pack" --runtime cpu --models both

cargo run --release -- scan /path/to/photos \
  --detector ml \
  --detector-model fast \
  --detector-device cpu \
  --detector-model-pack "$pack"
```

`--detector-model fast` selects the 4.57 MB U²-Net-P model; `accurate` selects the higher-detail 178.65 MB IS-Net General Use model. `--detector auto` stays heuristic, and `--detector-device auto` stays on CPU even when a CUDA runtime is installed. ML inference device selection, focus acceleration, and Rayon parallelism are independent. See [Linux local ML setup, provenance, and CUDA requirements](docs/LINUX_ML_MODELS.md).

Default scoring uses a `1280px` long-edge preview and refines up to two candidates per stack at `2048px`. Long runs report discovery, analysis, grouping, refinement, ranking, writing, and export progress with current item counts. Press `Ctrl+C` once to request cooperative cancellation; the CLI finishes the active frame safely and removes a newly created partial run directory.

Release CLI archives are standalone: the local review HTML/CSS/JavaScript, English and Chinese catalogs, and LibRaw-WASM worker are compiled into the executable. `scan`, `export`, and `serve` therefore work outside the repository. ImageMagick remains an optional system dependency for RAW formats that a platform's native/image-rs decoders cannot handle.

## Static WASM App

```bash
cargo install wasm-pack --version 0.15.0 --locked
git lfs pull
./web/wasm/build.sh
python3 -m http.server 4173 --directory web/dist
```

Open [http://127.0.0.1:4173](http://127.0.0.1:4173). Photos stay in the browser process. Settings expose quality presets, first-pass and refinement resolution, candidates and minimum keepers per stack, focus acceleration, detector, decode concurrency, and temporal/visual grouping thresholds. The decoder runs bounded parallel jobs, prefers scaled WebCodecs when the browser exposes it, and falls back to `createImageBitmap`; RAW-only assets use the bundled LibRaw-WASM worker.

WebGPU can accelerate both first-pass and targeted refinement focus metrics through Rust `wgpu`. Selecting browser ML lazily loads a separate Burn/WGPU U²-Net-P module and model, batches up to four previews per inference, and falls back to heuristic saliency if WebGPU or the model cannot initialize. The static edition can move and restore grouped files only when the folder was opened through a browser that provides read-write File System Access handles. Other browsers keep the workflow read-only and provide review JSON plus macOS/Linux and Windows scripts. Browser refinement follows the native candidate policy, but decoding, metadata, threading, and subject-model capabilities still differ from native execution.

The GitHub Pages workflow builds the same static directory.

## Binary Builds

The **Build distributable binaries** GitHub Actions workflow tests and packages:

| Artifact | Runner | Contents |
| --- | --- | --- |
| Linux CLI | Ubuntu 24.04 x86_64 | Standalone AVX2/CUDA-capable executable, model installer/guide, notices, and checksum |
| Linux CLI | Ubuntu 24.04 ARM64 | Standalone NEON-capable executable, model installer/guide, notices, and checksum |
| Linux app | Ubuntu 24.04 x86_64 / ARM64 | Desktop-integrated `.deb` containing GTK app and CLI, with checksum |
| macOS CLI | macOS 26 Apple Silicon | Standalone executable, notices, archive checksum |
| macOS app | macOS 26 Apple Silicon | Ad-hoc signed drag-to-Applications DMG and checksum |

Pushes to `main` and pull requests that include non-documentation changes, plus manual runs, produce temporary workflow artifacts. Documentation-only changes under `docs/` or in Markdown files skip the binary workflow. A pushed `v*` tag always builds and publishes the same files and checksums on GitHub Releases. See [installation and Gatekeeper guidance](docs/USAGE.md#installing-prebuilt-binaries).

<details>
<summary>Why "Publish GitHub Release assets" is skipped</summary>

The publish job is intentionally guarded by `startsWith(github.ref, 'refs/tags/v')`. It is therefore skipped on branch pushes, pull requests, and manual runs launched from a branch, even when the Linux and macOS package jobs succeed. Create and push a Semantic Versioning tag to publish a release:

```bash
VERSION=0.7.0 # choose the next unused Semantic Versioning release
git tag -a "v${VERSION}" -m "v${VERSION}"
git push origin main "v${VERSION}"
```

Do not reuse an existing release tag.

</details>

## Prerequisites

<details>
<summary>Build prerequisites and setup commands</summary>

| Requirement | macOS native | Linux/Windows CLI | Static WASM build |
| --- | --- | --- | --- |
| Rust/Cargo | Required | Required | Required |
| Swift 6 / Apple Command Line Tools | Required | Not required | Not required |
| ImageMagick | Optional compatibility fallback | Required by Linux GUI package for refined RAW previews; otherwise recommended | Not used |
| NVIDIA driver + NVRTC | Not used | Optional for `--acceleration gpu` on Linux | Not used |
| ONNX Runtime model pack | Not used | Optional for `--detector ml` on Linux | Not used |
| Git LFS | Benchmark fixture | Benchmark fixture | Benchmark fixture and U²-Net-P browser weights |
| `wasm-pack` | Optional | Optional | Required |
| Modern browser | Optional local review | Optional local review | Required |

macOS setup:

```bash
xcode-select --install
brew install git-lfs
git lfs install
rustup toolchain install stable
```

Install ImageMagick only when a required format is not handled by the system Camera RAW stack:

```bash
brew install imagemagick
```

Ubuntu 24.04 native app setup:

```bash
sudo apt-get update
sudo apt-get install libgtk-4-dev libadwaita-1-dev libgdk-pixbuf-2.0-dev \
  libraw23t64 imagemagick git-lfs
git lfs install
git lfs pull
rustup toolchain install stable
```

</details>

## Platform Support

Legend: ✅ supported · 🟡 partial or browser-dependent · 🧭 planned · — unavailable/not applicable

| Feature / backend | macOS Apple Silicon | Linux CPU | Linux NVIDIA | Windows CPU |
| --- | :---: | :---: | :---: | :---: |
| Headless CLI | ✅ | ✅ | ✅ | ✅ |
| Native GUI | ✅ SwiftUI | ✅ GTK/libadwaita | ✅ GTK/libadwaita | 🧭 |
| macOS 26 Liquid Glass controls | ✅ | — | — | — |
| Static WASM scan/review | ✅ | ✅ | ✅ | ✅ |
| JPEG/PNG/TIFF/WebP decode | ✅ | ✅ | ✅ | ✅ |
| RAW via Apple Camera RAW / `sips` | ✅ | — | — | — |
| RAW embedded preview via LibRaw | — | ✅ native app | ✅ native app | — |
| RAW via ImageMagick fallback | 🟡 optional | ✅ | ✅ | ✅ |
| Browser RAW via LibRaw-WASM | ✅ | ✅ | ✅ | ✅ |
| Confirmed move + restore | ✅ | ✅ native + CLI | ✅ native + CLI | 🟡 browser |
| Swapped-card RAW/JPEG counterpart move | ✅ native + CLI | ✅ native + CLI | ✅ native + CLI | ✅ CLI |
| Portable scalar + Rayon scoring | ✅ | ✅ | ✅ | ✅ |
| Runtime-dispatched AVX2 focus scoring | — | ✅ x86_64 | ✅ x86_64 | — |
| AArch64-baseline NEON focus scoring | ✅ | ✅ ARM64 | ✅ ARM64 | — |
| Metal focus scoring | ✅ | — | — | — |
| Heuristic subject detector | ✅ | ✅ | ✅ | ✅ |
| Unified native ML detector | ✅ Vision/Metal | ✅ ONNX/CPU | ✅ ONNX/CUDA→CPU | — |
| Browser U²-Net-P detector | 🟡 WebGPU | 🟡 WebGPU | 🟡 WebGPU | 🟡 WebGPU |
| Browser WebGPU focus scoring | ✅ | ✅ | ✅ | ✅ |
| CUDA focus scoring | — | — | ✅ opt-in | — |
| TensorRT learned detector | — | — | 🧭 | 🧭 |
| OpenCL on Apple Silicon | — deprecated/limited | — | — | — |
| OpenVINO | — | 🧭 | 🧭 | 🧭 |
| English / Simplified Chinese | ✅ | ✅ | ✅ | ✅ |
| Graceful scan cancellation | ✅ | ✅ | ✅ | ✅ browser |
| CI release binary | ✅ CLI + app | ✅ CLI + app | ✅ CUDA-capable CLI + app | 🧭 |

Requested and selected backends, capabilities, and fallback notes are recorded in every `manifest.json`.

## Locale Configuration

User-facing strings live in [`locales/en.json`](locales/en.json) and [`locales/zh-CN.json`](locales/zh-CN.json), outside Rust and Swift source. The app bundle and static build copy these files as resources. For development or custom wording, point native components at another synchronized locale directory:

```bash
BURST_DEDUP_LOCALES_DIR=/path/to/locales ./target/release/burst-frame-deduplicator serve --run runs/example
```

## Safety

Scanning is read-only for source photos. A reject move is a separate confirmed action: it copies every file in a grouped asset, verifies copied sizes, and only then removes originals. The default destination is `moved_rejects/` inside the run directory; the user may choose another non-temporary local folder outside the source card. A durable move journal enables restore. The app exposes no permanent-delete control.

The counterpart-card operation is separately planned and confirmed. It scans only names and file metadata on the currently mounted second card, moves safe opposite-format matches under `moved_counterparts/` by default, and records relative card paths so restore still works when the card later mounts under a different root.

## Benchmarks

The Git LFS fixture contains 120 metadata-stripped original-resolution aircraft/sky frames. It includes reviewed must-link, cannot-link, and posture-coverage labels.

```bash
git lfs pull
python3 benchmark/run_benchmarks.py
npm install --prefix benchmark
python3 benchmark/run_frontend_benchmarks.py
```

See [macOS accuracy/backend results](benchmark/results/latest.md), [Linux x86_64 portable/native CPU results](benchmark/results/latest-linux.md), [Linux ARM64 portable/native CPU results](benchmark/results/latest-linux-arm64.md), and [CLI/SwiftUI/WASM path results](benchmark/results/frontend-latest.md).

Detailed workflows are in [docs/USAGE.md](docs/USAGE.md). Architecture, FFI, acceleration, and timing details are in [docs/TECHNICAL.md](docs/TECHNICAL.md).
