# Phase 2A9 — Alpha UX corrective pass

Implementation commit: `161f615993ab85c34fa3ca503aeed93ef0314bda`. Portable-profile addendum included. No public
release/tag is published by this phase. Independent review and physical Windows
acceptance remain release gates.

## Delivered behavior

Image side/corner/full-edge drags are proportional, preserving a single image's
center, rotation, flips, crop, sampling and opacity. Near-edge scaling wins over
rotation; slightly farther outer corners rotate. The top stem and Reset Aspect
Ratio are removed. Legacy bindings for that removed action are discarded while
other custom keymaps remain. Reset Crop, Actual Size / 1:1 and shader-only
Grayscale support a selected image batch and one Undo. Actual Size uses source
pixels multiplied by crop extents and does not change camera zoom.

Ordinary imports fit roughly half the viewport and huge imports three quarters,
subject to fitting both dimensions; multi-import uses coherent viewport spacing.
Source Info distinguishes source pixels, display size and scale. Turquoise Linked,
fuchsia Embedded and red missing Linked identity share one palette helper across
selection, Source Info, import preference and source context commands.

Refresh Source and Toggle Linked / Embedded use the existing cancellable local
I/O worker. Source descriptors/fingerprints are re-probed, revisions use the
editor's high-water counter, asset dimensions change atomically, and visibility /
supply generation is invalidated. CPU/GPU/disk representations retain typed
source/revision keys; delayed old results cannot become new detail. There is no
watcher or full decode on the event/render path. Embedded bytes remain
authoritative; remembered origins are optional and persisted. Embedded → Linked
uses a validated origin or one native picker. Cancellation leaves the source
unchanged. Ambiguous multi-selection produces status feedback, not many pickers.
Binding changes are local-only: shared originals remain Embedded on the server;
save a local copy for rebinding. Grayscale/Lock and object geometry remain shared.

Selected Text Blocks scale proportionally with their font. Click creation enters
editing; Enter/outside click commits, Shift+Enter inserts a newline. Editing sides
resize one axis; corners resize the box freely with fixed font. A text minimum
prevents clipping, including multiline/tab content; horizontal narrowing clamps
width before requiring a taller box. A bounded 16-step width search preserves the
opposite anchor when clamped. The existing TextObject/Note representation is used.
The block caret's bitmap now occupies the left half of its glyph cell, matching
its eight-pixel advance in text and Frame rename.

Frame labels retain a minimum screen font and text width. A stable document-order
layout shifts colliding labels upward using a one-pass sorted interval sweep.
There are at most 128 labels; no persisted offsets, physics or idle solver. Draw,
Frame title hit and Link validation share those rectangles. The no-Frame path
returns immediately. Label rendering has the existing glyph/overlay budgets;
very crowded labels can leave the viewport and no leader is added.

Link flashes the Frame label turquoise for 1.8 seconds; Unlink flashes affected
objects/Frame labels fuchsia. Shared feedback waits for matching accepted state.
Deadlines and preview resources clear afterward. Object Lock is persisted and
undoable, separate from Annotation Selection Lock. Normal picking/marquee skips
locked objects. Select Locked Objects offers recovery; locked selections cannot
be transformed, styled, renamed, refreshed or erased. Moving a Frame with a locked
linked child is refused. Locks are UI editing protection, not an access-control
boundary against arbitrary protocol commands.

Keymap displays 134 rows covering 144 actions: temporary tool variants use their
tool row and Hold behavior. GENERAL/NAVIGATION/IMAGES/DRAWING/TEXT/FRAMES/
COLLABORATION/MEDIA/TOOLS headers are non-selectable; MEDIA has no actions yet.
Exclusive Image/Drawing/Text selection contexts may share a control, with
`Contextual — allowed`; Global/Canvas remain conservative. Runtime filters
inactive contexts before resolving physical/logical domains. Numpad +/- are fine
adjustment; Shift performs ten steps in one Undo. These defaults are remappable;
F10 is fixed. Update... stays open for checking/current/error states, shows both
versions, channel and source, and offers Check, Download/Update and Close.

The addendum delivers `.tackprofile` export/replace import with a safety backup,
named toolbar sets, set cycling without changing tools/layout, and preserved
`.tackey`. See [profile report](PROFILE_EXPORT_IMPORT_ADDENDUM_REPORT.md).

## Compatibility and bounded work

Metadata schema 8 appends Lock/Grayscale flags and supports Source record v2 for a
remembered Embedded origin. The reader accepts schemas 1–8. Files without new
metadata continue using their appropriate earlier schema; older writers reject
newer authority metadata. No image original is re-encoded for grayscale.
LAN protocol is now major 4; every cooperating client/server must use this build.
Source DTOs strip local original paths. Existing server ownership/history rules
also cover the new flag commands and durable restart/Undo/Redo.

