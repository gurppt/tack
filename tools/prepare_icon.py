#!/usr/bin/env python3
"""Build-only Tack artwork adapter: the selected bounded M/L/H/C/Z SVG paths.

Uses the existing pinned Pillow asset toolchain; never runs in the application.
Rejects unsupported artwork rather than silently approximating new SVG features.
"""
import hashlib
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw


def polygon(path):
    tokens = re.findall(r'[A-Za-z]|[-+]?(?:\d*\.\d+|\d+)(?:[eE][-+]?\d+)?', path)
    points, pos, i = [], (0., 0.), 0
    while i < len(tokens):
        command = tokens[i]; i += 1
        if command in 'zZ':
            break
        count = {'M': 2, 'L': 2, 'H': 1, 'C': 6}.get(command.upper())
        if count is None or i + count > len(tokens):
            raise ValueError('Unsupported icon SVG path')
        values = list(map(float, tokens[i:i+count])); i += count
        relative = command.islower()
        def pair(index):
            return (values[index] + (pos[0] if relative else 0), values[index+1] + (pos[1] if relative else 0))
        if command.upper() == 'H':
            pos = (values[0] + (pos[0] if relative else 0), pos[1]); points.append(pos)
        elif command.upper() == 'C':
            start, a, b, end = pos, pair(0), pair(2), pair(4)
            for step in range(1, 33):
                t = step / 32; u = 1-t
                points.append(tuple(u**3*start[j] + 3*u*u*t*a[j] + 3*u*t*t*b[j] + t**3*end[j] for j in range(2)))
            pos = end
        else:
            pos = pair(0); points.append(pos)
    if len(points) < 3 or any(not -1 <= v <= 25 for p in points for v in p):
        raise ValueError('Icon geometry outside bounded artwork')
    return points


def prepare(root, output):
    source = root / 'gfx/tack-icon.svg'
    if source.stat().st_size > 4096:
        raise ValueError('Icon SVG exceeds 4 KiB')
    svg = ET.fromstring(source.read_bytes())
    if svg.attrib.get('viewBox') != '0 0 24 24' or not 1 <= len(svg) <= 4:
        raise ValueError('Unexpected icon artwork')
    image = Image.new('RGBA', (512,512))
    draw = ImageDraw.Draw(image)
    for path in svg:
        if path.tag != '{http://www.w3.org/2000/svg}path':
            raise ValueError('Only flat paths supported')
        draw.polygon([(x*512/24,y*512/24) for x,y in polygon(path.attrib['d'])], fill=path.attrib['fill'])
    output.mkdir(parents=True, exist_ok=True)
    hashes = {}
    for size in (16,32,64,128,256):
        small = image.resize((size,size),Image.Resampling.LANCZOS)
        path = output / f'tack-icon-{size}.png'; small.save(path,optimize=True)
        hashes[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
        if size == 32:
            (output/'icon.rgba').write_bytes(small.tobytes())
    image.save(output/'tack-icon.ico', sizes=[(16,16),(32,32),(64,64),(128,128),(256,256)])
    # Share exact pixels with About's existing immediate bitmap geometry.
    small = image.resize((16,16),Image.Resampling.NEAREST)
    pixels = list(small.getdata())
    (output/'icon_pixels.rs').write_text('pub const PIXELS: [[u8;4];256] = [' + ','.join(str(list(p)) for p in pixels) + '];\n')
    receipt = {'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'png_sha256': hashes,
               'original_sha256': hashlib.sha256((root/'gfx/Rhombus--Streamline-Fluent-Ui-Filled.svg').read_bytes()).hexdigest(),
               'runtime': '32px RGBA window icon + 16px About bitmap; no SVG parser'}
    (output/'icon_asset.json').write_text(json.dumps(receipt,indent=2)+'\n')
    return receipt
