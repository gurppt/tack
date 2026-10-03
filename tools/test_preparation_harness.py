import tempfile
from pathlib import Path
import unittest
from run_preparation import concurrency, inventory


class PreparationEvidenceTests(unittest.TestCase):
    def test_decode_overlap_counts_errors_and_endpoints_on_one_clock(self):
        profiles = [dict(decode_started_ms=a, decode_finished_ms=b, failed=failed)
                    for a, b, failed in [(0, 10, False), (5, 15, True), (10, 11, False)]]
        self.assertEqual(concurrency(profiles), 2)
        self.assertEqual(concurrency([dict(decode_started_ms=None)]), 0)

    def test_input_inventory_distinguishes_overview_refinement_and_unpublished_file(self):
        with tempfile.TemporaryDirectory() as temp:
            cache = Path(temp)
            (cache / "0-hash-128.png").write_bytes(b"overview")
            (cache / "0-hash-512.png").write_bytes(b"refinement")
            (cache / "1-hash-128.tmp").write_bytes(b"unpublished")
            state = inventory(cache)
            self.assertEqual(state["files"], 2)
            self.assertEqual(state["overview_files"], 1)
            self.assertEqual(state["bytes"], 18)
            self.assertEqual(state["temporary_files"], 1)
            self.assertEqual(state["file_bytes"]["total_bytes"], 18)
            fingerprint = state["content_sha256"]
            (cache / "0-hash-128.png").write_bytes(b"corrupt!")
            self.assertNotEqual(inventory(cache)["content_sha256"], fingerprint)


if __name__ == "__main__":
    unittest.main()
