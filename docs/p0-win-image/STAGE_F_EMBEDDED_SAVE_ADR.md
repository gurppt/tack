# Stage F — multi-GB Embedded save, measured storage gate

**Decision:** preserve container version 1 and its durability rules. Prepare a
separate reviewed incremental-storage mission; no format migration here.
Production base `dev 7f19e8c`. Only an owned-file example and experimental tools
are added; they are not linked into ordinary Tack. The prototype extension is
`.experiment`, magic `TACKEXP0`, and is never offered to a production `.tack`
reader/writer. Master brief §9 authorizes this study, not a schema rewrite.

## Current storage and ownership

`tack-storage/src/{save,reader,codec,ownership,recovery}.rs` implement:

- An 80-byte `TACKSN01` header, container 1, document schema 1–8, authoritative
  metadata/original directory and disposable overview directory. Directory
  entries are 64 bytes, with bounded offsets, lengths and CRCs. Open validates
  metadata/structure before lazy original access; `verify_originals` explicitly
  streams payload CRCs. It is not an eager full-image decode.
- `TackFile` pins an `Arc<File>` generation. `Payload::Stored` and `RangeReader`
  stream stable bounded ranges; replacing the filename cannot mix old/new bytes.
  Save copies every supplied original through a 128 KiB buffer, recomputes CRC,
  checks stored CRC or source file length/mtime, and writes new metadata/header.
- `BoardLease` holds a cooperative, stable sidecar lock and expected file stamp.
  It refuses external changes and concurrent writers. Lock sidecars are not
  unlinked during ordinary ownership: that could split locking across inodes.
- Save writes a sibling temporary, checks ownership, calls `sync_all`, then
  renames without deleting the prior target first. Unix parent-directory sync
  follows. A post-publication directory-sync error is distinct from a pre-rename
  failure. Windows open-handle/replace and AV effects still require real OS tests.
- Recovery is separate: owner/document marker, alternating snapshot slots,
  validated 232-byte manifest and durable atomic manifest publication. Existing
  recovery generations and unknown files are not disposable caches.

There is no safe metadata-only in-place fast path in the current layout: lengths,
directories, CRCs and recorded file size are part of the published snapshot.
Skipping unchanged autosave scheduling can avoid a redundant request; it cannot
make an actual edited snapshot cheap. Do not weaken checksums/fsync/ownership.

## Real encoded corpus and measurement

Tools: `make_save_experiment.py`, `run_save_experiment.py`,
`embedded_generation_experiment.py`; example `embedded_save_probe.rs`.
These **storage-only** experiments deliberately bypass image admission/rendering.
The two largest PNGs exceed Stage D support. They are real noninterlaced L8 PNGs,
generated row by row with deterministic noise and DEFLATE level 0, not padding or
sparse-file substitutes. Streaming validation checks all PNG chunk CRCs and
decompression EOF/byte count without constructing a full canvas.

| Approximate target / actual PNG | Dimensions | Encoded bytes | SHA-256 |
|---|---|---:|---|
| 50 MiB (~50.03 MiB) | 8000×6554 | 52,461,635 | 24e3107f27848a3eb206c48a1b61efe7b734e36d6e5429f24b8c408a23003c0f |
| 500 MiB (~500.23 MiB) | 16000×32768 | 524,527,755 | 1eae248deee4f50dc63f1875fa64af62c6512f99b976b0cc5bd8d20d1213fb39 |
| 1.5 decimal GB (~1.398 GiB) | 30000×50000 | 1,500,582,741 | 4a043b6f4ff1962c3281225bd267ea38eaf1fe8061ba13c5cfc49ca375dfe40f |

Linux local SSD, warm/uncontrolled page cache, one sample per operation. A
two-job workspace compilation ran concurrently; fsync/storage noise is visible.
These are independent observations, **not a speedup ratio, statistical baseline,
Windows/Defender/SMB evidence or physical cold-drive measurements**. No global
cache eviction touched user files. Save time excludes the subsequent verification
passes; externally sampled `/proc/io` includes those CRC/SHA passes.

