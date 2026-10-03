# Phase 1B storage decision

2026-10-03, recorded before the production reader/writer. Starting commit
`c03d24b`; only unrelated `gfx/Untitled.png` is untracked. Required normative
briefs, reports, compatibility contract and context documents were read.
No authority conflict was found. Stop after 1B.

## Candidates and evidence

SQLite metadata + incremental BLOBs offers mature transaction/recovery machinery
and cheap metadata queries. Its individual BLOB limit is under 2 GiB, so large
sources require a chunk table and streaming adapter. Native SQLite deployment,
page-level corruption coupling and journal management are additional work.
See [limits](https://www.sqlite.org/limits.html),
[incremental access](https://www.sqlite.org/c3ref/blob_open.html) and
[atomic commit](https://www.sqlite.org/atomiccommit.html).

An indexed snapshot container offers independent metadata and original/preview
byte ranges, no decompression of originals on open, bounded streaming and local
checksums. It requires our own small validated directory/parser and transactional
temporary-file replacement. Whole-file replacement copies embedded bytes on save;
future incremental storage is not claimed. This is acceptable for the minimal
snapshot slice but must remain visible for very large boards/autosave planning.

A Python layout spike (1k rows, 49,152,000 dummy preview bytes, warm page cache)
measured SQLite write/open-metadata 219.3/2.56 ms, 49,672,192 bytes; indexed
40.7/0.109 ms, 49,200,008 bytes. The indexed metadata is dummy fixed records,
not equivalent domain parsing. These numbers establish feasibility, not a
production superiority claim; no fsync or crash tests were included.
Raw results: ignored `benchmark-results/phase1b-spike/results.json`.

**Select an indexed snapshot**: explicit little-endian versioned metadata,
original and overview directories before payloads; streamed uncompressed original
ranges; independent overview checksums. No ZIP extraction, database framework,
serde serialization of domain structs, source-sized encoded payload bank or
file I/O in renderer. Refuse unsupported authoritative schema/record values
before constructing an editable document; never silently discard newer work.

## Dependencies and decode boundary

Promote existing locked `getrandom =0.3.4` and `crc32fast =1.5.2` to direct
dependencies in storage. Neither adds a new transitive package to this workspace.
Core stays dependency-free. getrandom uses platform OS entropy for nonzero 128-bit
IDs; no time/path/slot identity, no network. CRC32 detects accidental corruption,
not authenticity or deliberate tampering. Structural validation remains mandatory.
Both are MIT/Apache-2.0; inspected upstream manifests/release histories:
[getrandom](https://github.com/rust-random/getrandom),
[history](https://github.com/rust-random/getrandom/blob/master/CHANGELOG.md),
[crc32fast](https://github.com/srijs/rust-crc32fast).
Use pinned existing versions to avoid incidental workspace upgrades. Security
history is checked through the configured RustSec cargo-deny gate, not inferred
from repository popularity. No native storage dependency is added.

The existing turbojpeg thumbnail boundary requires a complete encoded slice.
For embedded repair, ranged/streaming input is available and must be used;
reuse the existing scaled jpeg-decoder path + thumbnail resize/PNG encoding,
with a separately identified generator. Linked JPEG import may reuse the measured
native 128 tier within its existing 64 MiB cap. No second thumbnail role or LOD
pipeline is created. PNG input stays within explicit dimension/allocation caps.

## Implementation plan and invariants

1. Extend the existing Source with lossless platform path descriptors, revision
   and cheap size/mtime fingerprint; add reversible source replacement command.
   Track conservative dirty state through explicit editor operations, including undo/redo.
2. Add tack-storage: bounded explicit codec, directories, document reconstruction
   through commands, ranged source access, independent disposable previews,
   OS-random typed IDs, snapshot save to exclusive sibling temp + flush + rename.
   Failures preserve previous target; abandoned temps are never auto-promoted.
3. Reuse asset thumbnail/decode machinery behind reader adapters. Async product
   loading uses bounded workers/results and a capped representation cache; valid
   persisted overviews bypass sources. Missing/changed links retain explicit state.
4. Adapt the proven GPU quads for transform/crop/alpha/filtering and typed product
   representation keys. Keep source/container semantics outside renderer. Minimal
   CLI create/inspect/open/repair path; no manipulation/UI/backlog expansion.
5. Test corruption, malformed counts/ranges/records, unknown versions, IDs,
   lossless foreign/native paths, source revisions, streaming embedded reads,
   dirty state, interruptions/failed replacement, derived repair and GPU readback.
6. Measure 1k first preparation/reopen/missing/repair, embedded fixture,
   1k/5k/10k metadata/query and existing six renderer regression traces. Run all
   quality/security gates; independent persistence/security review and fixes.
   Document actual process-crash guarantees; do not claim tested power-loss or
   Windows behavior. Report, commit/push per user instruction, STOP before 1C.

## Hardened outcome

The selected runtime is now documented in [format v1](tack_file_format_v1.md).
Linux tests cover reader mutations, 20 GiB sparse metadata opening, independent
ranged cursors, derived corruption, source paths/revisions, private permissions,
failed copies and four real process-kill stages. Preview CRC is checked on demand;
original CRC remains deferred until save-copy, including partial-decode limitations.
File sync + parent sync are implemented, without claiming tested power loss.
Post-publication directory-sync failure has a distinct structured error.

Measured final 1k linked creation charges 35.4 s preparation and
141 ms snapshot save; the 47,215,836-byte file includes
44.65 MiB overviews and
0.835% nonpayload overhead.
Prepared ordinary reopen reaches recognizable content in 684 ms
including 578 ms native/GPU initialization, with
4.8 ms metadata parsing and zero source-byte reads.
Full tour reuses all 1000 previews. Whole-copy/space and 512 MiB preparation quota
limits remain visible. See [the mission report](../MISSION_1B_REPORT.md) and its
bound machine evidence. No rejected candidate code/dependency remains in runtime.
