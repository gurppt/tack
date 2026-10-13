#!/usr/bin/env python3
"""Run owned storage-only save benchmark and actual pre-rename process exits."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from make_save_experiment import MARKER


def digest(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def process_info(pid):
    try:
        values = Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines()
        pss = next(int(line.split()[1]) * 1024 for line in values if line.startswith("Pss:"))
        io = dict(line.split(": ", 1) for line in Path(f"/proc/{pid}/io").read_text().splitlines())
        return pss, {key: int(value) for key, value in io.items()}
    except (OSError, StopIteration):
        return 0, {}


def run(root, binary):
    if (root / "OWNER_SAVE_EXPERIMENT.txt").read_text() != MARKER:
        raise RuntimeError("refusing unowned fixtures")
    receipts = []
    for case in json.loads((root / "fixtures.json").read_text()):
        directory = root / case["case"]
        if (directory / "OWNER_SAVE_EXPERIMENT.txt").read_text() != MARKER:
            raise RuntimeError("unowned case")
        board = directory / "storage-only.tack"
        operations = []
        for operation in ["first", "unchanged", "minor", "second", "kill-metadata", "kill-synced"]:
            before = digest(board) if operation.startswith("kill-") else None
            started = time.perf_counter()
            child = subprocess.Popen([str(binary), str(directory), operation], stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True)
            peak = 0
            last_io = {}
            while child.poll() is None:
                pss, io = process_info(child.pid)
                peak = max(peak, pss)
                last_io = io or last_io
                time.sleep(0.02)
            stdout, stderr = child.communicate(timeout=5)
            entry = dict(operation=operation, exit=child.returncode, pss_peak_sampled=peak,
                         wall_ms=(time.perf_counter() - started) * 1000, last_sample_io=last_io,
                         stderr=stderr, sample_ms=20)
            if operation.startswith("kill-"):
                entry["previous_authority_sha256"] = before
                entry["authority_unchanged"] = digest(board) == before
                if child.returncode != 86 or not entry["authority_unchanged"]:
                    raise RuntimeError("interrupted save changed authority")
                temps = list(directory.glob("storage-only.tack.tack-tmp-*"))
                entry["owned_temporary_bytes"] = sum(path.stat().st_size for path in temps)
                entry["owned_temporary_count"] = len(temps)
                # Own crash-test temps only, after byte-identical authority was proved.
                for path in temps:
                    if path.is_symlink() or not path.is_file():
                        raise RuntimeError("unsafe crash-test temporary")
                    path.unlink()
            else:
                if child.returncode:
                    raise RuntimeError(f"save experiment failed: {stderr}")
                entry.update(json.loads(stdout))
                if entry["original_sha256"] != case["sha256"]:
                    raise RuntimeError("source hash mismatch")
            operations.append(entry)
            print(json.dumps(dict(case=case["case"], **entry)), flush=True)
            (directory / "save-receipt.json").write_text(json.dumps(operations, indent=2) + "\n")
        subprocess.run([sys.executable, str(Path(__file__).with_name("embedded_generation_experiment.py")),
                        str(directory)], check=True)
        receipts.append(dict(fixture=case, operations=operations,
                             prototype=json.loads((directory / "generation-prototype.json").read_text())))
    (root.parent / "storage-comparison.json").write_text(json.dumps(dict(
        scope="Linux local SSD; warm/uncontrolled page cache; no global drop_caches; no Windows/Defender/SMB proof",
        fixtures=receipts), indent=2) + "\n")


if __name__ == "__main__":
    run(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
