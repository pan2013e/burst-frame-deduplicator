# Benchmark Fixture

This directory contains a persisted benchmark fixture derived from the largest cluster in the SD-card run.

- Fixture asset: `assets/original_burst_frames.zip`
- Extracted working directory: `work/original_burst_frames`
- Size: 120 JPEG frames, 5776x4336 pixels
- Source form: metadata-stripped full-resolution JPEG copies made from the original camera files
- Privacy check: visual contact-sheet review showed aircraft against sky only; no people, documents, screens, license plates, or private locations were visible.

The zip is tracked with Git LFS. The extracted working directory and raw benchmark run artifacts are ignored because manifests contain absolute local paths.

`accuracy_labels.json` contains visually reviewed near-duplicate pairs, distinct-pose pairs, and broad posture phases. The benchmark reports pair accuracy and verifies that every posture phase retains at least one keep/review frame alongside runtime and peak RSS.

The native matrix is platform-specific:

- macOS covers Balanced CPU, Balanced Metal, Balanced Metal + Vision, Best Quality with Metal + Vision, and Faster CPU.
- Linux covers Balanced portable scalar CPU, Balanced explicit AVX2, Best Quality with AVX2, and Faster with AVX2. The Linux Best Quality case retains the reviewed `0.20` duplicate-distance radius because `0.18` over-separates two reviewed must-link pairs in the current `2048px` descriptor path. It intentionally does not request CUDA so the persisted CPU results can run on machines without an idle NVIDIA GPU.

Best Quality uses a `2048px` preview, `4096px` refinement, four refinement candidates, and `0.60` confidence; its reviewed duplicate-distance radius is `0.18` in the persisted macOS matrix and `0.20` in the Linux matrix. Faster is intentionally included to expose the quality cost of a smaller preview. Both platforms build the normal release defaults before running their matrix. An explicit AVX2 request still uses the runtime-checked scalar fallback on a Linux CPU without AVX2 support, and the selected backend in the manifest remains the source of truth.

Run:

```bash
/usr/bin/python3 benchmark/run_benchmarks.py
```

The script unzips the fixture when needed and writes ignored raw run artifacts under `benchmark/runs/`. macOS writes its sanitized summary to `benchmark/results/latest.md`; Linux writes to `benchmark/results/latest-linux.md` so it does not overwrite the persisted macOS result. Each report identifies its platform. Peak RSS comes from `/usr/bin/time -l` bytes on macOS and `/usr/bin/time -v` KiB on Linux, normalized to MiB in both reports.

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
