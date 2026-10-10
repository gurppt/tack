#!/usr/bin/env python3
"""Install missing starter icons without replacing artist-owned files."""
import argparse
from pathlib import Path
import shutil


def install_missing(source: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    for icon in sorted(source.glob('*.png')):
        try:
            output = (destination / icon.name).open('xb')
        except FileExistsError:
            continue
        with output, icon.open('rb') as original:
            shutil.copyfileobj(original, output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    install_missing(args.source, args.destination)
