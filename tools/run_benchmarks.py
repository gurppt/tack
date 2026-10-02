#!/usr/bin/env python3
"""Run isolated, reproducible Mission 0 scenarios and retain raw frame telemetry."""
import argparse
import datetime
import json
import hashlib
from pathlib import Path
import platform
import subprocess
import shutil
import zipfile


def distribution(values):
    values = sorted(values)
    def at(p):
        return values[min(len(values)-1, int((len(values)-1)*p + 0.999999))] if values else None
    return {"samples": len(values), "p50_ms": at(.5), "p95_ms": at(.95),
            "p99_ms": at(.99), "total_ms": sum(values)}


def demand_latency(episodes, field):
    return dict(distribution([e[field] for e in episodes if e[field] is not None]),
                censored=sum(e[field] is None for e in episodes))


def check_frame_bounds(frames, pressure, pending_limit, upload_limit):
    for frame in frames:
        if frame["cpu_cache_bytes"] > (16 if pressure else 64) * 1024**2:
            raise RuntimeError("CPU cache budget violated")
        if frame["gpu_resident_bytes"] > (24 if pressure else 128) * 1024**2:
            raise RuntimeError("GPU cache budget violated")
        if frame["pending"] > pending_limit or frame["in_flight"] > 3 or frame["upload_bytes"] > 16 * 1024**2 or frame["uploads"] > upload_limit:
            raise RuntimeError("queue or upload budget violated")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/tack-app"))
    parser.add_argument("--manifest", type=Path, default=Path("benchmark-data/mission0/manifest.json"))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--seconds", type=float, default=12)
    parser.add_argument("--headless", action="store_true")
    parser.add_argument("--scenarios", nargs="+", default=["cold", "warm", "pan", "zoom", "pressure"])
    parser.add_argument("--workers", type=int, default=2)
    parser.add_argument("--prefetch", choices=["none", "symmetric", "directional"], default="none")
    parser.add_argument("--shared-cache", type=Path, help="Explicit retained SSD cache, with a fresh process for every scenario")
    parser.add_argument("--source-snapshot", type=Path, help="Reuse the code snapshot paired with --binary for a multi-process suite")
    args = parser.parse_args()
    root = args.output or Path("benchmark-results") / datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    root.mkdir(parents=True, exist_ok=False)
    binary_snapshot = root / args.binary.name
    shutil.copy2(args.binary, binary_snapshot)
    corpus = json.loads(args.manifest.read_text(encoding="utf-8"))
    environment = {"platform": platform.platform(), "processor": platform.processor(),
                   "manifest_sha256": hashlib.sha256(args.manifest.read_bytes()).hexdigest(),
                   "headless": args.headless, "corpus_objects": len(corpus["objects"]),
                   "corpus_seed": corpus["seed"], "corpus_sources": corpus["source_count"],
                   "pillow_version": corpus["pillow_version"], "limitations": corpus["limitation"]}
    source_root = Path(__file__).resolve().parent.parent
    source_paths = [source_root / "Cargo.toml", source_root / "Cargo.lock", source_root / "rust-toolchain.toml"]
    source_paths += [p for p in (source_root / "crates").rglob("*") if p.suffix in [".rs", ".wgsl", ".toml"]]
    source_paths += list((source_root / "tools").glob("*.py"))
    source_paths += [p for p in (source_root / ".cargo").glob("*.toml")]
    digest = hashlib.sha256()
    if args.source_snapshot is not None:
        shutil.copy2(args.source_snapshot, root / "source-snapshot.zip")
        with zipfile.ZipFile(root / "source-snapshot.zip") as archive:
            for relative in sorted(archive.namelist()):
                digest.update(relative.encode() + b"\0" + archive.read(relative) + b"\0")
    else:
        with zipfile.ZipFile(root / "source-snapshot.zip", "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(source_paths):
                relative = str(path.relative_to(source_root))
                content = path.read_bytes()
                digest.update(relative.encode() + b"\0" + content + b"\0")
                archive.writestr(relative, content)
    environment["source_sha256"] = digest.hexdigest()
    environment["binary_sha256"] = hashlib.sha256(binary_snapshot.read_bytes()).hexdigest()
    environment["executed_harness_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    native_build = source_root / "target/native/libjpeg-turbo-3.2.0/native-build.json"
    if native_build.exists():
        environment["available_native_build"] = json.loads(native_build.read_text())
    for tool, command in {"cpu": ["lscpu"], "memory": ["free", "-b"], "gpu_pci": ["lspci", "-nn"],
                          "rustc": ["rustc", "--version"], "cargo": ["cargo", "--version"],
                          "nvidia": ["nvidia-smi", "--query-gpu=name,driver_version,memory.total", "--format=csv"]}.items():
        try:
            environment[tool] = subprocess.run(command, capture_output=True, text=True, check=True).stdout
        except (OSError, subprocess.CalledProcessError):
            environment[tool] = None
    (root / "environment.json").write_text(json.dumps(environment, indent=2), encoding="utf-8")
    summaries = []
    for scenario in args.scenarios:
        # Cold and warm deliberately share only SSD data, never decoded RAM/VRAM.
        cache = args.shared_cache or root / ("cold-warm-cache" if scenario in ["cold", "warm"] else scenario + "-cache")
        command = [str(binary_snapshot.resolve()), "--manifest", str(args.manifest.resolve()),
                   "--cache", str(cache.resolve()), "--scenario", scenario, "--seconds", str(args.seconds),
                   "--output", str((root / (scenario + ".json")).resolve())]
        command += ["--workers", str(args.workers), "--prefetch", args.prefetch]
        if args.headless:
            command.append("--headless")
        result = subprocess.run(command, capture_output=True, text=True)
        (root / (scenario + ".log")).write_text(result.stderr, encoding="utf-8")
        if result.returncode:
            parser.exit(result.returncode, f"{scenario} failed: {result.stderr}\n")
        report = json.loads((root / (scenario + ".json")).read_text(encoding="utf-8"))
        frames = report.pop("frames")
        report.pop("gpu_pass_samples", None)
        profiles = report.pop("worker_profiles", [])
        episodes = report.pop("coverage_episodes", [])
        names = report.get("worker_stage_names", [])
        report["worker_stages"] = {name: dict(distribution([p["stage_ms"][i] for p in profiles if p["stage_ms"][i] > 0]), bytes=sum(p["stage_bytes"][i] for p in profiles)) for i,name in enumerate(names)}
        report["worker_active"] = distribution([p["active_ms"] for p in profiles])
        report["first_image_after_demand"] = demand_latency(episodes, "first_image_delay_ms")
        report["requested_lod_after_demand"] = demand_latency(episodes, "requested_delay_ms")
        report["cancelled_by_stage"] = {stage: sum(p["cancelled_after"] == stage for p in profiles) for stage in sorted({p["cancelled_after"] for p in profiles if p["cancelled_after"]})}
        report["worker_memory_observed_peaks"] = {name: max((p[name] for p in profiles), default=0) for name in ["encoded_peak_bytes", "decoded_peak_bytes", "resize_peak_bytes"]}
        check_frame_bounds(frames, scenario == "pressure", report.get("pending_limit", args.workers), report.get("upload_count_limit", 2))
        visible = sum(f["visible"] for f in frames)
        report["content_coverage_fraction"] = (sum(f["visible"] - f["placeholders"] for f in frames) / visible) if visible else None
        report["lod_coverage_fractions"] = [sum(f["lods"][i] for f in frames)/visible if visible else None for i in range(3)]
        report["requested_coverage_fraction"] = sum(f["requested_covered"] for f in frames)/visible if visible and "requested_covered" in frames[0] else None
        report["worker_utilization_fraction"] = report["worker_active"]["total_ms"]/(args.seconds*1000*args.workers)
        report["worker_utilization_note"] = "Completed observed worker time / run worker capacity; excludes jobs still active at exit, therefore a lower bound."
        report["cpu_thumbnail_peak_bytes"] = max((f.get("cpu_thumbnail_bytes",0) for f in frames),default=0)
        report["gpu_thumbnail_peak_bytes"] = max((f.get("gpu_thumbnail_bytes",0) for f in frames),default=0)
        report["gpu_completion_fps"] = report["completed_submissions_at_report"] / args.seconds
        report["display_cache_bytes_after_run"] = sum(p.stat().st_size for p in cache.rglob("*") if p.is_file())
        report["display_cache_files_after_run"] = sum(p.is_file() for p in cache.rglob("*"))
        if report["display_cache_bytes_after_run"] > 512 * 1024**2:
            raise RuntimeError("persistent cache budget violated")
        summaries.append(report)
        print(f"{scenario}: CPU p50={report['cpu_p50_ms']:.3f} ms, p99={report['cpu_p99_ms']:.3f} ms, "
              f"content={report['content_coverage_fraction']:.1%}, completed={report['gpu_completion_fps']:.1f}/s", flush=True)
    (root / "summary.json").write_text(json.dumps(summaries, indent=2) + "\n", encoding="utf-8")
    print(root)


if __name__ == "__main__":
    main()
