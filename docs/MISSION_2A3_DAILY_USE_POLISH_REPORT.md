# Phase 2A3 — daily-use polish and desktop shared-board entry

Local technical checks **PASS**; exact-source Linux/Windows CI pending publication.
Baseline: Phase 2A2 implementation `873ce97cb57dc525441feebb94156073b3ebe87b`.
The original brief and its desktop-entry addendum are covered. Measurements and
raw-receipt hashes are in [phase2a3.json](measurements/phase2a3.json); independent
review: [phase2a3_daily_use.md](reviews/phase2a3_daily_use.md).

## Delivered behavior

- **View → Add bookmark / Camera bookmarks** saves exact center/zoom, jumps
  immediately, renames through F2 or a visible button, and deletes through Delete
  or a visible button. At most 64 views, 128 UTF-8 bytes per name, unique stable
  IDs and checked finite camera values. No canvas object, thumbnail or permanent
  panel. Add/rename/delete use the existing reversible history; jump changes only
  the camera. Frames remain separate.
- **Edit → Duplicate / Ctrl+D** copies selected images, notes, all current
  annotations and frames in one atomic Undo transaction, with one adaptive grid
  step offset. Images retain asset/source identity and crop/filter/opacity;
  object/group IDs are fresh. New group relations contain only copied image
  members. Original bytes and derived products are reused.
- **Image → Source → Image information** displays dimensions, linked/embedded
  storage, already-observed status, known encoded size, sampling, opacity, crop,
  revision and linked path. Missing/foreign/unobserved sources and unrecorded
  sizes are explicit. No filesystem read, metadata probing or decode admission.
- **File → Join shared board** accepts numeric `IP:PORT BOARD_ID` or
  `tack://IP:PORT/BOARD_ID`; IPv6 uses brackets. Validation precedes work. An
  explicit cancellable operation launches the same CLI `join` backend/native
  canvas in another window, with connecting/failed/joined feedback. The local
  board stays open and unchanged. Escape kills and reaps a pending child;
  cancelled late results cannot populate a later form. A temporary readiness
  receipt is written by the existing worker, only after successful admission.
  No local startup connection, DNS lookup, polling daemon or permanent worker.
- **File → Copy shared address** uses the existing native clipboard helper on
  the operation worker and emits a canonical URI. Local windows disable it.
- **Static identity:** supplied rhombus SVG preserved verbatim; separate
  fuchsia/turquoise italic-t adaptation follows the brief. Existing build-only
  Pillow derives PNG16/32/64/128/256, ICO, window RGBA and About bitmap. No runtime
  SVG dependency/decoder. The human package includes a Linux desktop entry.
  Windows has a window icon and distributed ICO; no executable-resource icon
  integration or native Windows runtime acceptance is asserted.

## Persistence and authority

Schema 4 appends bounded versioned bookmark records to the existing metadata;
container format 1 and schemas 1–3 remain unchanged/readable. The writer uses
schema 4 only when bookmarks exist. Older readers explicitly refuse schema 4;
unknown versions, duplicate IDs, invalid names/values, truncation and trailing
bytes fail before document admission. Exact f64 values survive save/reopen.
See the [compatibility contract](design/tack_document_compatibility.md).

Bookmark mutation and Duplicate are explicitly local-only in this slice, both
in menu guards and handlers. Shared snapshots can contain bookmarks and allow
camera jumps. No new replication command or separate history is introduced.
`tack-server` gains no renderer/UI/image codec dependency; its changed binary
reflects the common document/storage schema support.

Desktop **Publish** is deferred: the existing CLI consumes a committed file;
publishing current unsaved state needs a bounded snapshot/original-ownership
lifecycle beyond this entry-point pass. **Shared recents** are deferred because
existing recent records represent native file paths, not connection descriptors.
Existing explicit CLI publication and reconnection semantics remain available.

## Cost and verification

Serial native Linux/X11 800×600, RTX 2060, two paired runs with warm/unspecified
OS page cache. Startup includes GPU initialization; RSS includes the driver.

