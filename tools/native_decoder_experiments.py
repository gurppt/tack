#!/usr/bin/env python3
"""Run isolated fresh-process codec comparisons, never concurrently with GPU traces."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
from PIL import Image, ImageChops, ImageStat
from run_benchmarks import distribution


def run(sources, output, repetitions):
    output.mkdir(parents=True, exist_ok=False)
    binary = output / "thumbnail_microbench"
    shutil.copy2("target/release/examples/thumbnail_microbench", binary)
    records = {}
    for mode in ["reference", "native"]:
        report = output / (mode + ".json")
        command = [str(binary.resolve()), mode, str(sources.resolve()), str(repetitions), str(report.resolve())]
        memory = output / (mode + "-time.txt")
        subprocess.run(["/usr/bin/time", "-v", "-o", str(memory), *command], check=True)
        data = json.loads(report.read_text())
        data["process_peak_rss_kib"] = int(re.search(r"Maximum resident set size \(kbytes\): (\d+)", memory.read_text())[1])
        data["distributions"] = {key: distribution([r[key] for r in data["rows"]]) for key in ["header_ms", "decode_ms", "resize_ms", "total_ms"]}
        data["rss_per_iteration_kib"] = [max(r["rss_kib_after_decode"] for r in data["rows"] if r["iteration"] == i) for i in range(repetitions)]
        records[mode] = data
    comparisons = []
    for path in sorted(sources.glob("*.jpg"))[:8]:
        a = Image.open(output / ("reference-" + path.stem + ".png")).convert("RGB")
        b = Image.open(output / ("native-" + path.stem + ".png")).convert("RGB")
        if a.size != b.size:
            raise RuntimeError("decoder thumbnail dimensions differ")
        diff = ImageStat.Stat(ImageChops.difference(a, b))
        mse = sum(v * v for v in diff.rms) / 3
        comparisons.append(dict(source=path.name, source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
            dimensions=a.size, mean_absolute_rgb_difference=diff.mean, psnr_db=10*math.log10(255**2/mse) if mse else None))
    result = dict(schema=1, harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), comparisons=comparisons, variants=records,
        native_build=json.loads(Path("target/native/libjpeg-turbo-3.2.0/native-build.json").read_text()),
        limitation="Fresh processes; source I/O outside codec timings; same resize; no PNG encoding. Postdecode RSS samples miss scratch peaks; time -v captures process peak. Repeated-cycle plateau is a diagnostic, not a proof against all leaks.")
    attribution = sources / "attribution.json"
    if attribution.exists():
        result["photo_attribution"] = json.loads(attribution.read_text())
        shutil.copy2(attribution, output / "attribution.json")
    (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, default=Path("benchmark-data/mission0/sources"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repetitions", type=int, default=20)
    args = parser.parse_args()
    run(args.sources, args.output, args.repetitions)
