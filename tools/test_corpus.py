import hashlib
from pathlib import Path
import tempfile
import unittest

from PIL import Image
from generate_corpus import generate


class CorpusTests(unittest.TestCase):
    def test_recreation_and_geometry(self):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            first = generate(a, count=7, sources=2, width=120, height=90)
            second = generate(b, count=7, sources=2, width=120, height=90)
            self.assertEqual(first, second)
            self.assertEqual(len(first["objects"]), 7)
            self.assertEqual(len({o["path"] for o in first["objects"]}), 7)
            for obj in first["objects"]:
                path = Path(a) / obj["path"]
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), obj["source_sha256"])
                with Image.open(path) as image:
                    self.assertEqual(image.size, (120, 90))
                    self.assertEqual(image.format, "JPEG")

    def test_existing_data_and_invalid_sizes_are_preserved(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "keep"
            path.write_text("user data")
            with self.assertRaises(ValueError):
                generate(folder, count=2, sources=1)
            self.assertEqual(path.read_text(), "user data")
        with self.assertRaises(ValueError):
            generate("unused", count=0)


if __name__ == "__main__":
    unittest.main()
