#!/usr/bin/env python3
"""Generate the mission's square JPEG without materializing its raster."""
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import time


def main():
    root = Path(__file__).resolve().parents[1]
    dest = root / "benchmark-results/phase2a2/fixtures"
    dest.mkdir(parents=True, exist_ok=True)
    image = dest / "50000-square.jpg"
    receipt = dest / "generation.json"
    if image.exists() or image.is_symlink():
        raise SystemExit("Refusing to replace an existing fixture")
    if shutil.disk_usage(root).free < 11 * 1024**3:
        raise SystemExit("Insufficient disk reserve")
    encoder = root / "target/native/libjpeg-turbo-3.2.0/bin/cjpeg"
    args = [str(encoder), "-baseline", "-grayscale", "-quality", "70",
            "-outfile", str(image)]
    started = time.monotonic()
    process = subprocess.Popen(args, stdin=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        process.stdin.write(b"P5\n50000 50000\n255\n")
        base = bytes((x // 256 * 7 + x // 32 * 3) % 256 for x in range(50000))
        for y in range(0, 50000, 32):
            offset = (y // 256 * 11 + y // 32 * 5) % 256
            table = bytes((value + offset) % 256 for value in range(256))
            process.stdin.write(base.translate(table) * min(32, 50000 - y))
            if image.stat().st_size > 128 * 1024**2:
                raise RuntimeError("Fixture exceeds the explicit 128 MiB disk ceiling")
        process.stdin.close()
        error = process.stderr.read().decode(errors="replace")
        if process.wait() != 0:
            raise RuntimeError(error)
    except BaseException:
        process.kill()
        process.wait()
        image.unlink(missing_ok=True)
        raise
    status = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines() if ":" in line) if Path("/proc/self/status").exists() else {}
    generator_hwm_kib = int(status["VmHWM"].split()[0]) if "VmHWM" in status else resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    digest = hashlib.sha256()
    with image.open("rb") as source:
        while block := source.read(65536):
            digest.update(block)
    result = {
        "dimensions": [50000, 50000], "encoded_bytes": image.stat().st_size,
        "source_sha256": digest.hexdigest(), "generation_seconds": time.monotonic()-started,
        "logical_pgm_bytes": 2500000000, "full_raster_file": False,
        "row_bytes": 50000, "maximum_feed_block_bytes": 1600000,
        "pattern": "((x//256*7+x//32*3)+(y//256*11+y//32*5))%256",
        "encoder": "libjpeg-turbo 3.2.0 cjpeg, baseline grayscale quality 70",
        "generator_rss_kib": generator_hwm_kib,
        "generator_rss_source": "/proc/self/status VmHWM (RUSAGE_SELF may inherit launcher high-water)",
        "encoder_hwm_kib": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
        "free_disk_after_bytes": shutil.disk_usage(root).free,
    }
    receipt.write_text(json.dumps(result, indent=2)+"\n")
    print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
