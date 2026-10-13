#!/usr/bin/env python3
"""Extract only the requested immutable version section; no publishing side effect."""
from pathlib import Path
import re
import sys

def extract(text, version):
    matches = list(re.finditer(r"^## (\S+)(?:[^\n]*)\n", text, re.M))
    selected = [i for i, match in enumerate(matches) if match[1] == version]
    if len(selected) != 1:
        raise ValueError("Exactly one release-note section required for " + version)
    index = selected[0]
    end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
    section = text[matches[index].start():end].strip() + "\n"
    if not section.split("\n", 1)[1].strip():
        raise ValueError("Release notes must describe the selected version")
    return section

if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    Path(sys.argv[2]).write_text(extract((root / "CHANGELOG.md").read_text(), sys.argv[1]))
