#!/usr/bin/env python3
"""Measure the two separator lines in run_phase2a6_ui's initial captures."""
import argparse
import json
from PIL import Image
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    args = parser.parse_args()
    checks = []
    for run in sorted(args.root.iterdir()):
        if not run.name.startswith(('800x600-', '1024x768-')):
            continue
        size, scaled, placement = run.name.split('-')
        w, h = map(int, size.split('x'))
        scale = int(scaled[:-1]); side = 16 * scale
        vertical = placement in ('Left', 'Right')
        if placement in ('Top', 'Bottom'):
            x, y = round((w / scale - 210) / 2) * scale, 0 if placement == 'Top' else h-side
        elif vertical:
            x, y = 0 if placement == 'Left' else w-side, round((h / scale - 210) / 2) * scale
        else:
            x, y = 48 * scale, 48 * scale
        image = Image.open(run / 'artist-bar.png').convert('RGB')
        for offset in (128, 177):
            a, b = (x, y+offset*scale) if vertical else (x+offset*scale, y)
            x2, y2 = (a+side, b+scale) if vertical else (a+scale, b+side)
            color = image.getpixel(((a+x2)//2, (b+y2)//2))
            pixels = [(px, py) for py in range(b, y2) for px in range(a, x2)
                      if image.getpixel((px, py)) == color]
            actual = [max(px for px, py in pixels)-min(px for px, py in pixels)+1,
                      max(py for px, py in pixels)-min(py for px, py in pixels)+1]
            expected = [12*scale, scale] if vertical else [scale, 12*scale]
            checks.append(dict(run=run.name, offset=offset, actual=actual,
                               expected=expected, observed=actual == expected))
    (args.root / 'separator-pixels.json').write_text(json.dumps(checks, indent=2)+'\n')
    if len(checks) != 30 or not all(c['observed'] for c in checks):
        raise AssertionError(checks)
    print('30 native separator pixel measurements PASS')


if __name__ == '__main__':
    main()
