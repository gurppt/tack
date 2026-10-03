#!/usr/bin/env python3
"""Linux Mission 0.7 recovery proof against a frozen preparation executable."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from PIL import Image
from run_preparation import digest, inventory, summarize_prep, write


def run(inputs, root, name, manifest, cache):
    output = root / (name + ".json")
    command = [str((inputs / "overview_prepare").resolve()), "--manifest", str(manifest.resolve()),
               "--cache", str(cache.resolve()), "--output", str(output.resolve())]
    start = time.monotonic()
    result = subprocess.run(command, capture_output=True, text=True)
    (root / (name + ".log")).write_text(result.stderr)
    result.check_returncode()
    report = summarize_prep(json.loads(output.read_text()))
    report.update(command=command, manifest_sha256=digest(manifest),
                  binary_sha256=digest(inputs / "overview_prepare"),
                  process_wall_seconds=time.monotonic()-start, cache_after=inventory(cache))
    if report["pending_after_drain"] or report["worker_profile_dropped"]:
        raise RuntimeError("incomplete recovery accounting")
    return report


def interrupted(inputs, root, manifest):
    cache = root / "killed-cache"
    command = [str((inputs / "overview_prepare").resolve()), "--manifest", str(manifest.resolve()),
               "--cache", str(cache.resolve()), "--output", str((root / "interrupted.json").resolve())]
    start = time.monotonic()
    with (root / "interrupted.log").open("w") as log:
        process = subprocess.Popen(command, stdout=log, stderr=log)
        try:
            while len(list(cache.rglob("*.png"))) < 250:
                if process.poll() is not None or time.monotonic()-start > 60:
                    raise RuntimeError("interrupt threshold not reached")
                time.sleep(.02)
            process.kill()  # SIGKILL: no graceful worker drain.
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
    elapsed = time.monotonic()-start
    if process.returncode != -9:
        raise RuntimeError("expected Linux SIGKILL")
    before = inventory(cache)
    for path in cache.rglob("*.png"):
        with Image.open(path) as image:
            image.load()
            if image.size != (128, 96) or image.mode != "RGBA":
                raise RuntimeError("interruption published an invalid thumbnail")
    resumed = run(inputs, root, "hard-restart", manifest, cache)
    prep = resumed["preparation"]
    if prep["ready"] != 1000 or prep["errors"] or prep["cache_hits"] != before["overview_files"]:
        raise RuntimeError("restart failed to reuse exactly the published cache entries")
    if prep["source_read_attempts"] != 1000-before["overview_files"]:
        raise RuntimeError("unexpected duplicate source work on restart")
    if resumed["cache_after"]["temporary_files"]:
        raise RuntimeError("restart left unpublished entries")
    return {"command": command, "exit_code": process.returncode, "time_until_kill_seconds": elapsed,
            "cache_after_kill": before, "restart": resumed,
            "note": "SIGKILL consistency, not power-loss durability; interrupted worker telemetry is unavailable."}


def invalid_source(inputs, root, manifest):
    fixture = root / "invalid-input"
    fixture.mkdir()
    board = json.loads(manifest.read_text())
    for obj in board["objects"]:
        target = fixture / obj["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        os.link(manifest.parent / obj["path"], target)
    bad = fixture / "invalid.jpg"
    bad.write_bytes(bytes([255, 216, 255, 192, 0, 1]))
    board["objects"][0].update(path=bad.name, source_sha256=digest(bad))
    changed = fixture / "manifest.json"
    write(changed, board)
    cache = root / "invalid-cache"
    shutil.copytree(inputs / "prepared-100-cache", cache)
    # Remove the old source's cache identity; the invalid source has a new hash.
    for path in cache.rglob("0-*-128.png"):
        path.unlink()
    report = run(inputs, root, "invalid-source", changed, cache)
    p = report["preparation"]
    if (p["ready"], p["errors"], p["cache_hits"], p["source_read_attempts"]) != (999, 1, 999, 1):
        raise RuntimeError("one invalid source blocked valid prepared assets")
    if not p["settled"] or p["threshold_ms"]["100"] is not None or report["stop_reason"] != "settled":
        raise RuntimeError("errors were mislabeled as 100% preparation")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = Path("benchmark-data/mission0/manifest.json")
    reports = {"hard_interruption": interrupted(args.inputs, args.output, manifest),
               "invalid_source": invalid_source(args.inputs, args.output, manifest)}
    cache = args.output / "warm-cache"
    shutil.copytree(args.inputs / "prepared-100-cache", cache)
    reports["all_warm"] = run(args.inputs, args.output, "all-warm", manifest, cache)
    if reports["all_warm"]["preparation"]["cache_hits"] != 1000 or reports["all_warm"]["preparation"]["source_read_attempts"]:
        raise RuntimeError("warm preparation decoded original sources")
    reports["executed_harness_sha256"] = digest(Path(__file__))
    write(args.output / "summary.json", reports)
    print(json.dumps({"killed_after_seconds": reports["hard_interruption"]["time_until_kill_seconds"],
                      "published_at_kill": reports["hard_interruption"]["cache_after_kill"]["overview_files"],
                      "invalid_ready": reports["invalid_source"]["preparation"]["ready"],
                      "all_warm_ms": reports["all_warm"]["preparation"]["elapsed_ms"]}))


if __name__ == "__main__":
    main()
