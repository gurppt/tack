#!/usr/bin/env python3
"""Build-only raster/SDF conversion; no font parser or Python dependency ships."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import struct

import numpy as np
from PIL import Image, ImageDraw, ImageFont, features
from fontTools.ttLib import TTFont
from scipy.ndimage import distance_transform_edt


def build(root):
    provenance = json.loads((root / 'provenance.json').read_text())
    source = gzip.decompress((root / 'LiberationMono-Regular.ttf.gz').read_bytes())
    if hashlib.sha256(source).hexdigest() != provenance['source_sha256']:
        raise ValueError('font source checksum differs')
    cmap = TTFont(io.BytesIO(source)).getBestCmap()
    codes = [c for c in range(32, 0x500) if c in cmap and not 0x7f <= c < 0xa0]
    columns, tile_w, tile_h, factor = 32, 32, 48, 4
    rows = (len(codes) + columns - 1) // columns
    atlas = np.zeros((rows * tile_h, columns * tile_w), dtype=np.uint8)
    font = ImageFont.truetype(io.BytesIO(source), 32 * factor, layout_engine=ImageFont.Layout.BASIC)
    slots = [0xffff] * 0x500
    for slot, code in enumerate(codes):
        image = Image.new('L', (tile_w * factor, tile_h * factor))
        ImageDraw.Draw(image).text((4 * factor, 32 * factor), chr(code), font=font, fill=255, anchor='ls')
        inside = np.asarray(image) >= 128
        distance = (distance_transform_edt(inside) - distance_transform_edt(~inside)) / factor
        distance = distance.reshape(tile_h, factor, tile_w, factor).mean(axis=(1, 3))
        sdf = np.clip(np.rint(128 + distance * 16), 0, 255).astype(np.uint8)
        y, x = slot // columns * tile_h, slot % columns * tile_w
        atlas[y:y+tile_h, x:x+tile_w] = sdf
        slots[code] = slot
    encoded = io.BytesIO()
    Image.fromarray(atlas).save(encoded, format='PNG', compress_level=9)
    files = {'atlas.png': encoded.getvalue(), 'slots.bin': struct.pack('<1280H', *slots)}
    generated = {name: {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()} for name, data in files.items()}
    if 'generated' in provenance and generated != provenance['generated']:
        raise ValueError('conversion differs from pinned derivative')
    for name, data in files.items():
        (root / name).write_bytes(data)
    provenance.update({'generated': generated, 'atlas_size': list(atlas.shape[::-1]), 'glyphs': len(codes), 'freetype': features.version('freetype2'), 'advance_em': font.getlength('M')/(32*factor)})
    (root / 'provenance.json').write_text(json.dumps(provenance, indent=2)+'\n')
    print(json.dumps(provenance))


if __name__ == '__main__':
    build(Path(__file__).resolve().parent.parent / 'assets/note-font')
