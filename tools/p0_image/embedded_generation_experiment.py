#!/usr/bin/env python3
"""Non-.tack append-generation microprototype. No production reader/writer uses it.

Not a proposed schema: demonstrates copy-once cost and bounded-tail crash recovery.
CRC validates immutable bytes; SHA links each small metadata commit. One writer only.
"""
import hashlib
import json
import os
from pathlib import Path
import struct
import sys
import time
import zlib

MARKER = "P0 F disposable storage experiment; never a user board\n"
HEADER = struct.Struct("<8sQI44x")
RECORD = struct.Struct("<8sQI")
FOOTER = struct.Struct("<8sQI32s")
MAX_METADATA = 65536
TAIL_LIMIT = 128 * 1024


def recover(path):
    with path.open("rb") as handle:
        magic, blob_bytes, crc = HEADER.unpack(handle.read(HEADER.size))
        if magic != b"TACKEXP0":
            raise ValueError("experiment magic")
        size = path.stat().st_size
        start = max(HEADER.size + blob_bytes, size - TAIL_LIMIT)
        handle.seek(start)
        tail = handle.read(TAIL_LIMIT)
        end = len(tail)
        while (offset := tail.rfind(b"TCOMMIT0", 0, end)) >= 0:
            end = offset
            if offset + FOOTER.size > len(tail):
                continue
            _, record_offset, count, sha = FOOTER.unpack(tail[offset:offset + FOOTER.size])
            if not 0 < count <= MAX_METADATA:
                continue
            footer_offset = start + offset
            if record_offset < HEADER.size + blob_bytes or record_offset + RECORD.size + count != footer_offset:
                continue
            handle.seek(record_offset)
            rec = handle.read(RECORD.size)
            if len(rec) != RECORD.size:
                continue
            rec_magic, sequence, rec_len = RECORD.unpack(rec)
            if rec_magic != b"TGENEXP0" or rec_len != count:
                continue
            metadata = handle.read(count)
            if hashlib.sha256(rec + metadata).digest() != sha:
                continue
            return dict(sequence=sequence, metadata=json.loads(metadata), commit_end=footer_offset + FOOTER.size,
                        blob_bytes=blob_bytes, blob_crc=crc, tail_read_bytes=len(tail),
                        scope="bounded tail plus candidate record; not full original verification")
    raise ValueError("no valid experimental generation in bounded tail")


def append(path, sequence, metadata, cut=None):
    data = json.dumps(metadata, sort_keys=True).encode()
    if not 0 < len(data) <= MAX_METADATA:
        raise ValueError("metadata budget")
    with path.open("ab") as handle:
        offset = handle.tell()
        record = RECORD.pack(b"TGENEXP0", sequence, len(data)) + data
        handle.write(record)
        handle.flush()
        os.fsync(handle.fileno())
        if cut == "before-footer":
            return
        footer = FOOTER.pack(b"TCOMMIT0", offset, len(data), hashlib.sha256(record).digest())
        handle.write(footer[:7] if cut == "partial-footer" else footer)
        handle.flush()
        os.fsync(handle.fileno())


def run(root):
    if (root / "OWNER_SAVE_EXPERIMENT.txt").read_text() != MARKER:
        raise RuntimeError("refusing unowned experiment directory")
    source = root / "source.png"
    path = root / "single-file-generation.experiment"
    started = time.perf_counter()
    crc = 0
    with source.open("rb") as handle, path.open("xb") as output:
        output.write(HEADER.pack(b"TACKEXP0", source.stat().st_size, 0))
        while block := handle.read(128 * 1024):
            output.write(block)
            crc = zlib.crc32(block, crc)
        output.seek(0)
        output.write(HEADER.pack(b"TACKEXP0", source.stat().st_size, crc))
        output.flush()
        os.fsync(output.fileno())
    append(path, 1, dict(position=[0, 0], original="immutable", experiment=True))
    # Creation needs parent sync too; appends do not create another directory entry.
    if os.name == "posix":
        fd = os.open(root, os.O_RDONLY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    first_ms = (time.perf_counter() - started) * 1000
    timings = []
    for sequence, position in [(2, [0, 0]), (3, [1, 0]), (4, [1, 0])]:
        started = time.perf_counter()
        append(path, sequence, dict(position=position, original="immutable", experiment=True))
        result = recover(path)
        if result["sequence"] != sequence:
            raise RuntimeError("lost experimental commit")
        timings.append(dict(sequence=sequence, ms=(time.perf_counter() - started) * 1000,
                            tail_read_bytes=result["tail_read_bytes"]))
    stable = recover(path)
    faults = []
    for cut in ["before-footer", "partial-footer"]:
        append(path, 5, dict(position=[2, 0], experiment=True), cut=cut)
        result = recover(path)
        if any(result[key] != stable[key] for key in ["sequence", "metadata", "commit_end", "blob_bytes", "blob_crc"]):
            raise RuntimeError("incomplete tail replaced previous validated metadata")
        faults.append(dict(cut=cut, recovered_sequence=result["sequence"],
                          unchanged_metadata=True, process_power_loss_simulation=False))
        # Only this owned experimental file: discard incomplete experimental tail.
        with path.open("r+b") as output:
            output.truncate(stable["commit_end"])
            output.flush()
            os.fsync(output.fileno())
    # Verify complete blob with bounded buffers, outside timed metadata commits.
    actual = 0
    remaining = source.stat().st_size
    with path.open("rb") as handle:
        handle.seek(HEADER.size)
        while remaining:
            block = handle.read(min(128 * 1024, remaining))
            if not block:
                raise RuntimeError("short experimental blob")
            actual = zlib.crc32(block, actual)
            remaining -= len(block)
    if actual != crc:
        raise RuntimeError("experimental immutable blob CRC mismatch")
    receipt = dict(source_bytes=source.stat().st_size, first_copy_commit_ms=first_ms,
                   metadata_commits=timings, incomplete_tail_recovery=faults,
                   original_crc_verified=True, prototype_bytes=path.stat().st_size,
                   scope="single process Python prototype; not .tack; no OS power-loss or Windows proof")
    (root / "generation-prototype.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt), flush=True)


if __name__ == "__main__":
    run(Path(sys.argv[1]).resolve())
