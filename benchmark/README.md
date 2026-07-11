# Benchmark Fixture

This directory contains a persisted benchmark fixture derived from the largest cluster in the SD-card run.

- Fixture asset: `assets/original_burst_frames.zip`
- Extracted working directory: `work/original_burst_frames`
- Size: 120 JPEG frames, 5776x4336 pixels
- Source form: metadata-stripped full-resolution JPEG copies made from the original camera files
- Privacy check: visual contact-sheet review showed aircraft against sky only; no people, documents, screens, license plates, or private locations were visible.

The zip is tracked with Git LFS. The extracted working directory and raw benchmark run artifacts are ignored because manifests contain absolute local paths.

`accuracy_labels.json` contains visually reviewed near-duplicate pairs, distinct-pose pairs, and broad posture phases. The benchmark reports pair accuracy and verifies that every posture phase retains at least one keep/review frame alongside runtime and peak RSS.

The native matrix covers Balanced CPU, Balanced Metal, Balanced Metal + Vision, Best Quality, and Faster. Best Quality is the recommended accuracy configuration (`2048px` preview, `4096px` refinement, four refinement candidates, `0.18` duplicate distance, and `0.60` confidence); Faster is intentionally included to expose the quality cost of a smaller preview.

Run:

```bash
/usr/bin/python3 benchmark/run_benchmarks.py
```

The script unzips the fixture when needed, writes ignored raw run artifacts under `benchmark/runs/`, and writes a sanitized summary to `benchmark/results/latest.md`.

Compare the headless CLI, native Swift FFI, and static browser/WASM paths:

```bash
npm install --prefix benchmark
/usr/bin/python3 benchmark/run_frontend_benchmarks.py
```

This builds all three paths, scans the same original-resolution fixture, and writes `benchmark/results/frontend-latest.md`. The WASM harness uses local headless Chrome and records browser discovery, initialization, decode, scoring, clustering, and render timings. Browser decode defaults to four bounded jobs; compare against one job with:

```bash
node benchmark/wasm_benchmark.mjs \
  --source benchmark/work/original_burst_frames \
  --decode-concurrency 1 \
  --out benchmark/results/wasm-single-worker.json
```

On the current fixture, four jobs reduce browser decode from roughly `21.9s` to `9.8–10.9s` without changing assignments. WebCodecs use is reported by backend; headless Chrome currently selected the `image_bitmap` fallback for these JPEGs.
