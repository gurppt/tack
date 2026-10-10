"""Fresh download must preserve the selected native build target."""
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import prepare_turbojpeg as native


class NativeBuildTarget(unittest.TestCase):
    def test_fresh_download_preserves_linux_and_windows_target(self):
        for target in (None, 'windows-gnu'):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                source = root / 'source' / ('libjpeg-turbo-' + native.VERSION)
                source.mkdir(parents=True)
                for name in ('LICENSE.md', 'README.ijg'):
                    (source / name).write_text('fixture notice')
                suffix = '-windows-gnu' if target else ''
                prefix = root / ('libjpeg-turbo-' + native.VERSION + suffix)
                (prefix / 'lib').mkdir(parents=True)
                (prefix / 'lib/libturbojpeg.a').write_bytes(b'library')
                (prefix / 'bin').mkdir()
                decoder = 'djpeg.exe' if target else 'djpeg'
                (prefix / 'bin' / decoder).write_bytes(b'decoder')
                build = root / ('turbojpeg-build' + suffix)
                build.mkdir()
                (build / 'CMakeCache.txt').write_text('CMAKE_C_COMPILER:FILEPATH=fake\n')
                payload = b'owned archive fixture'
                with patch.object(native, 'ARCHIVE_SHA256', hashlib.sha256(payload).hexdigest()), \
                     patch.object(native.urllib.request, 'urlopen', return_value=io.BytesIO(payload)), \
                     patch.object(native.shutil, 'which', side_effect=lambda name: '/fake/' + name), \
                     patch.object(native.subprocess, 'run') as run:
                    run.return_value.stdout = 'NASM fixture'
                    native.prepare(root, target)
                receipt = json.loads((prefix / 'native-build.json').read_text())
                self.assertEqual(receipt['target'], target)
                self.assertIn('-DREQUIRE_SIMD=ON', receipt['cmake_command'])
                self.assertEqual((root / 'windows-gnu-toolchain.cmake').exists(), bool(target))


if __name__ == '__main__':
    unittest.main()
