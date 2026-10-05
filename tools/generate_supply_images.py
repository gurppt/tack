#!/usr/bin/env python3
"""Deterministic public-domain generated grids for local 1G fixtures."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageDraw


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    rows = []
    for i in range(64):
        image = Image.new('RGB', (1600, 1000))
        d = ImageDraw.Draw(image)
        d.rectangle((0, 0, 1599, 999), fill=(30+i*3, 60+i, 90+i))
        for x in range(0, 1600, 8):
            d.line((x, 0, x, 999), fill=(210, i*3, 70))
        for y in range(0, 1000, 8):
            d.line((0, y, 1599, y), fill=(80, 190, i*3))
        path = a.output/f'{i:03}.png'; image.save(path)
        rows.append({'file': path.name, 'bytes': path.stat().st_size,
                     'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    (a.output/'inventory.json').write_text(json.dumps(rows, indent=2)+'\n')


if __name__ == '__main__':
    main()
