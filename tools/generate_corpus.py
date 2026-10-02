#!/usr/bin/env python3
"""Deterministic Mission 0 corpus; no network or private images required."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import shutil

from PIL import Image, ImageDraw, __version__ as pillow_version


def generate(output, count=1000, sources=32, width=6000, height=4500, seed=20261002):
    if not (1 <= count <= 10000 and 1 <= sources <= count):
        raise ValueError("require 1 <= sources <= count <= 10000")
    if not (1 <= width <= 6000 and 1 <= height <= 4500):
        raise ValueError("dimensions exceed the Mission 0 decoder limits")
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        raise ValueError("output directory must be empty; existing corpora are never overwritten")
    source_dir = output / "sources"
    image_dir = output / "images"
    source_dir.mkdir()
    image_dir.mkdir()
    hashes = []
    for index in range(sources):
        rng = random.Random(seed + index)
        field = Image.frombytes("RGB", (256, 192), rng.randbytes(256 * 192 * 3))
        image = field.resize((width, height), Image.Resampling.BICUBIC)
        draw = ImageDraw.Draw(image)
        for _ in range(80):
            x, y = rng.randrange(width), rng.randrange(height)
            draw.rectangle((x, y, min(width - 1, x + 80), min(height - 1, y + 80)),
                           fill=tuple(rng.randrange(256) for _ in range(3)))
        path = source_dir / f"source-{index:04d}.jpg"
        image.save(path, "JPEG", quality=90, subsampling=0, optimize=False)
        hashes.append(hashlib.sha256(path.read_bytes()).hexdigest())
    columns = math.ceil(math.sqrt(count))
    objects = []
    for index in range(count):
        source = source_dir / f"source-{index % sources:04d}.jpg"
        path = image_dir / f"image-{index:04d}.jpg"
        try:
            os.link(source, path)
        except OSError:
            shutil.copyfile(source, path)
        objects.append({"id": index, "path": path.relative_to(output).as_posix(),
                        "source_sha256": hashes[index % sources],
                        "x": (index % columns) * 6600, "y": (index // columns) * 5100,
                        "width": width, "height": height})
    manifest = {"schema": 1, "seed": seed, "source_count": sources,
                "pillow_version": pillow_version, "objects": objects,
                "limitation": "Distinct asset IDs and paths reuse hardlinked JPEG content. "
                              "OS filesystem caching and visual diversity differ from 1000 unique photographs."}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--count", type=int, default=1000)
    parser.add_argument("--sources", type=int, default=32)
    parser.add_argument("--width", type=int, default=6000)
    parser.add_argument("--height", type=int, default=4500)
    parser.add_argument("--seed", type=int, default=20261002)
    args = parser.parse_args()
    try:
        result = generate(**vars(args))
    except (ValueError, OSError) as error:
        parser.exit(1, f"corpus generation failed: {error}\n")
    print(json.dumps({"objects": len(result["objects"]), "sources": result["source_count"]}))


if __name__ == "__main__":
    main()
