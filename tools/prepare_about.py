#!/usr/bin/env python3
"""Validate editable About metadata and derive a bounded, lossless pixel asset."""
import argparse
import hashlib
import json
from pathlib import Path
import tomllib
from PIL import Image
from prepare_icon import prepare as prepare_icon

FIELDS = ('name', 'version', 'author', 'website', 'contact', 'license',
          'copyright', 'tagline', 'source')
LABELS = {'author': 'Author', 'website': 'Website', 'contact': 'Contact',
          'license': 'License', 'copyright': 'Copyright', 'source': 'Source'}
MAX_HEIGHT = 224
MAX_PNG_BYTES = 128 * 1024


def wrapped(text, columns=18):
    columns = max(2, columns)
    lines, line, width = [], '', 0
    for word in text.split():
        word_width = sum(1 if ord(c) < 128 else 2 for c in word)
        if line and width + 1 + word_width <= columns:
            line += ' ' + word
            width += 1 + word_width
            continue
        if line:
            lines.append(line)
            line, width = '', 0
        for c in word:
            cells = 1 if ord(c) < 128 else 2
            if width + cells > columns:
                lines.append(line)
                line, width = '', 0
            line += c
            width += cells
    if line:
        lines.append(line)
    return lines


def validate(metadata, version):
    if set(metadata) - set(FIELDS):
        raise ValueError('Unknown About field; use the documented tiny schema')
    result = {}
    for field in FIELDS:
        value = metadata.get(field, '')
        if not isinstance(value, str) or len(value) > 96 or any(ord(c) < 32 or ord(c) == 127 for c in value):
            raise ValueError(f'{field}: expected one line of at most 96 characters')
        result[field] = value.strip()
    if not result['name']:
        raise ValueError('About name must not be empty')
    if result['version'] and result['version'] != version:
        raise ValueError('About version must match Cargo package version (or be omitted)')
    result['version'] = version
    rows = wrapped(result['name']) + wrapped('Version ' + version)
    if result['tagline']:
        rows += wrapped(result['tagline'])
    for key, label in LABELS.items():
        if result[key] or key in ('author', 'website', 'contact', 'license'):
            rows += wrapped(label + ': ' + (result[key] or 'Not set'))
    if len(rows) > 13:
        raise ValueError('About text exceeds 800x600 at 2x; shorten the fields (13 wrapped rows at 18 cells)')
    return result


def rust_string(value):
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"') + '"'


def prepare(root, output, version):
    metadata = validate(tomllib.loads((root / 'gfx/about.toml').read_text(encoding='utf-8')), version)
    source = root / 'gfx/tack_about.png'
    with Image.open(source) as original:
        if original.format != 'PNG' or max(original.size) > 4096 or original.width == 0 or original.height == 0:
            raise ValueError('About source must be a PNG with dimensions 1..4096')
        ratio = original.width / original.height
        if ratio > .95:
            raise ValueError('About artwork must retain the portrait aspect (width/height <= 0.95) for the bounded two-column layout')
        width = max(1, round(MAX_HEIGHT * ratio))
        derived = original.convert('RGBA').resize((width, MAX_HEIGHT), Image.Resampling.NEAREST)
    output.mkdir(parents=True, exist_ok=True)
    portrait = output / 'about.png'
    derived.save(portrait, format='PNG', optimize=True, compress_level=9)
    if portrait.stat().st_size > MAX_PNG_BYTES:
        raise ValueError('Compact About PNG exceeds 128 KiB')
    code = ['pub const METADATA: Metadata = Metadata {']
    code += [f'    {key}: {rust_string(value)},' for key, value in metadata.items()]
    code += ['};', f'pub const IMAGE_SIZE: [u32; 2] = [{width}, {MAX_HEIGHT}];']
    (output / 'about_metadata.rs').write_text('\n'.join(code) + '\n', encoding='utf-8')
    receipt = {'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
               'source_bytes': source.stat().st_size, 'derived_size': [width, MAX_HEIGHT],
               'derived_bytes': portrait.stat().st_size, 'derived_sha256': hashlib.sha256(portrait.read_bytes()).hexdigest(),
               'metadata': metadata, 'version_authority': 'Cargo package version',
               'filter': 'nearest; lossless RGBA PNG; no runtime source decode'}
    (output / 'about_asset.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args()
    prepare(args.root.resolve(), args.output.resolve(), args.version)
    prepare_icon(args.root.resolve(), args.output.resolve())
