#!/usr/bin/env python3
"""Run isolated, reproducible Mission 0 scenarios and retain raw frame telemetry."""
import argparse
import datetime
import json
import hashlib
from pathlib import Path
import platform
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/tack-app"))
    parser.add_argument("--manifest", type=Path, default=Path("benchmark-data/mission0/manifest.json"))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--seconds", type=float, default=12)
    parser.add_argument("--headless", action="store_true")
    args = parser.parse_args()
    root = args.output or Path("benchmark-results") / datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    root.mkdir(parents=True, exist_ok=False)
    corpus = json.loads(args.manifest.read_text(encoding="utf-8"))
    environment = {"platform": platform.platform(), "processor": platform.processor(),
                   "headless": args.headless, "corpus_objects": len(corpus["objects"]),
                   "corpus_seed": corpus["seed"], "corpus_sources": corpus["source_count"],
                   "pillow_version": corpus["pillow_version"], "limitations": corpus["limitation"]}
    source_root = Path(__file__).resolve().parent.parent
    source_paths = [source_root / "Cargo.toml", source_root / "Cargo.lock", source_root / "rust-toolchain.toml"]
    source_paths += [p for p in (source_root / "crates").rglob("*") if p.suffix in [".rs", ".wgsl", ".toml"]]
    source_paths += list((source_root / "tools").glob("*.py"))
    digest = hashlib.sha256()
    for path in sorted(source_paths):
        digest.update(str(path.relative_to(source_root)).encode() + b"\0" + path.read_bytes() + b"\0")
    environment["source_sha256"] = digest.hexdigest()
    environment["binary_sha256"] = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    for tool, command in {"cpu": ["lscpu"], "memory": ["free", "-b"], "gpu_pci": ["lspci", "-nn"],
                          "rustc": ["rustc", "--version"], "cargo": ["cargo", "--version"],
                          "nvidia": ["nvidia-smi", "--query-gpu=name,driver_version,memory.total", "--format=csv"]}.items():
        try:
            environment[tool] = subprocess.run(command, capture_output=True, text=True, check=True).stdout
        except (OSError, subprocess.CalledProcessError):
            environment[tool] = None
    (root / "environment.json").write_text(json.dumps(environment, indent=2), encoding="utf-8")
    summaries = []
    for scenario in ["cold", "warm", "pan", "zoom", "pressure"]:
        # Cold and warm deliberately share only SSD data, never decoded RAM/VRAM.
        cache = root / ("cold-warm-cache" if scenario in ["cold", "warm"] else scenario + "-cache")
        command = [str(args.binary.resolve()), "--manifest", str(args.manifest.resolve()),
                   "--cache", str(cache.resolve()), "--scenario", scenario, "--seconds", str(args.seconds),
                   "--output", str((root / (scenario + ".json")).resolve())]
        if args.headless:
            command.append("--headless")
        result = subprocess.run(command, capture_output=True, text=True)
        (root / (scenario + ".log")).write_text(result.stderr, encoding="utf-8")
        if result.returncode:
            parser.exit(result.returncode, f"{scenario} failed: {result.stderr}\n")
        report = json.loads((root / (scenario + ".json")).read_text(encoding="utf-8"))
        frames = report.pop("frames")
        report.pop("gpu_pass_samples", None)
        for frame in frames:
            if frame["cpu_cache_bytes"] > (16 if scenario == "pressure" else 64) * 1024**2:
                raise RuntimeError("CPU cache budget violated")
            if frame["gpu_resident_bytes"] > (24 if scenario == "pressure" else 128) * 1024**2:
                raise RuntimeError("GPU cache budget violated")
            if frame["pending"] > 2 or frame["in_flight"] > 3 or frame["upload_bytes"] > 16 * 1024**2 or frame["uploads"] > 2:
                raise RuntimeError("queue or upload budget violated")
        visible = sum(f["visible"] for f in frames)
        report["content_coverage_fraction"] = (sum(f["visible"] - f["placeholders"] for f in frames) / visible) if visible else None
        report["gpu_completion_fps"] = report["completed_submissions_at_report"] / args.seconds
        report["display_cache_bytes_after_run"] = sum(p.stat().st_size for p in cache.rglob("*") if p.is_file())
        if report["display_cache_bytes_after_run"] > 512 * 1024**2:
            raise RuntimeError("persistent cache budget violated")
        summaries.append(report)
        print(f"{scenario}: CPU p50={report['cpu_p50_ms']:.3f} ms, p99={report['cpu_p99_ms']:.3f} ms, "
              f"content={report['content_coverage_fraction']:.1%}, completed={report['gpu_completion_fps']:.1f}/s", flush=True)
    (root / "summary.json").write_text(json.dumps(summaries, indent=2) + "\n", encoding="utf-8")
    print(root)


if __name__ == "__main__":
    main()
