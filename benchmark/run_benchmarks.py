#!/usr/bin/env python3
"""Run option benchmarks on the persisted largest-cluster fixture."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import zipfile
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ASSET_ZIP = ROOT / "benchmark" / "assets" / "original_burst_frames.zip"
FIXTURE = ROOT / "benchmark" / "work" / "original_burst_frames"
RUNS = ROOT / "benchmark" / "runs"
RESULTS = ROOT / "benchmark" / "results"
BIN = ROOT / "target" / "release" / "burst-frame-deduplicator"
CARGO = os.environ.get("CARGO", "cargo")


CASES = [
    ("cpu_heuristic", ["--acceleration", "cpu", "--detector", "heuristic"]),
    ("metal_heuristic", ["--acceleration", "metal", "--detector", "heuristic"]),
    ("metal_vision", ["--acceleration", "metal", "--detector", "vision"]),
]


def main() -> None:
    prepare_fixture()
    subprocess.run(
        [CARGO, "build", "--release"],
        cwd=ROOT,
        check=True,
    )
    RUNS.mkdir(parents=True, exist_ok=True)
    RESULTS.mkdir(parents=True, exist_ok=True)

    rows = []
    for name, options in CASES:
        out = RUNS / name
        if out.exists():
            shutil.rmtree(out)
        cmd = [
            str(BIN),
            "scan",
            str(FIXTURE),
            "--out",
            str(out),
            "--max-time-gap",
            "60",
            "--max-cluster-span",
            "60",
            "--workers",
            "8",
            *options,
        ]
        subprocess.run(cmd, cwd=ROOT, check=True)
        manifest = json.loads((out / "manifest.json").read_text())
        by_stage = {item["stage"]: item for item in manifest["benchmarks"]}
        rows.append(
            {
                "case": name,
                "acceleration": manifest["acceleration"]["selected"],
                "detector": manifest["detector"]["selected"],
                "assets": manifest["summary"]["discovered_assets"],
                "clusters": manifest["summary"]["clusters"],
                "total_ms": by_stage["scan_total"]["elapsed_ms"],
                "assets_per_sec": by_stage["scan_total"]["items_per_sec"],
                "scoring_ms": by_stage["scoring_total"]["elapsed_ms"],
                "feature_worker_ms": by_stage["feature_scoring_worker_sum"]["elapsed_ms"],
                "refined_assets": by_stage["refinement_total"]["items"],
                "refinement_ms": by_stage["refinement_total"]["elapsed_ms"],
                "refine_feature_worker_ms": by_stage["refinement_feature_worker_sum"][
                    "elapsed_ms"
                ],
                "detector_worker_ms": by_stage["detector_worker_sum"]["elapsed_ms"],
            }
        )

    write_markdown(rows)


def write_markdown(rows: list[dict]) -> None:
    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    lines = [
        "# Benchmark Results",
        "",
        f"Generated: {now}",
        "",
        "Dataset: `benchmark/assets/original_burst_frames.zip` unpacked to `benchmark/work/original_burst_frames` (120 metadata-stripped original-resolution aircraft/sky JPEG frames).",
        "",
        "| Case | Acceleration | Detector | Assets | Clusters | Total ms | Assets/sec | Scoring ms | Refined | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for row in rows:
        lines.append(
            "| {case} | {acceleration} | {detector} | {assets} | {clusters} | {total_ms:.2f} | {assets_per_sec:.2f} | {scoring_ms:.2f} | {refined_assets} | {refinement_ms:.2f} | {feature_worker_ms:.2f} | {refine_feature_worker_ms:.2f} | {detector_worker_ms:.2f} |".format(
                **row
            )
        )
    lines.append("")
    lines.append("Raw run artifacts are intentionally ignored because manifests contain absolute local paths.")
    (RESULTS / "latest.md").write_text("\n".join(lines) + "\n")
    print(RESULTS / "latest.md")


def prepare_fixture() -> None:
    if not ASSET_ZIP.exists():
        raise SystemExit(
            f"Missing benchmark asset zip: {ASSET_ZIP}. If this is a fresh clone, run `git lfs pull`."
        )
    with ASSET_ZIP.open("rb") as handle:
        header = handle.read(128)
    if header.startswith(b"version https://git-lfs.github.com/spec"):
        raise SystemExit(
            f"{ASSET_ZIP} is a Git LFS pointer, not the real zip. Run `git lfs pull`."
        )
    if FIXTURE.exists() and any(FIXTURE.iterdir()):
        return
    if FIXTURE.exists():
        shutil.rmtree(FIXTURE)
    FIXTURE.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(ASSET_ZIP) as archive:
        for member in archive.infolist():
            target = (FIXTURE.parent / member.filename).resolve()
            if not str(target).startswith(str(FIXTURE.parent.resolve())):
                raise SystemExit(f"Refusing unsafe zip member: {member.filename}")
        archive.extractall(FIXTURE.parent)
    if not FIXTURE.exists():
        raise SystemExit(f"Zip did not contain expected directory: {FIXTURE.name}")


if __name__ == "__main__":
    main()