| Board | 2A2 startup median | 2A3 startup median | RSS median change |
| --- | ---: | ---: | ---: |
| Empty | 452.35 ms | 453.92 ms | +0.080 MiB |
| 1,000 objects | 450.48 ms | 467.09 ms | +0.168 MiB |
| 64 bookmarks, empty canvas | old schema refused | 439.28 ms | +0.068 MiB versus current empty |

The 1,000-object startup median is +3.7%; individual deltas have opposite signs,
and most variation is GPU setup. Outside GPU setup, median startup changes from
18.88 to 21.33 ms. Two pairs are a regression witness, not statistical proof of
zero cost. All ten idle intervals have **41 threads, zero IP sockets, zero I/O
and zero redraws**. CPU remains 1–2 scheduler ticks per three seconds, including
driver activity. Image CPU/GPU payload is unchanged: 3,194,880 bytes for the
1,000-object board, zero for empty/bookmark-only boards.

- Stripped client: **19,450,176 bytes**, +120,312 bytes (**+0.62%**) over 2A2.
  SHA-256 `3ad64ecb0533b383d47e36884785c36f065f774df69e478201a7ca3e064a8893`.
  Cargo.lock/dependency declarations unchanged; decoder/About assets unchanged.
- 64 bookmarks: **3,359-byte file**, metadata commit 0.009 ms in the diagnostic.
- Duplicate 1,000 objects: 1.54 ms construction + 0.77 ms commit;
  10,000: 16.10 + 9.06 ms, 1.68 MiB history. One source/asset throughout;
  zero original bytes read. This diagnostic uses synthetic 900 GiB encoded-size
  metadata, not a fabricated claim of processing a 900 GiB image. Independent
  save/reopen tests verify a real single embedded blob is retained once.
- Full gate: fmt/check/Clippy/tests/docs, **334 Rust tests**, **27 Python tests**,
  cargo-deny advisories/bans/licenses/sources pass. Five explicit GPU suites:
  **14 tests pass**, including 12,000-upload eviction/reentry churn.
- **20 daily native checks**: exact saved jump, rename/delete Undo, duplicate
  Undo/Redo/save/reopen, actual selected-image info without decode, malformed and
  refused join, cancel/reopen, valid IP+ID join, canonical URI clipboard and
  shared Duplicate refusal. **41 About checks** include theme/scale layouts,
  repeated resource retirement, and missing/corrupt/oversized optional artwork.
- Inherited huge-raster checks: **33 settled poses** across 50k×50k JPEG normal,
  potato and mixed JPEG/PNG; exact return/reopen performs zero source/codec work
  and reuses all six hot tiles. No idle I/O/redraws; bounded payloads retained.
- LOD: **2,520 wheel transitions, 32 pans, five filtering edits**, save/reopen.
  Existing collaboration baseline: **32 native checks**, three clients/server,
  authority/conflict/Undo/CAS/rejoin and alongside-local isolation pass.

Three independent findings were corrected and retested: schema 4 annotation
admission, cancelled late join/paste results, and readiness receipts lost on a
busy operation queue. Earlier native harness attempts selected no image for
Info or changed the background selection during About comparison; those witnesses
were corrected and rerun. Only the verified final receipts support acceptance.

Generated phase evidence is approximately 51 MiB before final receipts/cleanup;
superseded owned runs were removed. Existing corpus and one 2A2 baseline reused;
shared Rust target retained. Approximately 21 GiB remained free, above the 10 GiB
reserve. No user files, briefs or source artwork were removed.

## Human review and stop boundary

Subjective comfort and the derived logo's final owner approval remain in
[HUMAN_REVIEW_PENDING](HUMAN_REVIEW_PENDING.md). The supplied SVG contained a black
rhombus only; colors/t are a declared implementation interpretation, not a claim
of final approved artwork. Check overview/close-up naming and jumping, rename/
delete Undo, grouped duplication offset, missing-image information, Join/Cancel/
Copy address, and 16/32px identity. Windows desktop validation remains separate
from CI. No timed media or subsequent feature phase was started.
