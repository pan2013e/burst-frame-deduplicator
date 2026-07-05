# Burst Frame Deduplicator

Cull burst photos without losing control.

Burst Frame Deduplicator scans a camera card or photo folder, groups burst frames, suggests keep/reject decisions, and opens a local review UI where the suggestions are already applied as checkboxes. It is designed for birds, aircraft, vehicles, athletes, and other fast subjects where many frames are near-duplicates.

## What It Does

- Groups RAW/JPEG bursts by filename counter, close timestamps, and visual similarity.
- Scores sharpness, exposure, contrast, object completeness, and out-of-frame risk.
- Uses a fast preview pass plus targeted higher-resolution refinement for keep candidates and close calls.
- Extracts compact EXIF context such as ISO, aperture, shutter speed, and focal length.
- Keeps RAW/JPEG pairs and sidecars together as one asset.
- Serves a local web UI for quick review, full-resolution inspection, RAW preview, zoom/pan, and confirmed reject moves.
- Writes CSV/JSON artifacts for repeatable review.
- Never deletes files automatically.

## Quick Start

Install prerequisites, then run the smoother one-command workflow:

```bash
cargo run --release -- app /Volumes/CARD/DCIM --open --acceleration metal --detector heuristic
```

This scans the folder, writes a run under `runs/`, then starts the review UI automatically.

Default scoring uses a `1280px` long-edge first pass and refines selected candidates at `2048px`. You can tune this with `--preview-size`, `--refine-size`, `--refine-candidates-per-cluster`, or disable refinement with `--no-refine`.

For separate scan/review steps:

```bash
cargo run --release -- scan /Volumes/CARD/DCIM --acceleration metal --detector heuristic
cargo run --release -- serve --run runs/run_YYYYMMDD_HHMMSS --open
```

## Prerequisites

| Requirement | macOS Apple Silicon | macOS Intel | Linux | Windows |
| --- | --- | --- | --- | --- |
| Rust/Cargo | Required | Required | Required | Required |
| Git LFS | Required for benchmark assets | Required for benchmark assets | Required for benchmark assets | Required for benchmark assets |
| ImageMagick | Recommended for RAW/HEIC fallback | Recommended for RAW/HEIC fallback | Recommended for RAW | Recommended for RAW |
| macOS `sips` | Built in | Built in | Not available | Not available |
| Xcode Command Line Tools | Required for Metal/Vision features | Required for Metal/Vision features | Not available | Not available |
| Browser | Any modern local browser | Any modern local browser | Any modern local browser | Any modern local browser |

macOS setup:

```bash
xcode-select --install
brew install imagemagick
brew install git-lfs
git lfs install
```

Rust setup:

```bash
rustup toolchain install stable
```

## Platform Support

| Feature / Backend | macOS Apple Silicon | macOS Intel | Linux CPU | Linux NVIDIA | Windows CPU |
| --- | --- | --- | --- | --- | --- |
| JPEG/PNG/TIFF/WebP decode | Supported | Supported | Supported | Supported | Supported |
| RAW decode via ImageMagick | Supported | Supported | Supported | Supported | Supported |
| RAW decode via `sips` | Supported | Supported | Not available | Not available | Not available |
| Browser RAW preview via local LibRaw-WASM | Supported | Supported | Supported | Supported | Supported |
| CPU/Rayon scoring | Supported | Supported | Supported | Supported | Supported |
| Metal sharpness/gradient scoring | Supported | Supported if Metal device exists | Not available | Not available | Not available |
| Heuristic local detector | Supported | Supported | Supported | Supported | Supported |
| macOS Vision saliency detector | Supported | Supported | Not available | Not available | Not available |
| CUDA / TensorRT | Planned | Planned | Planned | Planned | Planned |
| OpenCL / OpenVINO | Planned | Planned | Planned | Planned | Planned |

## Review Workflow

The review page uses the scanner’s suggestions up front:

- checked = keep
- unchecked = reject
- indeterminate = needs review

You can collapse clusters, inspect the “Why” details, compare EXIF chips, click thumbnails for a full-resolution view, move through a cluster with the arrow keys, zoom/pan, save review files, and move final rejects after confirmation.

The scan phase is intentionally the heavy part: decoding, scoring, EXIF extraction, detector work, refinement, thumbnail generation, clustering, and artifact export happen there. The WebUI is light by default: it reads the manifest and thumbnails, then loads full-resolution or RAW previews only when you open an image.

Moved rejects go into a local `moved_rejects/` folder under the run directory. They are not sent to `/tmp`, and they are not permanently deleted.

## Benchmark

A privacy-stripped, metadata-stripped, original-resolution benchmark fixture is tracked with Git LFS in `benchmark/assets/original_burst_frames.zip`.

```bash
git lfs pull
python3 benchmark/run_benchmarks.py
```

Latest sanitized results are in [benchmark/results/latest.md](benchmark/results/latest.md).

Detailed end-user instructions are in [docs/USAGE.md](docs/USAGE.md). More implementation details are in [docs/TECHNICAL.md](docs/TECHNICAL.md).
