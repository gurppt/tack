# Phase 2A3 independent daily-use review

2026-10-09. Baseline: Phase 2A2 implementation `873ce97`. Scope: bookmarks,
duplication, metadata information, native shared-board entry, static identity
assets and inherited authority/performance boundaries. The integrator authored
production behavior; this reviewer authored focused independent tests and
reviewed the relevant diff. No Cargo, decoder or native/GPU execution by the
reviewer during serialized integrator checks.

Status: **LOCAL VERIFICATION PASS — exact-source CI pending**. No unresolved
concrete defect remains in the reviewed paths and final local receipts. Exact
implementation Linux/Windows/dependency CI remains to be attached after commit.

## Concrete findings and corrections

1. **Schema 4 initially rejected annotation objects.** Adding a bookmark to a
   board with a note/annotation selected schema 4, while the reader admitted
   those object kinds only with `schema == 3`. Such a saved board could not
   reopen. The integrator changed admission to `schema >= 3`. Independent
   `bookmarks_compat.rs` covers schema 4 with images, frame, all current
   annotation kinds, a valid image group and bookmarks. Correction reviewed and
   executed independent regression green in the final gate.
2. **Cancelled join/paste results could target a later panel.** Cancel A then
   immediately confirm B while A was still draining: B queued with Connecting
   visible; A's untagged `Joined(false)` could replace that panel, hiding B's
   result and its cancellation affordance. Clipboard results could similarly
   populate a newly opened form. `LocalWorker::poll` now filters cancelled
   `Opened`, `Joined` and `AddressText`; dismissing both Connecting and Join
   cancels active work and clears queued work. Private `cancel_tests.rs` covers
   late-result suppression and the reviewer's uncancelled delivery witness.
   Correction reviewed; suppression and uncancelled delivery tests are green in
   the final gate. Final native receipts cover cancellation, failed/successful
   join and clipboard round-trip; they do not claim a forced Cancel A→B race.
3. **A busy child operation queue could permanently lose its join receipt.**
   `shared_update` took `options.join_receipt` before admitting `JoinReceipt`
   through the bounded operation queue. If both worker and queue were occupied
   by explicit child/profile activity, admission failed with no remaining path
   to retry; the parent timed out and killed an already joined window. Retain
   the receipt until successful admission and retry from bounded completion
   handling. The integrator's `notify_join_receipt` now leaves the path intact
   while worker/queue is busy, consumes it only after admission succeeds, and
   retries from `poll_local` after bounded worker drainage. Failure text is
   bounded to 256 characters. Correction reviewed; rebuilt final native join
   succeeds. Neither reviewer nor final receipt claims a native run forcing the
   busy child-queue case.

The reviewer's initial group fixtures incorrectly included a note; existing
groups admit images only. Fixtures were corrected to three-image partial groups
and a two-image storage group. This was a test error, not a production defect.

## Source audit

- Bookmarks are invisible metadata: at most 64, nonzero typed IDs, unique IDs,
  names at most 128 UTF-8 bytes without controls, bounded finite center and
  zoom. The existing `Camera::set_view` performs exact valid jumps. Whole-list
  commands use existing reversible history and rollback; no-op edits do not
  create history. No frame/object/spatial representation is allocated for views.
- Schema 4 appends checked bookmark records to existing spatial metadata. Old
  schemas 1–3 remain accepted; other schema/record versions, invalid IDs, counts,
  names, floating values, truncations and trailing data are refused. Removing
  all bookmarks permits the existing minimal schema again. No thumbnail or
  original/cache payload is bookmark state.
- Duplicate constructs one flat metadata batch, in document order, with fresh
  object/group IDs and a deterministic grid offset. Images preserve asset and
  source identities plus crop/opacity/filtering/transform state. Notes,
  annotations and frames copy their durable state. A group's duplicated image
  members are linked only to new members; a single selected member produces no
  new group. Count/missing-object/invalid-transform failures occur before the
  existing atomic command path mutates authority. No image bytes/cache products
  are inputs to duplication.
- Shared context disables Duplicate and bookmark creation; handlers enforce the
  same policy. Bookmark rename/delete are disabled in shared panels and rejected
  by mutation handlers. Shared DTO serialization explicitly refuses bookmark
  edits, avoiding an added protocol. Existing persisted bookmark snapshots can
  be viewed/jumped; local-only editing is a stated boundary.
- Source information formats already-known document/source status/fingerprint
  or pinned original-range size. It has no filesystem/decoder/cache/worker
  access. Missing/foreign/unobserved states and unrecorded size are explicit;
  no guessed codec or content analysis is introduced. Temporary info/name/list
  panels retain only bounded metadata. Current 800×600/scale 1–2 layout tests
  cover maximum bookmark lists and controls, not subjective readability.
- Join validates a bounded numeric IP/port and nonzero board ID or canonical
  `tack://` syntax before admitting work. DNS/server browsing/recent-network
  probing are absent. An explicit temporary operation launches the same CLI
  `join` executable/backend/canvas in another window; no new protocol or local
  authority conversion occurs. Receipt polling, process startup, clipboard,
  directory and file operations occur on the existing operation worker. Cancel,
  failure or 15-second timeout kills/reaps an uncompleted child; successful
  shared windows own the existing backend. No new idle worker/timer is added.
