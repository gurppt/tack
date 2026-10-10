"""Release identity and package checks reject mismatched or replaced candidates."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import package_release as release


class ReleaseContracts(unittest.TestCase):
    def identity(self, **changes):
        value = dict(schema=1, version='0.1.0-dev.1', channel='dev',
                     git_sha='a'*40, protocol_major=3)
        return dict(value, **changes)

    def fragments(self, root):
        paths = []
        for platform in ('linux-x86_64', 'windows-x86_64'):
            archive = root / ('tack-'+platform+'.zip')
            archive.write_bytes(platform.encode())
            data = dict(self.identity(), assets=[dict(platform=platform, name=archive.name,
                        size=archive.stat().st_size, sha256=release.digest(archive))])
            fragment = root / ('manifest-'+platform+'.json')
            fragment.write_text(json.dumps(data))
            paths.append(fragment)
        return paths

    def test_channel_identity_matches_rust_policy(self):
        release.validate_identity(self.identity())
        release.validate_identity(self.identity(version='0.1.0', channel='stable'))
        for changes in [dict(channel='stable'), dict(version='0.1.0-alpha.1', channel='stable'),
                        dict(version='0.1.0-dev.01'), dict(schema=9),
                        dict(protocol_major=4), dict(git_sha='not-a-commit')]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                release.validate_identity(self.identity(**changes))

    @patch.object(release.subprocess, 'check_output', return_value='a'*40)
    def test_merge_verifies_exact_artifacts_and_is_immutable(self, _):
        with tempfile.TemporaryDirectory(prefix='tack-release-test-') as temporary:
            root = Path(temporary)
            paths = self.fragments(root)
            merged = release.merge(paths, root)
            self.assertEqual(len(merged['assets']), 2)
            original = (root/'update-manifest.json').read_bytes()
            with self.assertRaises(ValueError):
                release.merge(paths, root)
            self.assertEqual((root/'update-manifest.json').read_bytes(), original)

    @patch.object(release.subprocess, 'check_output', return_value='a'*40)
    def test_wrong_hash_commit_protocol_and_extra_platform_are_refused(self, _):
        for change in ('bytes', 'commit', 'protocol', 'duplicate', 'path'):
            with self.subTest(change=change), tempfile.TemporaryDirectory(prefix='tack-release-test-') as temporary:
                root = Path(temporary)
                paths = self.fragments(root)
                record = json.loads(paths[1].read_text())
                if change == 'bytes':
                    (root/record['assets'][0]['name']).write_bytes(b'tampered')
                elif change == 'commit': record['git_sha'] = 'b'*40
                elif change == 'protocol': record['protocol_major'] = 4
                elif change == 'duplicate': record['assets'].append(record['assets'][0])
                elif change == 'path': record['assets'][0]['name'] = '../outside.zip'
                paths[1].write_text(json.dumps(record))
                with self.assertRaises(ValueError): release.merge(paths, root)
                self.assertFalse((root/'update-manifest.json').exists())


if __name__ == '__main__':
    unittest.main()
