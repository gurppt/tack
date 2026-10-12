#!/usr/bin/env python3
"""Pre-fix source-contract witnesses, not physical Windows integration tests."""
import pathlib, sys, unittest
root = pathlib.Path(sys.argv.pop(1)) if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parents[2]
class Contracts(unittest.TestCase):
    def test_windows_picker_has_no_script_policy_dependency(self):
        source = (root/'crates/tack-app/src/native_files.rs').read_text()
        self.assertNotIn('tack-file-picker.ps1', source)
        self.assertNotIn('WINDOWS_PICKER', source)
    def test_import_reason_reaches_ui(self):
        source = (root/'crates/tack-app/src/product_window/local/updates.rs').read_text()
        self.assertNotIn('let _ = (path, message);', source)
        self.assertNotIn('rejected (unsupported, remote or unreadable files)', source)
unittest.main()
