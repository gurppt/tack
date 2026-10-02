#!/usr/bin/env python3
"""Mission 0.5 cold traces, full-board SSD preparation, and matched warm reopens."""
import argparse
import datetime
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--seconds", type=float, default=12)
    parser.add_argument("--prepare-seconds", type=float, default=120)
    parser.add_argument("--workers", type=int, choices=[1,2,4], default=2)
    parser.add_argument("--headless", action="store_true")
    args = parser.parse_args()
    root = args.output or Path("benchmark-results") / ("mission0_5-" + datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
    root.mkdir(parents=True, exist_ok=False)
    runner = Path(__file__).with_name("run_benchmarks.py")
    def run(label, scenarios, seconds, cache=None):
        command = ["python3", str(runner), "--output", str(root/label), "--seconds",str(seconds), "--workers",str(args.workers), "--prefetch","none", "--scenarios", *scenarios]
        if label != "cold":
            command += ["--binary", str(root/"cold"/"tack-app"), "--source-snapshot", str(root/"cold"/"source-snapshot.zip")]
        if cache is not None: command += ["--shared-cache",str(cache)]
        if args.headless: command += ["--headless"]
        with (root/(label+".log")).open("w") as log:
            subprocess.run(command,stdout=log,stderr=log,check=True)
        print(label, flush=True)
    run("cold", ["cold","warm","pan","zoom","pressure","pan-slow","pan-normal","pan-fast","zoom-traverse","scan","board-tour"], args.seconds)
    cache = root/"cold"/"board-tour-cache"
    manifest = json.loads(Path("benchmark-data/mission0/manifest.json").read_text())
    expected = {o["id"] for o in manifest["objects"]}
    attempts = []
    for attempt in range(3):
        label = f"prepare-{attempt}"
        run(label, ["board-tour"], args.prepare_seconds, cache)
        found = {int(p.name.split("-")[0]) for p in cache.rglob("*-128.png")}
        attempts.append(dict(label=label, thumbnails=len(found & expected)))
        if found >= expected: break
    else:
        raise RuntimeError("full-board overview preparation incomplete; no warm-board claim")
    run("warm-board", ["board-tour","pan-slow","pan-normal","pan-fast","zoom-traverse","scan"], args.seconds, cache)
    (root/"suite.json").write_text(json.dumps(dict(preparation=attempts, cold_duration=args.seconds, warm_duration=args.seconds, workers=args.workers, full_board_overview_count=len(expected), note="Every scenario is a new process with empty CPU/GPU caches. Preparation uses the ordinary source pipeline; only SSD persists."),indent=2)+"\n")
    print(root)


if __name__ == "__main__":
    main()
