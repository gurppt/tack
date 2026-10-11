# Linked LOD refresh corrective

Implementation: `11c6b3259a2b8dce63e3aaad9bd8765b73cc406d`.

The user's Linked → Embedded → Linked stale-view report exposed persistent
whole-image cache keys containing source/asset ID, revision, generator and edge,
but no source fingerprint. A revision allocated during a previous session could
be allocated again after reopening. Validating the new Linked file did not stop
that new descriptor from finding the previous revision's pixels. A regression
first failed with `stale Thumbnail`, then passed with the corrected namespace.

Persistent whole-image LOD names now hash document identity, resolved link path,
source kind, recorded size/mtime fingerprint and asset dimensions. Embedded keys
use the pinned original's length/CRC. Shared verified paths use their actual
fingerprint. Existing persistent regional tiles already include fingerprint and
namespace checks. A stored board overview also requires its source descriptor to
match the current source, beyond matching revision. Old unqualified derived
filenames are ignored; the existing disk quota trims disposable data. No original
or runtime artwork is rewritten.

Local boards perform one finite Linked admission pass on open and focus return.
Fingerprint/header work runs on the existing local worker. Only changed valid
Linked sources receive fresh revisions and dimensions, in one atomic Undo batch.
Transforms, crop, flips, filtering, opacity, grayscale and locks remain intact.
The normal source-transition invalidation re-requests progressive detail. Results
check captured document/source identity before applying; explicit queued source
edits win over the background result. Application waits for active gestures/text
editing to finish. Embedded authority and shared/offline copies are excluded.
Missing/invalid files retain last-known state without an automatic modal.

There is no watcher, recurring timer, idle redraw or whole-original hash on every
focus. Edits while Tack keeps focus need another focus/open event or explicit
Refresh Source. Changes preserving both file size and modification timestamp
cannot be detected by this stat fingerprint; explicit Refresh remains available.
An unchanged pass adds no Undo entry. The changed descriptor is saved with the
board through its existing save/recovery path.

Validation:

- Full workspace checks: fmt, all-target/all-feature compile, Clippy, docs,
  452 Rust tests, 31 Python tests, cargo-deny all OK.
- One subsequent additional multi-source regression passed (453 unique Rust
  tests in total); the focused source-transition suite contains five passes.
  Tests cover one Undo, distinct revisions, cancellation, late toggle safety,
  unchanged/missing/invalid links and Embedded exclusion.
- Final native binary: 12 owned Xvfb/Lavapipe checks. Warmed red LODs at transient
  revisions 3/4, restored/saved revision 1, edited externally to green, reopened,
  and verified green through Embedded/Linked with reused revisions. Focus return
  after a blue external edit updated visible pixels without a refresh command.
  Geometry stayed identical. Settled CPU ticks and I/O were zero over two seconds.
- Final 50,000×50,000 Actual Size run: 128 frames, four tiles, maximum CPU pixel
  payload and reported GPU residence each 2,179,136 bytes, stable RSS 120,938,496
  bytes, zero idle CPU ticks/I/O over three seconds. Software Vulkan readings
  exclude decoder scratch and OS cache and are not physical GPU timings.
- The no-default-features/all-target build also passed. A Rust 1.95 incremental
  query-cache compiler error required deleting only owned `huge_authority`
  incremental entries; subsequent checks passed. This was a compiler failure,
  not a Tack crash.
- All 169 snapshotted runtime icon PNGs remain byte-identical after both builds.

Final Linux SHA-256:
`02e8df108185e022c8a34827a2c203c23ca7f9fb7aabf6be379cd5fbb14e8b0f`.
Final Windows ZIP (13,825,838 bytes) SHA-256:
`e18b433b724f98698e3707e041bea4882c7b52b5417348ffe34ebba478fb4e45`.
The Windows build retains libjpeg-turbo 3.2.0 static SIMD SSE2/AVX2.
Metadata schema and protocol remain 8 / 4. Physical Windows and independent
review remain acceptance gates; no public release/tag is created.

Compact receipts and raw frame records:
[`benchmarks/phase2a9/linked-refresh/`](../benchmarks/phase2a9/linked-refresh/).
Larger owned evidence is ignored under `benchmark-results/phase2a9/linked-*`.
Reproduce with `tools/run_linked_refresh_checks.py --binary bin/tack --profile
benchmark-results/phase2a8/foundation-delivered/profile/preferences.json --output
benchmark-results/phase2a9/linked-recheck`, using the existing prepared Xvfb root,
X11 tooling and xmessage. Each run uses its own board, profile and display.
