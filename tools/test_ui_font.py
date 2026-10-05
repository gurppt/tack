"""Verify shipped bitmap bytes reproduce from the pinned upstream BDF."""
import hashlib
import json
from pathlib import Path
import unittest
from build_ui_font import convert


class UiFontTests(unittest.TestCase):
    def test_pinned_bitmap_reproduction(self):
        root = Path(__file__).resolve().parent.parent / 'assets/ui-font'
        packed = convert(root)
        self.assertEqual(packed, (root / 'spleen-glyphs.bin').read_bytes())
        self.assertEqual(len(packed), 1001 * 37)
        self.assertEqual(hashlib.sha256(packed).hexdigest(),
                         '6c083bb6db7086dbb167e50bafb3b93f5eca3077e8384af6f0b7a20602f3f837')
        self.assertEqual(json.loads((root / 'provenance.json').read_text())['license'], 'BSD-2-Clause')


if __name__ == '__main__':
    unittest.main()