- Publish UI and recent shared descriptors are absent rather than a second
  authority/lifecycle implementation. Existing CLI publish remains the path.

## Static icon boundary

The supplied black rhombus SVG remains intact. A separate derived SVG supplies
the brief's fuchsia base/turquoise italic `t`. Existing pinned Pillow tooling
generates fixed PNG/ICO sizes, 32² window RGBA and 16² About pixels at build time.
Runtime includes pixels only, with no SVG parser/image worker/dependency.
Packaging copies static icons and creates a Linux desktop entry; window icon
uses the existing Winit API. The ICO is distributed; Windows executable-resource
integration is not inferred merely from that file's presence.

The derived `t` artwork is an implementation interpretation, not an explicitly
approved final owner SVG. The brief's instruction to wait for final supplied
artwork is therefore a separate owner-artwork acceptance limit. Do not present
this source/runtime review as final brand approval; keep that decision in the
human queue and distinguish it from reproducible asset/package tests.

## Independent tests

New reviewer integration tests: `tack-core/tests/bookmarks.rs` (3),
`tack-storage/tests/bookmarks_compat.rs` (3), and `tack-app/tests/duplicate.rs`
(3). They cover malformed/max metadata, exact history/Frame independence,
legacy/current schema, mixed/partial grouped duplication, fresh identity,
single original/source/asset reuse and exact undo/redo/save/reopen. Generated
test payload is 4 KiB, with owned temporary directory cleanup. The reviewer
also added the uncancelled witness to integrator `local_worker/cancel_tests.rs`.

All nine independent integration tests and the uncancelled witness passed in
the final gate. No test/fixture failure remains.

## Final local evidence

The reviewer checked the receipts directly, without launching compilation or
native work. All 424 files in `benchmark-results/phase2a3/final-build/source.json`
match current content. Its SHA-256 is
`dea428ca62d8bbf6983d524cdc6b1c3cdc43b26ce1c70703fc6fadc348807564`;
all 17 raw receipt hashes in `docs/measurements/phase2a3.json` also match.
The measured client SHA-256 is
`3ad64ecb0533b383d47e36884785c36f065f774df69e478201a7ca3e064a8893`.
Documentation is excluded from that implementation manifest.

- `full-gate.log`: 334 Rust tests, 27 Python tests, formatting/Clippy and
  cargo-deny checks pass. Five `gpu-*.log` receipts total 14 passing GPU tests.
- `daily-verified/receipt.json`: 20/20 checks pass, including bookmark exact
  camera/save/reopen, metadata duplication with one undo/redo, image information
  without new decode admission, local authority preserved across Join, child
  cleanup, canonical shared-address clipboard round-trip and shared restrictions.
  The reviewer visually inspected `daily-image-info.png`: one selected image
  and its actual Image Info panel, not an error panel. The earlier 19-check
  pilots did not select the image correctly and are excluded.
- `about-verified/receipt.json`: 41/41 checks pass across three palettes and
  scales 1–2, repeated open/close, missing/corrupt/oversized optional artwork,
  canvas resumption and retired image GPU resources. The earlier whole-window
  comparison pilot changed selection through right-click and is excluded.
  Source About artwork and decoder hashes are unchanged.
- `idle-final/summary.json`: 10 runs, all with zero idle redraws and zero I/O
  delta; socket records are Unix sockets, no local IP socket. Baseline/current
  thread counts remain 41. Empty startup medians are 452.35/453.92 ms, 1k-board
  medians 450.48/467.09 ms. These are two observations per case on this Linux
  RTX 2060 host, not a statistical speedup or old-CPU claim. The 64-bookmark
  case is current-only: the baseline binary refuses schema 4, so no equivalent
  baseline-board comparison is asserted.
- `metadata/metadata-cost.json`: 64-bookmark commit 0.0093 ms and 3,359-byte
  board; 1k/10k-object duplicate construction 1.54/16.10 ms and commit
  0.77/9.06 ms. Assets/sources stay one and original bytes read stay zero.
  The huge original-size descriptor is synthetic metadata, not a generated
  900-GiB file. History cost grows with copied object metadata.
- Square default/potato and mixed final receipts each cover 11 settled poses,
  complete requested tiles, same-session return with zero additional source
  work, and reopen hot with six raw hits and no source work. Quiet I/O is zero;
  CPU/GPU and disk limits remain within their profiles. `lod-final` records
  2,520 wheel, 32 pan and five filter actions with zero supplied-pixel valleys
  and zero idle redraw/I/O. This native sequence does not force evictions;
  the separate GPU churn test covers that boundary. `shared-final/checks.json`
  has 32/32 passing authority, CAS/history/relink/restart checks.

Stripped client size is 19,450,176 bytes versus 19,329,864 baseline bytes
(+120,312, about 0.62%). Server SHA changes with core/storage metadata machinery;
it is not claimed byte-identical to Phase 2A2. Dependency evidence shows no new
UI, image or renderer baggage in the server. Supplied black-rhombus SVG remains
intact; reproducible static asset tests do not settle final owner branding.

Exact-source CI, human interaction/artwork acceptance and Windows desktop runtime
are distinct remaining closeout items. Local technical verification is complete.
