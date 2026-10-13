#!/usr/bin/env python3
"""Generate real row-streamed PNGs for storage experiments, not image admission."""
import hashlib
import json
import os
from pathlib import Path
import random
import shutil
import struct
import sys
import zlib

MARKER = "P0 F disposable storage experiment; never a user board\n"
CASES = [("50MiB", 8000, 6554), ("500MiB", 16000, 32768), ("1.5GB", 30000, 50000)]


def generate(root):
    if shutil.disk_usage(root.parent).free < 18 * 1024**3:
        raise RuntimeError("18 GiB required: fixtures, durable generations, 10 GiB reserve")
    root.mkdir(exist_ok=False)
    (root / "OWNER_SAVE_EXPERIMENT.txt").write_text(MARKER)
    receipts = []
    for index, (name, width, height) in enumerate(CASES):
        case = root / name
        case.mkdir()
        (case / "OWNER_SAVE_EXPERIMENT.txt").write_text(MARKER)
        path = case / "source.png"
        sha = hashlib.sha256()
        rng = random.Random(26101300 + index)
        with path.open("xb") as output:
            def write(data):
                sha.update(data)
                output.write(data)

            def chunk(kind, data):
                write(struct.pack(">I", len(data)) + kind + data
                      + struct.pack(">I", zlib.crc32(kind + data)))

            write(b"\x89PNG\r\n\x1a\n")
            chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 0, 0, 0, 0))
            compressor = zlib.compressobj(level=0)
            for _ in range(height):
                compressed = compressor.compress(b"\x00" + rng.randbytes(width))
                if compressed:
                    chunk(b"IDAT", compressed)
            chunk(b"IDAT", compressor.flush())
            chunk(b"IEND", b"")
            output.flush()
            os.fsync(output.fileno())
        receipts.append(dict(case=name, width=width, height=height, pixels="L8, noninterlaced",
                             bytes=path.stat().st_size, sha256=sha.hexdigest(), seed=26101300 + index,
                             scope="valid PNG encoded bytes; storage-only, not render support"))
        print(json.dumps(receipts[-1]), flush=True)
    (root / "fixtures.json").write_text(json.dumps(receipts, indent=2) + "\n")


if __name__ == "__main__":
    generate(Path(sys.argv[1]).resolve())
