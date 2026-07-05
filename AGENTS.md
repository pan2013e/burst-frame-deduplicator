# AGENTS.md

## Scope

This repository implements a non-destructive burst-frame deduplication app for local photo folders and mounted SD cards.

## Implementation Rules

- Keep all source-photo operations non-destructive by default. Scans may read source files and write artifacts under the selected output directory, but must not delete, rename, or move source files.
- Any source-folder mutator must be explicit and confirmed. The web move operation must copy final rejects into a local run-directory folder, verify the copied size, then remove the original source file. Generated helper scripts must move rejects into a local run-directory folder, not `/tmp` or the source card.
- Treat same-basename RAW/JPEG files as one asset. Decisions must apply to the grouped asset and its sidecars together.
- Keep backend functionality independent from the current web UI. The CLI scan/export path must remain usable without launching a browser.
- Gate platform-specific native code with Rust features and `cfg(target_os = "...")`. macOS Metal/Vision code must compile only on supported Apple targets.
- When adding acceleration or detector backends, provide a CPU or heuristic fallback and record the selected backend plus fallback notes in `manifest.json`.
- Keep user-facing review UI simple. Show recommendations as preselected keep/reject controls and hide low-level metrics behind expandable details.
- Do not expose permanent delete controls in the web UI. Moving rejects must require a confirmation dialog and leave files recoverable in a local folder.

## Testing Rules

- Run `cargo fmt` after Rust edits.
- Run `cargo check` for compile validation.
- Run `cargo test` even when there are no dedicated tests yet, because it builds the test profile.
- Use `git lfs pull` before benchmark work if the fixture zip is only a pointer file.
- Run a local sample scan after changing scoring, clustering, export, or UI state behavior:

  ```bash
  cargo run -- scan samples --out runs/sample --max-time-gap 5 --max-cluster-span 10
  ```

- For performance-sensitive pipeline changes, benchmark a large real corpus when available, and inspect the stage timings in `manifest.json`.
- If testing against an SD card, never run generated move scripts unless explicitly asked.

## Benchmark Expectations

- Record separate timings for discovery, decode, feature scoring, high-resolution refinement, detector scoring, thumbnail generation, clustering, manifest writing, and export.
- Report throughput as assets/sec where applicable.
- Compare acceleration and detector selections from `manifest.json`; do not assume a requested hardware backend was actually selected.
