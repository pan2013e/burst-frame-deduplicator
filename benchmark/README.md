# Benchmark Fixture

This directory contains a persisted benchmark fixture derived from the largest cluster in the SD-card run.

- Fixture asset: `assets/original_burst_frames.zip`
- Extracted working directory: `work/original_burst_frames`
- Size: 120 JPEG frames, 5776x4336 pixels
- Source form: metadata-stripped full-resolution JPEG copies made from the original camera files
- Privacy check: visual contact-sheet review showed aircraft against sky only; no people, documents, screens, license plates, or private locations were visible.

The zip is tracked with Git LFS. The extracted working directory and raw benchmark run artifacts are ignored because manifests contain absolute local paths.

Run:

```bash
/usr/bin/python3 benchmark/run_benchmarks.py
```

The script unzips the fixture when needed, writes ignored raw run artifacts under `benchmark/runs/`, and writes a sanitized summary to `benchmark/results/latest.md`.
