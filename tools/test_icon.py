"""Build-only logo provenance, small legibility and deterministic package outputs."""
import tempfile
import unittest
from pathlib import Path
from PIL import Image
import prepare_icon

class IconPreparation(unittest.TestCase):
    def test_source_to_package_is_reproducible_and_small_sizes_retain_both_colors(self):
        root=Path(__file__).resolve().parent.parent
        source=(root/'gfx/Rhombus--Streamline-Fluent-Ui-Filled.svg').read_bytes()
        with tempfile.TemporaryDirectory() as temp:
            out=Path(temp)
            first=prepare_icon.prepare(root,out/'one')
            second=prepare_icon.prepare(root,out/'two')
            self.assertEqual(first,second)
            for path in (out/'one').iterdir():
                self.assertEqual(path.read_bytes(),(out/'two'/path.name).read_bytes())
            self.assertEqual((out/'one/icon.rgba').stat().st_size,4096)
            for size in (16,32):
                with Image.open(out/'one'/f'tack-icon-{size}.png') as image:
                    colors=list(image.getdata())
                    self.assertTrue(any(r>200 and b>80 and g<90 and a>200 for r,g,b,a in colors))
                    self.assertTrue(any(g>170 and b>140 and r<110 and a>200 for r,g,b,a in colors))
                    self.assertEqual(image.size,(size,size))
        self.assertEqual(source,(root/'gfx/Rhombus--Streamline-Fluent-Ui-Filled.svg').read_bytes())