No new permanent thread, polling, watcher or metadata database is introduced.
Source replacement reuses the finite worker and existing private spool limits
(64 MiB encoded/image, 4 GiB/session). Frame layout and text constraints are
bounded event/draw work. Source size remains separate from pixel residence.

## Validation and measurements

`tools/check.sh` completed: fmt, all-target/all-feature check, Clippy with
warnings denied, **449 Rust tests**, **31 Python tests**, docs, and cargo-deny
(advisories/bans/licenses/sources all OK). The default workspace run leaves 17
explicit tests ignored; nine renderer tests were separately run on Vulkan
Lavapipe and passed. The no-default-features/all-target check also passed.
Focused new tests cover source cancellation/revisions and metadata persistence,
image geometry and multi-actions, Text behavior, contextual input routing,
keymap category hits, profiles/toolbar sets and durable shared flag history.

The final Linux binary passed **30 native checks**, split across 800×600 and
1024×768: save/Undo/Redo, lock recovery, Actual Size, Linked↔Embedded,
visible refreshed pixels, toolbar content/persistence, native profile pickers and
backup, and persistent Update UI. Screenshots were inspected for toolbar sets and
Update layout. A previous candidate also passed six Link overlay and 21 Mouse
regression checks; their separate receipts retain that candidate's SHA rather
than claiming the final binary was used for those runs.

| Final native viewport | Frames | CPU submission p95 | Maximum reported GPU bytes |
| --- | ---: | ---: | ---: |
| 800×600 | 102 | 2.391 ms | 2,631,680 |
| 1024×768 | 102 | 0.453 ms | 2,631,680 |

These are software Vulkan measurements, not physical GPU timings. The first
viewport overlapped the huge-image run and incremental Windows build. Both
windows settled to zero CPU ticks and zero I/O over separate two-second idle
samples. Raw per-frame records are retained in
[`benchmarks/phase2a9/frame-telemetry.json`](../benchmarks/phase2a9/frame-telemetry.json).

The existing 50,000×50,000 JPEG reached logical Actual Size without allocating a
10 GB RGBA surface: maximum reported CPU pixel payload and GPU residence were
both **2,179,136 bytes**, with four tiles requested/ready. Its 119-frame receipt
ended with zero CPU ticks/I/O over three seconds, stable RSS 120,897,536 bytes
and ten threads. Payload/residency values exclude decoder scratch and OS cache.

Final builds identify the implementation commit above, version `0.1.0-dev.1`,
protocol 4. Linux is `bin/tack`; Windows is `bin/tack-windows-x86_64.zip`,
13,819,536 bytes, with the pinned static libjpeg-turbo 3.2.0 SIMD SSE2/AVX2
decoder. Both source builds passed; Windows runtime acceptance remains pending.
Build metadata still flags the worktree because of retained untracked user
files/reports; application source is committed.

- Linux SHA-256: `548ba4c9d4b7f2446a998302d7cc6589e66c55fa7908bc6a6f136749663824b4`
- Windows ZIP SHA-256: `b643d84212b555103e7716685f76d56ef882e8aa0ca0899271545c20dcb6cff7`

Compact validation, build, artwork, native and huge receipts are committed under
[`benchmarks/phase2a9/`](../benchmarks/phase2a9/README.md); generated boards,
profiles, pixels and binaries remain ignored. To reproduce owned native checks:

```sh
python3 tools/run_phase2a9_checks.py --binary bin/tack \
  --profile benchmark-results/phase2a8/foundation-delivered/profile/preferences.json \
  --output benchmark-results/phase2a9/native-recheck
python3 tools/run_phase2a9_huge.py --binary bin/tack \
  --source benchmark-results/phase2a2/fixtures/50000-square.jpg \
  --profile benchmark-results/phase2a8/foundation-delivered/profile/preferences.json \
  --output benchmark-results/phase2a9/huge-recheck
```

The native tools require the existing prepared Xvfb root and X11 tooling. They
use owned displays/profiles and do not interact with the user's live desktop.

Evidence is under `benchmark-results/phase2a9/`. Candidate receipts record their
actual binary SHA and pre-commit dirty baseline; final receipts identify the
implementation commit. Reports distinguish CPU submission from GPU timing and
software Vulkan from physical hardware. Reused the existing 50k JPEG, shared
Cargo target and cached Windows native library. Generated evidence is below
512 MiB and free disk remained above the 10 GiB reserve. All 169 runtime icon PNGs
from the initial hash snapshot remain unchanged, including Windows packaging.

## Remaining release gates

Independent review required by CONTRIBUTING has not been performed by a separate
reviewer in this session. Physical Windows picker/keyboard/rendering acceptance,
physical GPU/driver behavior and artist review of crowded labels/Text/keymaps
remain human gates. IME rename of toolbar sets is not specifically validated.
Profile import rejects unknown fields/new wrapper versions cleanly; it does not
silently reinterpret future formats. No alpha readiness or public release is
claimed beyond the automated/software-renderer checks above. Work stops at 2A9
plus the explicitly requested portable-profile addendum.
