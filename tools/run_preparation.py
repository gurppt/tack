#!/usr/bin/env python3
"""Mission 0.7: charged partial preparation and isolated navigation inputs.

Never generates a warm cache by silently mixing preceding navigation traces.
All cache cloning/inventory is harness work, outside the measured process.
"""
import argparse
import hashlib
import json
from pathlib import Path
import resource
import shutil
import subprocess
import time
import zipfile
from run_benchmarks import distribution

SCENARIOS = ["pan-normal", "pan-fast", "zoom-traverse", "scan", "board-tour", "pan"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def inventory(cache):
    files = sorted(cache.rglob("*.png")) if cache.exists() else []
    sizes = [p.stat().st_size for p in files]
    fingerprint = hashlib.sha256()
    for p in files:
        fingerprint.update(str(p.relative_to(cache)).encode() + b"\0" + bytes.fromhex(digest(p)))
    size_distribution = {key.replace("_ms", "_bytes"): value for key, value in distribution(sizes).items()}
    size_distribution.update(min_bytes=min(sizes, default=None), max_bytes=max(sizes, default=None))
    return {"files": len(files), "overview_files": sum(p.name.endswith("-128.png") for p in files),
            "bytes": sum(sizes), "file_bytes": size_distribution,
            "content_sha256": fingerprint.hexdigest(),
            "temporary_files": sum(1 for _ in cache.rglob("*.tmp")) if cache.exists() else 0}


def concurrency(profiles):
    events = []
    for p in profiles:
        if p.get("decode_started_ms") is not None:
            events += [(p["decode_started_ms"], 1), (p["decode_finished_ms"], -1)]
    active = peak = 0
    for _, delta in sorted(events):
        active += delta
        peak = max(peak, active)
    return peak


def summarize_prep(raw):
    report = {k: v for k, v in raw.items() if k != "worker_profiles"}
    profiles = raw["worker_profiles"]
    report["concurrent_decode_peak"] = concurrency(profiles)
    report["worker_memory_observed_peaks"] = {k: max((p[k] for p in profiles), default=0)
        for k in ["encoded_peak_bytes", "decoded_peak_bytes", "resize_peak_bytes"]}
    report["worker_stages"] = {name: dict(distribution([p["stage_ms"][i] for p in profiles if p["stage_ms"][i] > 0]),
                                          bytes=sum(p["stage_bytes"][i] for p in profiles))
                               for i, name in enumerate(raw["worker_stage_names"])}
    report["preparation"].pop("progress", None)
    return report


def freeze(root):
    root.mkdir(parents=True, exist_ok=False)
    for name, source in [("tack-app", Path("target/release/tack-app")),
                         ("overview_prepare", Path("target/release/examples/overview_prepare"))]:
        shutil.copy2(source, root / name)
    paths = [Path(n) for n in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]]
    paths += [p for p in Path("crates").rglob("*") if p.suffix in [".rs", ".wgsl", ".toml"]]
    paths += list(Path("tools").glob("*.py")) + list(Path(".cargo").glob("*.toml"))
    with zipfile.ZipFile(root / "source-snapshot.zip", "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(paths):
            archive.writestr(str(path), path.read_bytes())
    write(root / "provenance.json", {
        "app_sha256": digest(root / "tack-app"), "preparation_sha256": digest(root / "overview_prepare"),
        "source_snapshot_sha256": digest(root / "source-snapshot.zip"),
        "executed_harness_sha256": digest(Path(__file__)),
        "manifest_sha256": digest(Path("benchmark-data/mission0/manifest.json")),
        "native_build": json.loads(Path("target/native/libjpeg-turbo-3.2.0/native-build.json").read_text()),
        "note": "Fresh processes; cold means derived cache empty, not kernel page cache flushed. Synthetic IDs share 32 contents; no PureRef asset access."})


def preparation(root, name, fraction, workers=2, manifest=None, cache=None):
    cache = cache or root / (name + "-cache")
    manifest = manifest or Path("benchmark-data/mission0/manifest.json")
    input_cache = inventory(cache)
    output = root / (name + ".json")
    command = [str((root / "overview_prepare").resolve()), "--manifest",
               str(manifest.resolve()),
               "--cache", str(cache.resolve()), "--fraction", str(fraction), "--workers", str(workers),
               "--output", str(output.resolve())]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    result = subprocess.run(command, capture_output=True, text=True, check=True)
    wall = time.monotonic() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    (root / (name + ".log")).write_text(result.stderr)
    raw = json.loads(output.read_text())
    report = summarize_prep(raw)
    report.update(process_wall_seconds=wall, process_user_seconds=after.ru_utime-before.ru_utime,
                  process_system_seconds=after.ru_stime-before.ru_stime, cache_after=inventory(cache),
                  input_cache=input_cache, manifest_sha256=digest(manifest),
                  binary_sha256=digest(root / "overview_prepare"), command=command)
    if raw["worker_profile_dropped"] or raw["pending_after_drain"]:
        raise RuntimeError("incomplete preparation accounting")
    if fraction == 1 and (raw["preparation"]["ready"] != raw["preparation"]["total"] or raw["preparation"]["errors"]):
        raise RuntimeError("healthy full preparation did not reach 100% ready")
    print(f"{name}: {report['preparation']['ready']} ready, "
          f"{report['preparation']['elapsed_ms']/1000:.3f}s, RSS {report['rss_high_water_kib']/1024:.1f} MiB", flush=True)
    return report


def prepare_suite(root):
    freeze(root)
    reports = {}
    for percent in [0, 25, 50, 75, 100]:
        reports[str(percent)] = preparation(root, "prepared-" + str(percent), percent/100)
    reports["four_workers"] = preparation(root, "prepared-100-four", 1, 4)
    # Two local synthetic progressive contents, eight distinct IDs each. This
    # isolates the codec scratch multiplier without reading a private fixture.
    fixture = root / "progressive-input"
    fixture.mkdir(exist_ok=False)
    base = json.loads(Path("benchmark-data/mission0/manifest.json").read_text())
    objects = []
    for i in range(16):
        source = Path(f"benchmark-data/mission0_6-progressive/progressive-{i%2*2}.jpg")
        target = fixture / f"{i}.jpg"
        shutil.copy2(source, target)
        obj = dict(base["objects"][i], id=i, path=target.name, source_sha256=digest(target))
        objects.append(obj)
    write(fixture / "manifest.json", dict(base, objects=objects))
    for workers in [2, 4]:
        reports[f"progressive_{workers}"] = preparation(root, f"progressive-{workers}", 1, workers, fixture / "manifest.json")
    # Real full-corpus cancellation/drain above, then restart using the exact
    # drained 25% cache. Keep original input immutable for the navigation suite.
    resumed = root / "restart-cache"
    shutil.copytree(root / "prepared-25-cache", resumed)
    reports["restart"] = preparation(root, "restart", 1, cache=resumed)
    # One missing and one corrupt PNG in an otherwise complete cache.
    repaired = root / "repair-cache"
    shutil.copytree(root / "prepared-100-cache", repaired)
    entries = sorted(repaired.rglob("*.png"))
    entries[0].unlink(); entries[1].write_bytes(b"invalid PNG")
    reports["repair"] = preparation(root, "repair", 1, cache=repaired)
    write(root / "preparation-summary.json", reports)


def navigation_run(root, inputs, label, percent, scenario, ongoing, workers=2, warm_cache=None):
    cache = root / (label + "-cache")
    shutil.copytree(warm_cache or inputs / f"prepared-{percent}-cache", cache)
    before = inventory(cache)
    output = root / label
    command = ["python3", "tools/run_benchmarks.py", "--binary", str(inputs / "tack-app"),
               "--source-snapshot", str(inputs / "source-snapshot.zip"), "--output", str(output),
               "--shared-cache", str(cache), "--workers", str(workers), "--prefetch", "none",
               "--seconds", "12", "--scenarios", scenario]
    if ongoing:
        command += ["--prepare-overview"]
    subprocess.run(command, check=True)
    report = json.loads((output / "summary.json").read_text())[0]
    raw = json.loads((output / (scenario + ".json")).read_text())
    if raw.get("asset_pending_after_drain", 0) or raw.get("worker_profile_dropped", 0):
        raise RuntimeError("incomplete asset accounting")
    after = inventory(cache)
    if after["temporary_files"]:
        raise RuntimeError("navigation cache contains an unpublished temporary entry")
    report["cache_after"] = after
    report["input_cache"] = before
    report["input_prepared_percent"] = 100*before["overview_files"]/1000
    report["concurrent_decode_peak"] = concurrency(raw["worker_profiles"])
    if report.get("preparation"):
        report["preparation"].pop("progress", None)
    report["label"] = label
    report["preparation_cost_ms"] = json.loads((inputs / "preparation-summary.json").read_text())[str(percent)]["preparation"]["elapsed_ms"]
    report["label_note"] = "Prepared input creation time + this fresh process startup + navigation duration + explicit asset drain; cache cloning/inventory excluded harness work."
    return report, cache


def navigate_suite(root, inputs, workers=2, limited=False):
    root.mkdir(parents=True, exist_ok=False)
    reports = []
    percents = [0] if limited else [0, 25, 50, 75, 100]
    scenarios = ["pan-normal", "board-tour"] if limited else SCENARIOS
    for percent in percents:
        for scenario in scenarios:
            label = f"prepared-{percent}-{scenario}"
            report, cache = navigation_run(root, inputs, label, percent, scenario, percent < 100, workers)
            reports.append(report)
            if percent == 100 and not limited:
                report, _ = navigation_run(root, inputs, f"warm-{scenario}", percent, scenario, False, workers, warm_cache=cache)
                reports.append(report)
    write(root / "summary.json", reports)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prepare", "navigate"])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--workers", type=int, choices=[2, 4], default=2)
    parser.add_argument("--limited", action="store_true", help="Only immediate pan and overview for worker sanity checks")
    args = parser.parse_args()
    if args.mode == "prepare":
        prepare_suite(args.output)
    else:
        if args.inputs is None:
            parser.error("navigation requires a frozen preparation input directory")
        navigate_suite(args.output, args.inputs, args.workers, args.limited)


if __name__ == "__main__":
    main()
