#!/usr/bin/env python3
"""Run option benchmarks on the persisted largest-cluster fixture."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ASSET_ZIP = ROOT / "benchmark" / "assets" / "original_burst_frames.zip"
FIXTURE = ROOT / "benchmark" / "work" / "original_burst_frames"
RUNS = ROOT / "benchmark" / "runs"
RESULTS = ROOT / "benchmark" / "results"
ACCURACY_LABELS = ROOT / "benchmark" / "accuracy_labels.json"
BIN = ROOT / "target" / "release" / "burst-frame-deduplicator"
CARGO = os.environ.get("CARGO") or shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")


CASES = [
    ("cpu_heuristic", ["--acceleration", "cpu", "--detector", "heuristic"]),
    ("metal_heuristic", ["--acceleration", "metal", "--detector", "heuristic"]),
    ("metal_vision", ["--acceleration", "metal", "--detector", "vision"]),
]


def main() -> None:
    prepare_fixture()
    labels = json.loads(ACCURACY_LABELS.read_text())
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
        peak_rss_mb = run_scan(cmd)
        manifest = json.loads((out / "manifest.json").read_text())
        by_stage = {item["stage"]: item for item in manifest["benchmarks"]}
        accuracy = evaluate_accuracy(manifest, labels)
        detector_usage = {}
        for asset in manifest["assets"]:
            backend = (asset.get("detector") or {}).get("backend", "off")
            detector_usage[backend] = detector_usage.get(backend, 0) + 1
        rows.append(
            {
                "case": name,
                "acceleration": manifest["acceleration"]["selected"],
                "detector": manifest["detector"]["selected"],
                "detector_usage": ", ".join(
                    f"{backend}={count}" for backend, count in sorted(detector_usage.items())
                ),
                "assets": manifest["summary"]["discovered_assets"],
                "bursts": manifest["summary"].get("bursts", 0),
                "clusters": manifest["summary"]["clusters"],
                "keeps": manifest["summary"]["suggested_keep"],
                "rejects": manifest["summary"]["suggested_reject"],
                "reviews": manifest["summary"]["suggested_review"],
                "total_ms": by_stage["scan_total"]["elapsed_ms"],
                "assets_per_sec": by_stage["scan_total"]["items_per_sec"],
                "peak_rss_mb": peak_rss_mb,
                "scoring_ms": by_stage["scoring_total"]["elapsed_ms"],
                "feature_worker_ms": by_stage["feature_scoring_worker_sum"]["elapsed_ms"],
                "refined_assets": by_stage["refinement_total"]["items"],
                "refinement_ms": by_stage["refinement_total"]["elapsed_ms"],
                "refine_feature_worker_ms": by_stage["refinement_feature_worker_sum"][
                    "elapsed_ms"
                ],
                "detector_worker_ms": by_stage["detector_worker_sum"]["elapsed_ms"],
                **accuracy,
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
        "Accuracy labels: `benchmark/accuracy_labels.json`.",
        "",
        "| Case | Acceleration | Detector | Assets | Bursts | Stacks | Keep | Reject | Review | Pair accuracy | Phase coverage | Total ms | Assets/sec | Peak RSS MB | Refined |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for row in rows:
        lines.append(
            "| {case} | {acceleration} | {detector} | {assets} | {bursts} | {clusters} | {keeps} | {rejects} | {reviews} | {pair_accuracy:.1%} | {phase_coverage:.1%} | {total_ms:.2f} | {assets_per_sec:.2f} | {peak_rss_mb:.1f} | {refined_assets} |".format(
                **row
            )
        )
    lines.extend(
        [
            "",
            "| Case | Per-frame detector usage | Scoring ms | Refinement ms | Feature worker ms | Refine feature worker ms | Detector worker ms | Must-link | Cannot-link |",
            "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
        ]
    )
    for row in rows:
        lines.append(
            "| {case} | {detector_usage} | {scoring_ms:.2f} | {refinement_ms:.2f} | {feature_worker_ms:.2f} | {refine_feature_worker_ms:.2f} | {detector_worker_ms:.2f} | {must_link_accuracy:.1%} | {cannot_link_accuracy:.1%} |".format(
                **row
            )
        )
    lines.append("")
    lines.append("Raw run artifacts are intentionally ignored because manifests contain absolute local paths.")
    (RESULTS / "latest.md").write_text("\n".join(lines) + "\n")
    print(RESULTS / "latest.md")


def run_scan(cmd: list[str]) -> float:
    if sys.platform != "darwin" or not Path("/usr/bin/time").exists():
        subprocess.run(cmd, cwd=ROOT, check=True)
        return 0.0
    completed = subprocess.run(
        ["/usr/bin/time", "-l", *cmd],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    if completed.stdout:
        print(completed.stdout, end="")
    if completed.stderr:
        print(completed.stderr, end="", file=sys.stderr)
    completed.check_returncode()
    match = re.search(r"^\s*(\d+)\s+maximum resident set size$", completed.stderr, re.MULTILINE)
    return int(match.group(1)) / (1024 * 1024) if match else 0.0


def evaluate_accuracy(manifest: dict, labels: dict) -> dict[str, float]:
    by_name = {
        Path(asset["representative"]["rel_path"]).name: asset
        for asset in manifest["assets"]
    }

    def same_stack(pair: list[str]) -> bool:
        left, right = (by_name[name] for name in pair)
        return left["cluster_id"] == right["cluster_id"]

    must_link = labels["must_link_pairs"]
    cannot_link = labels["cannot_link_pairs"]
    must_link_accuracy = sum(same_stack(pair) for pair in must_link) / len(must_link)
    cannot_link_accuracy = sum(not same_stack(pair) for pair in cannot_link) / len(cannot_link)
    covered = 0
    for phase in labels["coverage_phases"]:
        names = [f"frame_{index:04d}.jpg" for index in range(phase["start"], phase["end"] + 1)]
        if any(by_name[name]["suggestion"]["action"] in {"keep", "review"} for name in names):
            covered += 1
    phase_coverage = covered / len(labels["coverage_phases"])
    pair_count = len(must_link) + len(cannot_link)
    pair_accuracy = (
        must_link_accuracy * len(must_link) + cannot_link_accuracy * len(cannot_link)
    ) / pair_count
    return {
        "must_link_accuracy": must_link_accuracy,
        "cannot_link_accuracy": cannot_link_accuracy,
        "pair_accuracy": pair_accuracy,
        "phase_coverage": phase_coverage,
    }


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
