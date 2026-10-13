import unittest
from release_notes import extract

class Notes(unittest.TestCase):
    def test_only_exact_requested_version_is_published(self):
        text = "# Changelog\n\n## 0.1.0-dev.3 — bridge\n\nNew bridge.\n\n## 0.1.0-dev.2\n\nOld changes.\n"
        result = extract(text, "0.1.0-dev.3")
        self.assertIn("New bridge", result)
        self.assertNotIn("Old changes", result)
        for version in ["0.1.0-dev", "missing"]:
            with self.assertRaises(ValueError):extract(text,version)
        with self.assertRaises(ValueError):extract(text + "\n## 0.1.0-dev.3\nDuplicate\n", "0.1.0-dev.3")
