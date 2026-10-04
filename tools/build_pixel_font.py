#!/usr/bin/env python3
"""Reproduce Tack Label Bitmap from pinned, retained OFL Unifont HEX sources."""
import gzip
import hashlib
import json
from pathlib import Path
import struct


def convert(root):
    provenance = json.loads((root / 'provenance.json').read_text())
    records = {}
    for name in ('unifont-18.0.01.hex.gz', 'unifont_upper-18.0.01.hex.gz'):
        data = (root / name).read_bytes()
        if hashlib.sha256(data).hexdigest() != provenance[f'font-builds/{name}']['sha256']:
            raise ValueError('pinned font input checksum differs')
        for line in gzip.decompress(data).decode('ascii').splitlines():
            code, hex_rows = line.split(':')
            bitmap = bytes.fromhex(hex_rows)
            if len(bitmap) not in (16, 32):
                raise ValueError('unexpected glyph dimensions')
            rows = b''.join((b << 8).to_bytes(2, 'big') for b in bitmap) if len(bitmap) == 16 else bitmap
            records[int(code, 16)] = (8 if len(bitmap) == 16 else 16, rows)
    result = b''.join(struct.pack('<IB', code, width) + rows for code, (width, rows) in sorted(records.items()))
    if hashlib.sha256(result).hexdigest() != provenance['tack-label-glyphs.bin']['sha256']:
        raise ValueError('font conversion checksum differs')
    return result


if __name__ == '__main__':
    root = Path(__file__).resolve().parent.parent / 'assets/pixel-font'
    result = convert(root)
    (root / 'tack-label-glyphs.bin').write_bytes(result)
    print(f'{len(result)} bytes; exact pinned conversion')
