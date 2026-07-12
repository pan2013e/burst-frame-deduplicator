# AGENTS.md

## Scope

This repository implements a non-destructive burst-frame deduplication app for local photo folders and mounted SD cards.

## Implementation Rules

- Keep all source-photo operations non-destructive by default. Scans may read source files and write artifacts under the selected output directory, but must not delete, rename, or move source files.
- Any source-folder mutator must be explicit and confirmed. Move operations must copy each grouped asset into the run folder by default, or into an explicit user-selected local destination, verify every copied size, then remove the original files as one recoverable transaction. Never use `/tmp` or a destination inside the source folder/card. Persist enough state to restore moved files to their original paths.
- Treat same-basename RAW/JPEG files as one asset. Decisions must apply to the grouped asset and its sidecars together.
- For swapped-card RAW/JPEG counterpart operations, match by case-insensitive filename stem only; directory and mount prefixes may differ. Never guess when a stem is duplicated in the run or has multiple opposite-format matches on the selected card.
- Keep temporal bursts and near-duplicate stacks as separate concepts. Filename/time heuristics form parent bursts; subject-aware visual comparison forms culling stacks.
- Never suggest reject solely because a frame ranks below a fixed keeper count. Automatic reject requires duplicate confidence at or above the configured threshold; uncertain matches must remain review items.
- Use EXIF original capture time with subseconds/offset when available, then fall back to filesystem timestamps.
- Keep backend functionality independent from the current web UI. The CLI scan/export path must remain usable without launching a browser.
- Keep the macOS GUI under `macos/BurstFrameDeduplicatorApp` as native SwiftUI. It must call the shared Rust backend through the public C ABI; the default Rust CLI build must not require any windowing dependency.
- Keep CPU scoring primitives in `crates/burst-core` portable to `wasm32-unknown-unknown`; native acceleration wrappers belong in the root crate.
- Keep English and Simplified Chinese locale keys synchronized in `locales/*.json`. User-facing strings belong in these external catalogs, not Rust, Swift, or JavaScript source, unless they are low-level diagnostics.
- Keep release CLI binaries self-contained for locale catalogs, the local review frontend, and the browser RAW decoder. Development overrides may load external resources, but the normal fallback must be compile-time embedded.
- First-launch tutorials must use synthetic data and must not invoke scan, decision, move, or restore APIs. Keep a visible skip action on every step and a persistent Help/`?` entry that reopens the tutorial.
- Diagnostics may expose build/runtime/backend capabilities but must not include source paths, run paths, filenames, or other user-specific photo data.
- Gate a backend by its narrowest real requirement. Architecture-only SIMD belongs behind `cfg(target_arch = "...")`; OS APIs retain `cfg(target_os = "...")`. macOS Metal/Vision code must compile only on supported Apple targets.
- When adding acceleration or detector backends, provide a CPU or heuristic fallback and record the selected backend plus fallback notes in `manifest.json`.
- Keep public acceleration choices capability-oriented (`auto`, `cpu`, `gpu`, `portable`) and report focus acceleration separately from Rayon parallelism. Do not expose ISA names as normal user settings.
- Keep browser ML in the separately loaded `web/ml-wasm` crate so ordinary static scans do not fetch its runtime or weights. Verify Git LFS model integrity at build time, preserve provenance/license files, and share mask postprocessing with native detectors through `burst-core`.
- Keep user-facing review UI simple. Show recommendations as preselected keep/reject controls and hide low-level metrics behind expandable details.
- Do not expose permanent delete controls in the web UI. Moving rejects must require a confirmation dialog and leave files recoverable in a local folder.
- Treat result-folder relocation as a backend operation shared by CLI and GUI. Never overwrite an existing run; verify cross-volume copies, repair internal move-journal paths, and publish the new path only after the operation succeeds.
- Package the native GUI for Apple Silicon only. Public DMGs must use Developer ID hardened-runtime signing and notarization; ad-hoc signatures are local-test artifacts. Do not bundle optional external decoders without documenting provenance, licensing, and runtime selection.

## Versioning And Releases

