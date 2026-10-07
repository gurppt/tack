"""About build contract: owner data, bounded pixels and reproducible output."""
import hashlib
import tempfile
import unittest
from pathlib import Path
from PIL import Image
import prepare_about as about


class AboutPreparation(unittest.TestCase):
    def test_rejects_stale_version_invalid_schema_and_unreadable_text(self):
        for data in ({'name': 'Tack', 'version': '99'},
                     {'name': 'Tack', 'unknown': 'field'},
                     {'name': 'Tack', 'author': 'line\nbreak'},
                     {'name': 'Tack', 'author': 12},
                     {'name': ''},
                     {key: 'x' * 96 for key in about.FIELDS if key != 'version'}):
            with self.subTest(data=data), self.assertRaises(ValueError):
                about.validate(data, '0.0.1')
        self.assertEqual(about.validate({'name': 'Tack'}, '0.0.1')['version'], '0.0.1')

    def test_owner_asset_is_untouched_and_output_is_reproducible_nearest(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'gfx').mkdir()
            metadata = 'name = "Tack"\nauthor = "Captain Cool - Suspicious Sausage Records"\nwebsite = "https://github.com/gurppt/tack"\n'
            (root / 'gfx/about.toml').write_text(metadata)
            original = Image.new('RGBA', (587, 635), (10, 20, 30, 255))
            original.paste((240, 220, 200, 255), (300, 0, 587, 635))
            source = root / 'gfx/tack_about.png'
            original.save(source)
            initial = source.read_bytes()
            receipt = about.prepare(root, root / 'one', '0.0.1')
            about.prepare(root, root / 'two', '0.0.1')
            self.assertEqual(initial, source.read_bytes())
            for file in ('about.png', 'about_metadata.rs', 'about_asset.json'):
                self.assertEqual((root / 'one' / file).read_bytes(), (root / 'two' / file).read_bytes())
            with Image.open(root / 'one/about.png') as derived:
                self.assertEqual(derived.size, (207, 224))
                self.assertEqual(set(derived.getdata()), {(10, 20, 30, 255), (240, 220, 200, 255)})
            self.assertEqual(receipt['source_sha256'], hashlib.sha256(initial).hexdigest())
            self.assertLess(receipt['derived_bytes'], about.MAX_PNG_BYTES)
            self.assertIn('Captain Cool', (root / 'one/about_metadata.rs').read_text())

    def test_word_wrap_keeps_author_readable_and_long_urls_complete(self):
        self.assertEqual(about.wrapped('Author: Captain Cool - Suspicious Sausage Records'),
                         ['Author: Captain', 'Cool - Suspicious', 'Sausage Records'])
        url = 'https://github.com/gurppt/tack'
        self.assertEqual(''.join(about.wrapped(url)), url)
        self.assertTrue(all(sum(1 if ord(c) < 128 else 2 for c in row) <= 18
                            for row in about.wrapped('é' * 24)))

    def test_missing_or_corrupt_source_fails_preparation(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'gfx').mkdir()
            (root / 'gfx/about.toml').write_text('name = "Tack"')
            with self.assertRaises(FileNotFoundError):
                about.prepare(root, root / 'out', '0.0.1')
            (root / 'gfx/tack_about.png').write_bytes(b'not a png')
            with self.assertRaises(OSError):
                about.prepare(root, root / 'out', '0.0.1')
