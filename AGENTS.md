# AGENTS.md

## Scope

This repository implements a non-destructive burst-frame deduplication app for local photo folders and mounted SD cards.

## Implementation Rules

- Keep all source-photo operations non-destructive by default. Scans may read source files and write artifacts under the selected output directory, but must not delete, rename, or move source files.
- Any source-folder mutator must be explicit and confirmed. Move operations must copy each grouped asset into the run folder by default, or into an explicit user-selected local destination, verify every copied size, then remove the original files as one recoverable transaction. Never use `/tmp` or a destination inside the source folder/card. Persist enough state to restore moved files to their original paths.
- Treat same-basename RAW/JPEG files as one asset. Decisions must apply to the grouped asset and its sidecars together.
- Keep temporal bursts and near-duplicate stacks as separate concepts. Filename/time heuristics form parent bursts; subject-aware visual comparison forms culling stacks.
- Never suggest reject solely because a frame ranks below a fixed keeper count. Automatic reject requires duplicate confidence at or above the configured threshold; uncertain matches must remain review items.
- Use EXIF original capture time with subseconds/offset when available, then fall back to filesystem timestamps.
- Keep backend functionality independent from the current web UI. The CLI scan/export path must remain usable without launching a browser.
- Keep the macOS GUI under `macos/BurstFrameDeduplicatorApp` as native SwiftUI. It must call the shared Rust backend through the public C ABI; the default Rust CLI build must not require any windowing dependency.
- Keep CPU scoring primitives in `crates/burst-core` portable to `wasm32-unknown-unknown`; native acceleration wrappers belong in the root crate.
- Keep English and Simplified Chinese locale keys synchronized in `locales/*.json`. User-facing strings belong in these external catalogs, not Rust, Swift, or JavaScript source, unless they are low-level diagnostics.
- Gate platform-specific native code with Rust features and `cfg(target_os = "...")`. macOS Metal/Vision code must compile only on supported Apple targets.
- When adding acceleration or detector backends, provide a CPU or heuristic fallback and record the selected backend plus fallback notes in `manifest.json`.
- Keep user-facing review UI simple. Show recommendations as preselected keep/reject controls and hide low-level metrics behind expandable details.
- Do not expose permanent delete controls in the web UI. Moving rejects must require a confirmation dialog and leave files recoverable in a local folder.

## Testing Rules

- Run `cargo fmt` after Rust edits.
- Run `cargo check` for compile validation.
- Run `swift build --package-path macos/BurstFrameDeduplicatorApp` and `scripts/test_macos_app.sh` after native GUI or C ABI changes. The test script supplies the standalone Command Line Tools `Testing.framework` path when full Xcode is not selected.
- Run `cargo check -p burst-wasm --target wasm32-unknown-unknown` after portable browser changes.
- Run `cargo test` even when there are no dedicated tests yet, because it builds the test profile.
- Use `git lfs pull` before benchmark work if the fixture zip is only a pointer file.
- Run a local sample scan after changing scoring, clustering, export, or UI state behavior:

  ```bash
  cargo run -- scan samples --out runs/sample --max-time-gap 5 --max-cluster-span 10
  ```

- For performance-sensitive pipeline changes, benchmark a large real corpus when available, and inspect the stage timings in `manifest.json`.
- For clustering changes, run `benchmark/run_benchmarks.py` and preserve the reviewed must-link, cannot-link, and posture-phase coverage expectations in `benchmark/accuracy_labels.json`.
- Run `web/wasm/build.sh` and browser-test both locales at desktop and mobile widths after changing the static application.
- Run `scripts/build_macos_app.sh` and inspect the packaged app with native UI automation after changing SwiftUI layout, navigation, locale loading, or packaging.
- If testing against an SD card, never run generated move scripts unless explicitly asked.

## Benchmark Expectations

- Record separate timings for discovery, decode, feature scoring, high-resolution refinement, detector scoring, thumbnail generation, clustering, manifest writing, and export.
- Report throughput as assets/sec where applicable.
- Report peak RSS where the platform supports it.
- Compare acceleration and detector selections from `manifest.json`; do not assume a requested hardware backend was actually selected.
- Run `benchmark/run_frontend_benchmarks.py` for changes that could affect CLI, Swift FFI, or WASM path overhead.