- For substantive implementation work or bug fixes, excluding documentation-only changes, bump the project version according to Semantic Versioning before completion: patch for backward-compatible fixes, minor for backward-compatible functionality, and major for incompatible changes.
- Keep every version-bearing file synchronized, including the workspace and crate `Cargo.toml` files, `Cargo.lock`, and `macos/BurstFrameDeduplicatorApp/Info.plist`.
- After the required tests pass, commit and push the implementation and version bump, create an annotated `v<version>` tag, and push that tag. Never move or reuse a published version tag. If authentication, network access, or repository permissions prevent a push, report that blocker explicitly.
- Documentation-only changes do not require a version bump or release tag.

## Testing Rules

- Run `cargo fmt` after Rust edits.
- Run `cargo check` for compile validation.
- Run `swift build --package-path macos/BurstFrameDeduplicatorApp` and `scripts/test_macos_app.sh` after native GUI or C ABI changes. The test script supplies the standalone Command Line Tools `Testing.framework` path when full Xcode is not selected.
- Run `cargo clippy --features linux-gui --all-targets -- -D warnings`, build the optimized GTK binary, and run `scripts/test_linux_gui.sh` after Linux backend or native GUI changes. The smoke test requires GTK 4/libadwaita, Xvfb, Metacity, Dogtail/AT-SPI, and `xdotool`.
- Run `cargo check -p burst-wasm --target wasm32-unknown-unknown` after portable browser changes.
- Run `cargo check --manifest-path web/ml-wasm/Cargo.toml --target wasm32-unknown-unknown` after browser ML changes. Keep this crate outside the root workspace so its large inference dependency graph stays lazy.
- Run `cargo test` even when there are no dedicated tests yet, because it builds the test profile.
- Use `git lfs pull` before benchmark or browser-ML work if a fixture/model asset is only a pointer file.
- Run a local sample scan after changing scoring, clustering, export, or UI state behavior:

  ```bash
  cargo run -- scan samples --out runs/sample --max-time-gap 5 --max-cluster-span 10
  ```

- For performance-sensitive pipeline changes, benchmark a large real corpus when available, and inspect the stage timings in `manifest.json`.
- For clustering changes, run `benchmark/run_benchmarks.py` and preserve the reviewed must-link, cannot-link, and posture-phase coverage expectations in `benchmark/accuracy_labels.json`.
- Run `web/wasm/build.sh` and browser-test both locales at desktop and mobile widths after changing the static application.
- After changing embedded resources or packaging, copy the release CLI to a directory outside the checkout and verify a scan, `/api/diagnostics`, locale response, and LibRaw-WASM response with no repository files available.
- After changing tutorials or About dialogs, test first launch, skip from a non-final step, explicit reopen, both locales, and responsive browser layouts.
- Parse or lint edited GitHub Actions YAML and keep the portable and native build commands aligned with local test scripts.
- Keep usage screenshots current, free of personal paths and metadata, and at least 1920 pixels wide or an equivalent high-density native capture.
- Run `scripts/build_macos_app.sh` and inspect the packaged app with native UI automation after changing SwiftUI layout, navigation, locale loading, or packaging.
- Verify packaged macOS load commands do not retain an absolute workspace `LC_RPATH`; the app must resolve the Rust dylib from `Contents/Frameworks`.
- After DMG changes, build and mount the image, verify that the app and `/Applications` alias are present, and run `codesign --verify` on the packaged app. Never claim an ad-hoc build is Gatekeeper-ready.
- After Linux package changes, run `scripts/build_linux_app.sh`, inspect/extract the `.deb` with `dpkg-deb`, and validate its desktop file and AppStream metadata. Test ARM-specific work on native AArch64 Linux when available; a cross-compile alone does not validate NEON dispatch or the external ONNX Runtime pack.
- If testing against an SD card, never run generated move scripts unless explicitly asked.

## Benchmark Expectations

- Record separate timings for discovery, decode, feature scoring, high-resolution refinement, detector scoring, thumbnail generation, clustering, manifest writing, and export.
- Report throughput as assets/sec where applicable.
- Report peak RSS where the platform supports it.
- Compare acceleration and detector selections from `manifest.json`; do not assume a requested hardware backend was actually selected.
- Run `benchmark/run_frontend_benchmarks.py` for changes that could affect CLI, Swift FFI, or WASM path overhead.
