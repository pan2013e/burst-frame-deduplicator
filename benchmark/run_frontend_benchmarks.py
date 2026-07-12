#!/usr/bin/env python3
"""Compare CLI, native Swift FFI, and static WASM scan paths."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path

from run_benchmarks import (
    ACCURACY_LABELS,
    FIXTURE,
    ROOT,
    RUNS,
    RESULTS,
    evaluate_accuracy,
    prepare_fixture,
)


CARGO = os.environ.get("CARGO") or shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
WASM_PACK = os.environ.get("WASM_PACK") or shutil.which("wasm-pack") or str(Path.home() / ".cargo/bin/wasm-pack")
SWIFT_PACKAGE = ROOT / "macos" / "BurstFrameDeduplicatorApp"


def main() -> None:
    prepare_fixture()
    build_paths()
    RUNS.mkdir(parents=True, exist_ok=True)
    RESULTS.mkdir(parents=True, exist_ok=True)

    cli_run = RUNS / "frontend_cli"
    ffi_run = RUNS / "frontend_swift_ffi"
    for path in (cli_run, ffi_run):
        if path.exists():
            shutil.rmtree(path)

    common = [
        "--max-time-gap", "60",
        "--max-cluster-span", "60",
        "--workers", "8",
        "--acceleration", "cpu",
        "--detector", "heuristic",
    ]
    subprocess.run(
        [str(ROOT / "target" / "release" / "burst-frame-deduplicator"), "scan", str(FIXTURE), "--out", str(cli_run), *common],
        cwd=ROOT,
        check=True,
    )
    cli_manifest = json.loads((cli_run / "manifest.json").read_text())

    swift_bin = swift_binary_directory() / "BurstFrameFFIBenchmark"
    ffi_completed = subprocess.run(
        [
            str(swift_bin),
            "--source", str(FIXTURE),
            "--out", str(ffi_run),
            "--max-time-gap-ms", "60000",
            "--max-cluster-span-ms", "60000",
            "--workers", "8",
            "--acceleration", "cpu",
            "--detector", "heuristic",
        ],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    print(ffi_completed.stderr, end="")
    ffi_result = json.loads(ffi_completed.stdout)

    wasm_results = {}
    wasm_cases = [
        ("portable", "portable", "heuristic"),
        ("webgpu", "webgpu", "heuristic"),
        ("auto", "auto", "heuristic"),
        ("webgpu_ml", "webgpu", "ml"),
    ]
    for name, acceleration, detector in wasm_cases:
        wasm_completed = subprocess.run(
            [
                "node",
                "benchmark/wasm_benchmark.mjs",
                "--source",
                str(FIXTURE),
                "--acceleration",
                acceleration,
                "--detector",
                detector,
            ],
            cwd=ROOT,
            check=True,
            text=True,
            capture_output=True,
        )
        wasm_results[name] = json.loads(wasm_completed.stdout)
    write_results(cli_manifest, ffi_result, wasm_results)


def build_paths() -> None:
    subprocess.run([CARGO, "build", "--release"], cwd=ROOT, check=True)
    dylib = ROOT / "target" / "release" / "libburst_frame_deduplicator.dylib"
    subprocess.run(["install_name_tool", "-id", "@rpath/libburst_frame_deduplicator.dylib", str(dylib)], check=True)
    subprocess.run(["swift", "build", "-c", "release", "--package-path", str(SWIFT_PACKAGE)], cwd=ROOT, check=True)
    env = os.environ.copy()
    env["WASM_PACK"] = WASM_PACK
    env["PATH"] = f"{Path(CARGO).parent}:{env.get('PATH', '')}"
    subprocess.run([str(ROOT / "web" / "wasm" / "build.sh")], cwd=ROOT, env=env, check=True)


def swift_binary_directory() -> Path:
    result = subprocess.run(
        ["swift", "build", "-c", "release", "--show-bin-path", "--package-path", str(SWIFT_PACKAGE)],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    return Path(result.stdout.strip())


def write_results(cli_manifest: dict, ffi: dict, wasm_results: dict[str, dict]) -> None:
    labels = json.loads(ACCURACY_LABELS.read_text())
    native_accuracy = evaluate_accuracy(cli_manifest, labels)
    cli_stages = {entry["stage"]: entry for entry in cli_manifest["benchmarks"]}
    ffi_stages = {entry["stage"]: entry for entry in ffi["stages"]}
    cli_total = cli_stages["scan_total"]["elapsed_ms"]
    ffi_total = ffi_stages["scan_total"]["elapsedMs"]
    overhead = (ffi["elapsedMs"] - ffi_total) / ffi_total * 100
    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    lines = [
        "# Frontend Path Benchmarks",
        "",
        f"Generated: {now}",
        "",
        "Dataset: 120 metadata-stripped original-resolution frames from `benchmark/assets/original_burst_frames.zip`.",
        "",
        "| Path | Engine/backend | Assets | Pair accuracy | Phase coverage | Total ms | Assets/sec | Relative to CLI |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |",
        f"| Headless CLI | {native_acceleration_label(cli_manifest)} + {cli_manifest['detector']['selected']} | {cli_manifest['summary']['discovered_assets']} | {native_accuracy['pair_accuracy']:.1%} | {native_accuracy['phase_coverage']:.1%} | {cli_total:.2f} | {cli_stages['scan_total']['items_per_sec']:.2f} | 1.00x |",
        f"| SwiftUI Rust FFI | {ffi_acceleration_label(ffi)} + {ffi['detector']} | {ffi['assets']} | {native_accuracy['pair_accuracy']:.1%} | {native_accuracy['phase_coverage']:.1%} | {ffi_total:.2f} | {ffi_stages['scan_total']['itemsPerSecond']:.2f} | {ffi_total / cli_total:.2f}x |",
    ]
    for mode, wasm in wasm_results.items():
        accuracy = evaluate_wasm_accuracy(wasm, labels)
        lines.append(
            f"| Static WASM ({mode}) | {wasm_acceleration_label(wasm)} | {wasm['completed_assets']} | {accuracy['pair_accuracy']:.1%} | {accuracy['phase_coverage']:.1%} | {wasm['total_ms']:.2f} | {wasm['assets_per_second']:.2f} | {wasm['total_ms'] / cli_total:.2f}x |"
        )
    lines.extend([
        "",
        f"Swift bridge call overhead around the Rust scan was {overhead:.2f}% ({ffi['elapsedMs']:.2f} ms wall time versus {ffi_total:.2f} ms recorded by the shared engine).",
        "",
        "The WASM path performs browser decode, first-pass scoring, targeted candidate refinement, and clustering. WebGPU accelerates focus metrics; descriptors and ranking remain portable WASM CPU work. The browser path does not run Rayon, Metal, platform Vision/native ONNX, or the native RAW stack, so its timing is not an accuracy-equivalent replacement for the native scan.",
        "",
        "## WASM Stages",
        "",
        "| Mode | Stage | Time ms |",
        "| --- | --- | ---: |",
    ])
    for mode, wasm in wasm_results.items():
        for stage, elapsed in wasm["stages"].items():
            if not stage.endswith("_ms"):
                continue
            label = stage.removesuffix("_ms").replace("_", " ").title()
            lines.append(f"| {mode} | {label} | {elapsed:.2f} |")
    lines.append("")
    output = RESULTS / "frontend-latest.md"
    output.write_text("\n".join(lines))
    print(output)


def native_acceleration_label(manifest: dict) -> str:
    acceleration = manifest["acceleration"]
    focus = acceleration.get("focus_backend") or acceleration["selected"]
    parallel = acceleration.get("parallelism_backend") or "rayon"
    workers = acceleration.get("parallelism_workers") or 0
    return f"{focus} + {parallel}({workers})"


def ffi_acceleration_label(result: dict) -> str:
    focus = result.get("focusBackend") or result["acceleration"]
    parallel = result.get("parallelismBackend") or "rayon"
    workers = result.get("parallelismWorkers") or 0
    return f"{focus} + {parallel}({workers})"


def wasm_acceleration_label(result: dict) -> str:
    usage = ", ".join(
        f"{backend}={count}"
        for backend, count in sorted(result.get("focus_backends", {}).items())
    ) or "unknown"
    adapter = result.get("webgpu_adapter")
    detector = ", ".join(
        f"{backend}={count}"
        for backend, count in sorted(result.get("detector_backends", {}).items())
    ) or "unknown detector"
    focus = f"{usage} ({adapter})" if adapter and "webgpu" in usage else usage
    return f"{focus} + {detector}"


def evaluate_wasm_accuracy(result: dict, labels: dict) -> dict[str, float]:
    by_name = {item["filename"]: item for item in result.get("assignments", [])}

    def same_stack(pair: list[str]) -> bool:
        return by_name[pair[0]]["stack_id"] == by_name[pair[1]]["stack_id"]

    must_link = labels["must_link_pairs"]
    cannot_link = labels["cannot_link_pairs"]
    must_link_accuracy = sum(same_stack(pair) for pair in must_link) / len(must_link)
    cannot_link_accuracy = sum(not same_stack(pair) for pair in cannot_link) / len(cannot_link)
    covered = 0
    for phase in labels["coverage_phases"]:
        names = [f"frame_{index:04d}.jpg" for index in range(phase["start"], phase["end"] + 1)]
        if any(by_name[name]["action"] in {"keep", "review"} for name in names):
            covered += 1
    pair_count = len(must_link) + len(cannot_link)
    return {
        "pair_accuracy": (
            must_link_accuracy * len(must_link)
            + cannot_link_accuracy * len(cannot_link)
        ) / pair_count,
        "phase_coverage": covered / len(labels["coverage_phases"]),
    }


if __name__ == "__main__":
    main()
