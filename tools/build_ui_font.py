#!/usr/bin/env python3
"""Pack the pinned existing Spleen bitmap; no rasterization or custom font design."""
import hashlib
import json
from pathlib import Path
import struct


def convert(root):
    provenance = json.loads((root / 'provenance.json').read_text())
    data = (root / 'spleen-8x16.bdf').read_bytes()
    if hashlib.sha256(data).hexdigest() != provenance['files']['spleen-8x16.bdf']['sha256']:
        raise ValueError('pinned BDF checksum differs')
    records = {}
    for block in data.decode('ascii').split('STARTCHAR ')[1:]:
        lines = block.splitlines()
        encoding = int(next(line[9:] for line in lines if line.startswith('ENCODING ')))
        box = next(line for line in lines if line.startswith('BBX '))
        if box != 'BBX 8 16 0 -4':
            raise ValueError('unexpected glyph box')
        start = lines.index('BITMAP') + 1
        rows = bytes.fromhex(''.join(lines[start:start + 16]))
        if len(rows) != 16 or not 0 <= encoding <= 0x10ffff or encoding in records:
            raise ValueError('invalid glyph')
        records[encoding] = b''.join(struct.pack('>H', row << 8) for row in rows)
    return b''.join(struct.pack('<IB', code, 8) + rows for code, rows in sorted(records.items()))


if __name__ == '__main__':
    root = Path(__file__).resolve().parent.parent / 'assets/ui-font'
    result = convert(root)
    (root / 'spleen-glyphs.bin').write_bytes(result)
    print(f'{len(result)} bytes; SHA256 {hashlib.sha256(result).hexdigest()}')