| Save milliseconds | First | Unchanged | 1-pixel transform edit | Second unchanged |
|---|---:|---:|---:|---:|
| 50 MiB | 148.87 | 167.63 | 158.84 | 159.89 |
| 500 MiB | 1149.11 | 1206.86 | 1178.02 | 1174.19 |
| 1.5 GB | 5542.63 | 3107.09 | 3446.96 | 11262.29 |

All 12 completed saves reopen with equal document, original CRC and source
SHA-256. Peak sampled standalone PSS ~1.13–1.20 MiB, 20 ms sampling; not app/GPU
memory. Each save writes source size +369 bytes; even unchanged saves remain
O(original bytes). Reopen metadata reads just 369 bytes for this one-image board.

Six real child-process terminations skip Rust destructors: one after metadata
write and one after file sync, for each size, before rename. Every old authority
file remains byte-identical by SHA-256; the previous completed generation had
already passed reopen/CRC checks. Owned temporary generations remain (one full
payload each) and are explicitly retired only after that proof. These do not
simulate disk power loss, post-rename kill, AV, denied access or full filesystem.
Existing storage/recovery tests cover additional boundaries; physical Windows
fault tests remain a prerequisite for migration.

## Single-file immutable-blob microprototype

The Python prototype copies one original with CRC, syncs it, then commits small
metadata using a record hash and footer. Metadata is synced **before** footer;
footer is synced before acknowledging. Recovery reads at most a 128 KiB tail
plus a bounded candidate record (metadata ≤64 KiB), never scans the GB original.
Whole-blob verification is a separate bounded CRC pass. Appended incomplete
records and partial footers recover the prior valid metadata generation in all
six injected cases. No real OS/process power-loss guarantee follows from this.

| Prototype, one initial copy / three metadata commits | First copy+commit ms | Metadata ms range |
|---|---:|---:|
| 50 MiB | 151.46 | 4.38–6.14 |
| 500 MiB | 1123.18 | 3.26–8.16 |
| 1.5 GB | 2969.45 | 3.19–6.08 |

Three committed records add 137 bytes each in this tiny document; timed recovery
reads 274/411/548 tail bytes. This demonstrates O(metadata) writes after a blob
exists, **not** production crash/locking/schema acceptance. The toy header is
not a validated production index; it has no compaction, concurrent writer,
corrupt-pointer hardening, migration or Windows replacement contract. Losing
more than the bounded permitted tail would need an explicitly designed checkpoint.

## Options and bounded follow-up: STORAGE-GENERATIONS-01

| Option | Cost and benefit | Recovery/compatibility risk |
|---|---|---|
| Current whole-container replacement | Small, proven API; bounded RAM, standalone portable file; source-sized writes and temporary disk on every actual save | Retain as compatibility/reference path. A no-change early exit must prove document/assets/derived policy equality, not skip ownership or dirty edits. |
| Immutable blobs + appended metadata generations in one file | New blobs streamed once; normal edits O(metadata); portable file; grows with edits/deletions | Transactional footer/checkpoints, bounded orphan recovery, original CRC identity, exclusive lease, pinned old generations, safe compaction and new version are required. Prototype favors studying this option. |
| Transactional copy-on-write index/pages | Small changed-page writes; fine-grained reuse; can compact incrementally | Highest implementation/verification cost: page allocator, roots, free-space reachability, torn writes, corruption and old-reader semantics. Do not add this machinery without evidence it outperforms simpler generations. |

Follow-up must specify container-2 capability/version rejection by old readers,
read-only backward loading of v1, explicit Save As migration, and rollback/export
to v1 before conversion. Never silently modify the only valid board. Define
original identity, changed-source publication, blob CRC and durable index roots,
64-bit range checks, cooperative locks, directory/file sync, disk quota, compaction
and retained recovery lifecycle first. Test killed writers at every commit step,
corrupt/truncated tails and indexes, disk-full, foreign changes, concurrent opens,
and physical Win10 locks/Defender/rename. Benchmark cold/warm and ordinary boards
against v1 before choosing a schema. No mandatory permanent external asset folder.

Experiment disk budget explicitly permits ~8 GiB peak for the required GB corpus,
keeping a 10 GiB reserve; compact receipts/seeds survive end-of-session cleanup.
This is an exception to the preferred 512 MiB fixture budget, not a new cache cap.
Data/user files and production storage remain unchanged. Rollback removes only
these tools/example/docs; no `.tack` migration exists to undo.
