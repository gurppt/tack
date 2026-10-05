# Mission 1F — Local production hardening

Date: 2026-10-05. Starting main/origin:
`759fee0ac8ace8558556571498c84a6270b44760`, Phase 1E **A — PASS**.
The required product brief, engineering/UI charters, phase reports, preparation,
backlog, settings audit and current interaction/spatial/annotation/format
contracts were read. The user's amendment removes ellipses completely and
updates the ignored product brief; no contrary compatibility requirement remains.

This report is being finalized. Final measurements and configured CI receipts
must be present before the final gate can pass.

## Delivered local workflows

An ordinary launch creates an Untitled board. New, Open and Recent use independent
native processes. Import through a real native picker, multi-file native drop,
clipboard PNG, local path/file URI and bounded text paste into notes are usable.
Preferences expose linked/embedded defaults, image sampling, grid, integer UI
scale and handle/hit sizes. The searchable canonical keymap supports unassigned
or multiple bindings, keyboard/pointer/wheel, press/release, conflict detection,
reset action/category/all and readable import/export. Recent holds 16 paths,
without thumbnails, automatic last-board reopening, account history or watchers.

Normal Save preserves later dirty generations. Save As uses an unused target,
preserves identities/layout and safely rebases supported relative links; edits
and local completions pause during its publication. Close provides Save,
Discard and Cancel. Note/frame drafts commit on Save/close, survive focus/DPI
changes and remain recoverable after validation errors. Imported/relinked
mutations wait while a draft/gesture is active; a capacity-one worker channel
provides backpressure rather than replacing the draft's document underneath it.

Same-file writers are visibly refused under a stable canonical-path OS lease;
other boards have independent sessions/recovery. Normal Save/repair detect an
external replacement and retain dirty work. An initial Open failure has an
explicit noneditable error-only window. Enter/Escape closes it; it cannot accept
unsaveable notes/imports or arm an expired recovery deadline. A real Save error
instead keeps the opened editor usable. Invalid/future preferences stay intact
while defaults permit an ordinary board to open.

Relink changes a selected image's shared Source and associated asset dimensions
atomically, preserving object IDs/layout/crop/styles and advancing revision.
Undo/redo never recycle issued revisions. Missing references retain last-known
previews where available and show explicit missing/foreign/changed state;
obsolete async results cannot publish as the new revision. There is no watcher.

## Recovery and authority

Recovery is a separate whole-document snapshot, never a publication over the
normal board. Dirty edits debounce for five seconds, with a 30-second ceiling
under sustained edits. A clean/already captured generation has no autosave
clock; failed work does not retry forever. The existing storage worker streams
CRC-validated ranges; no per-edit fsync or original-file read on the render path.

Two alternating snapshots plus a validated owner marker and manifest permit
publishing the inactive snapshot before replacing control state. Fault injection
before manifest publication preserves the previous valid recovery. Startup
requires matching normal-file and snapshot stamps. Explicit Restore validates
original CRCs on a worker, restores dirty work and requires normal Save;
Discard removes only recognized owned artifacts. Corrupt/foreign/unknown entries
cannot overwrite authority and are retained conservatively.

Only edits in a completed snapshot can be recovered. Whole-board snapshots can
cost substantial disk bytes, especially embedded boards. One save temporary and
an inactive recovery snapshot may coexist. Unix directory syncing is implemented;
process-kill tests are not power-loss durability proof. Cooperative leases and
header/length/mtime comparison are not an atomic CAS against adversarial writers,
hard-link aliases or arbitrary network-filesystem lock semantics.

The 16 private Untitled slots are reclaimed only under ownership after checking
empty identity and recognized artifacts. Symlinks, foreign files and crash seeds
with recovery are not inferred disposable. Save As/clean close can retire a
verified empty seed without replacing its stable lock sidecar.

## Pixel rendering and font decision

Tack-owned UI, text, handles, guides, frames, rectangles, segments, arrows and
scribbles use binary coverage on an integer logical pixel grid, without edge
antialiasing. Requested opacity still blends normally. User images keep their
Smooth/Nearest behavior. Tested fractional DPI retains all bitmap rows; presentation
rounds to a whole scale while geometry remains in the existing world model.

Existing **Spleen 8×16 2.2.0**, pinned to upstream commit
`57f9219328c9f5873085320fe8bc8f7dd34b8791`, is the primary bitmap face under
BSD-2-Clause. It supplies 1001 glyphs in 37037 packed bytes. GNU Unifont remains
the existing 115053-record, 4256961-byte fallback. Both immutable tables are
looked up lazily, with no heap-wide preload, glyph texture or growing font cache.
The old 1867776-byte note texture and 416042-byte compressed payload are removed.
No custom typeface or runtime font engine was designed.

Espy/Mac/Amiga candidates and their unresolved distribution rights are recorded
in [the font decision](design/ui_font_decision.md); supplied user `gfx/` files
remain outside this change. The selected font includes pinned provenance,
reproducible BDF conversion, license and byte/hash tests. Complex shaping,
bidi/RTL, color emoji and full localization remain unimplemented.

Ellipse creation/actions/tools/picking/wire support/shader specialization and
exclusive tests/probes are removed. Schema 3 retains other record numbers;
kind 5 is rejected, with no compatibility reader or migration. Ten owned old
ellipse fixtures were deleted; clean current fixtures were regenerated. Historical
1E reports/measurements remain explicitly historical. No private production
assets, `.pur` files or user boards were removed or uploaded.

## Bounds, dependencies and anti-bloat audit

One requested local operation uses one on-demand worker and a one-slot result
channel, with at most one queued request. Existing product preview supply stays
at two workers/16 pending jobs, 64 MiB CPU payloads, 512 MiB SSD cache, 128 MiB
GPU residency, eight uploads/16 MiB per frame and three in-flight submissions.
No resident local service, font daemon, network client, telemetry or analytics.

Image/header policy remains JPEG/PNG, 64 MiB encoded and 6000×4500 pixels.
Imports are progressive and capped at 4096 paths, 2 GiB copied per batch and
4 GiB per session spool. Streaming uses 128 KiB chunks and stable range handles;
cancellation retains admitted images and rolls back failed partial ranges.
Clipboard helper output is bounded before Rust allocation (64 MiB PNG, 64 KiB
text/reference, 16 KiB note). Profile JSON is capped at 256 KiB, bindings at 256,
Recent at 16; UI search and glyph packets are also bounded.

Loaded embedded originals needed by undo/redo may retain **whole old container
inodes and disk allocation** after relink/save. That cost is independent of the
4 GiB import-spool quota until history eviction/subsequent publication/close.
The 200-entry/32 MiB history metadata budget does not count original payloads.
There is no eager-copy/compaction subsystem or zero-retained-disk claim.

No new Rust package/version is added. Winit's existing serde support and existing
CRC support are enabled; the renderer's direct image dependency is removed.
The minimal existing overlay draws local panels, without a GUI framework,
webview, rounded widget stack, animation or alternate document/renderer/input.
Negative asset states settle more than 256 missing references without clearing
and re-requesting the entire cache. Title equality prevents redundant property
writes; ordinary settled windows use ControlFlow::Wait and no redraw/caret timer.
Optional bounded error output uses existing stderr/tracing, with no automatically
created persistent logs; user-directed shell capture is not rotated by Tack.

## Measured evidence

Final shipping binary SHA-256:
`92e50f202a196fe919dacc1ac2031143b12f70f3392fd3ad84fcc68c18001a66`.
Canonical source ZIP SHA-256:
`44e89897d6fd5d875dc96e2c84540683953f36ac423969f681f1de912ba945c7`.
It includes runtime, manifests, lockfile, all bitmap assets and reproducible
harnesses; runtime files compare byte-for-byte to the measured implementation in
[verification](../benchmarks/phase1f-verification.json). Each suite retains its own
binary/harness/board/report provenance. Earlier incomplete binaries and failed
harness attempts remain local and are not substituted for accepted evidence.

Linux 7.1.5, Rust 1.95.0, RTX 2060 6 GiB/NVIDIA 580.173.02, Vulkan; owned
**Xvfb :92 with noncomposited xfwm4**, 1280×720 clients. Measurements are serialized,
without concurrent builds or other owned GPU benchmarks. Kernel page cache is
not flushed. Xvfb/native driver initialization also creates llvmpipe threads:
RSS/thread inventory includes that platform baseline, not only Tack-owned heap.
These measurements do not establish low-end-machine readiness or monitor latency.

### Startup and settled idle

| Generated board | Native startup ms | Metadata ms | First frame ms | 80% image-useful ms | GPU setup ms | Settled RSS MiB |
|---|---:|---:|---:|---:|---:|---:|
| Empty/new | 523.2 | 5.06 | 539.2 | N/A | 498.4 | 335.21 |
| Small, 8 embedded refs | 459.2 | 0.27 | 475.1 | 513.0 | 433.8 | 335.77 |
| Prepared, 1k linked refs | 463.3 | 3.13 | 479.7 | 517.5 | 437.3 | 337.97 |

The first useful clock is CPU submission of image coverage before present, not
monitor presentation or annotation readiness. Empty boards have no image clock.
Source-original bytes on all three prepared/native opens: **zero**. Preparation
is separately charged in the product regression rather than hidden as startup:
1k synthetic source preparation took 35.72 seconds and read 7.11 GB of originals.

Each ordinary idle row observes five settled seconds without application benchmark
timers. Total process CPU: **0.4%, 0.2%, 0.2% of one core** respectively; all have
41 threads, unchanged RSS, zero redraws/submissions, zero title-property updates
and zero /proc file I/O. Per-thread switches remain in
[local measurements](../benchmarks/phase1f-local.json): the sleeping event loop
and no local polling thread do not imply NVIDIA driver threads never wake.
The prior frozen 1E binary is remeasured against the same owned fixtures/display;
matched comparison is recorded in [baseline](../benchmarks/phase1f-baseline.json).
The observed matched 1E RSS is 334.11/335.27/337.30 MiB, giving 1F deltas
+1.11/+0.50/+0.68 MiB. Thread counts remain 41; both versions have zero idle
redraws/submissions/I/O. Native startup deltas are +69.1/+20.3/+18.4 ms in these
single observations. Empty 1F includes New/ownership/seed creation; its 1E
counterpart opens that empty file because old 1E has no native New workflow.
The two prepared comparisons use ordinary Open on the same files. Single samples
and GPU setup variability are not a statistically significant speed/regression
claim; 1E lacks separate GPU-setup/first-frame instrumentation.

The extra failed-open-after-drop check has zero main-thread CPU/context switches
and zero I/O over six seconds. Total CPU is **16 ticks (~2.67% of one core)**,
15 in NVIDIA `[vkps] Update` plus one driver tick. This is a remaining native-driver
cost in that tested error/drop sequence; the application's expired recovery
spin is eliminated. A separate error-only diagnostic without input measured one
total tick. Initial harness failures conflated total driver work with event-loop
spin; the accepted receipt preserves both and checks the latter directly.

### Progressive import and recovery

| Native drop | Admitted/requested | Encoded input | Admission ms | Peak RSS MiB | Preview overviews generated |
|---|---:|---:|---:|---:|---:|
| One small PNG | 1/1 | 233 B | 126.1 | 336.00 | 1 |
| Many small PNGs | 64/64 | 26329 B | 2412.5 | 337.61 | 12 |
| Three 4000×2500 PNGs | 3/3 | 27375679 B | 233.0 | 345.77 | 3 |
| Cancel 120 repeated large paths | 4/120 | 1095027160 B requested | 264.8 | 345.85 | 3 |

Admission clocks include drop coalescing/input, not full preview preparation or
normal Save. Only visible previews are scheduled; 12 overviews after 64 imports
is progressive work, not a claim that all 64 were prepared. Cancellation follows
a short native input interval and retains the admitted objects. Duplicate paths
share sources/assets. Peak memory is sampled VmHWM and may miss very brief peaks.
I/O receipts separately include spool copying, preview work and normal Save;
large embedded final containers are about 27.4 MB.

| Recovery board | Debounce to manifest ms | Storage worker ms | Recovery directory B | /proc physical write B | Pan callback p99/max ms |
|---|---:|---:|---:|---:|---:|
| Small/8 refs | 4965.4 | 14.42 | 12318 | 20480 | 43.31 / 44.56 |
| Prepared/1k | 5069.0 | 156.78 | 47213092 | 47222784 | 41.38 / 46.62 |

Normal board hashes stayed exact and both editors remained dirty. Recovery is
one completed generation, not a normal-save clean acknowledgement. Native pan
was exercised **after** snapshot publication; edit-during-save correctness is
proven separately by deterministic worker/generation tests, not inferred from
that pan. Pan callback includes acquisition/presentation. The ~47.2 MB linked
snapshot includes persisted derived previews; it is not an incremental journal.

### Regression costs and native cases

On the final binary: seven image manipulation/cancel traces, eight spatial
snap/grid/frame traces, nine shape/text/scribble/mixed traces, ten product
persistence/preparation/repair/reopen runs and four navigation/pressure/tour
traces pass. Traces use six seconds here; annotation steady CPU begins after
one second. P99 is sorted floor(p×(n−1)); maxima/raw reports remain available.

| Annotation stress | CPU p99 ms | GPU pass p99 ms | Callback p99 ms | Peak RSS MiB |
|---|---:|---:|---:|---:|
| 1k shapes | 0.73 | 0.056 | 40.17 | 339.21 |
| 10k shapes | 3.71 | 0.338 | 47.80 | 350.35 |
| 1k short notes | 4.51 | 0.302 | 48.42 | 343.77 |
| 100 bounded scribbles | 1.01 | 0.149 | 40.60 | 340.60 |
| Prepared 1k images + 100 shapes/frame | 0.52 | 0.084 | 39.63 | 340.42 |

Generated dense shape/text stress is not an artist corpus. The generated varied
1k-reference board, native image/group/frame cases, notes/strokes and shared
missing-source board collectively exercise representative local content.
The native/automated production pass contains **21 production + 19 recovery/error
checks**, **25 annotation checks at each of scales 1/1.5/2**, **24 spatial**,
**16 image** and **8 zoom** checks. Resize/unmap/map is proven; OS minimize policy,
live monitor migration and stylus feel are not inferred. Captures were inspected
in the owned display. Detailed receipts are in
[native](../benchmarks/phase1f-native.json) and
[regressions](../benchmarks/phase1f-regressions.json).

The native surface callbacks have ~36–48 ms presentation-dominated tails on this
Xvfb setup despite much smaller canvas GPU pass time. Historical desktop 1E
callbacks used a different display path and are not a valid performance delta.
Distant-jump supply/high-LOD/presentation limits remain visible; these short
regressions do not establish 50k or the final maximum-performance target.

### Multiple instances and binary weight

The production flow's two simultaneous boards use 336.73/335.14 MiB RSS,
41 threads each, 0.6%/0.4% of one CPU core and zero I/O over five seconds.
A dedicated repeated two-board audit measures 336.04/335.42 MiB, 41 threads,
0.2% CPU each and zero I/O. NVIDIA reports **24 MiB graphics-process allocation
per instance** in that snapshot, including driver allocations; application image
payloads are 222208 B for the mixed board's current view and zero for the empty
one. Offscreen annotations allocate zero annotation buffers. This is per-process
memory attribution, not a per-process GPU-time or whole-machine capacity claim.

Final ELF sizes: full/debug **113568696 B**, strip-debug **20814072 B**,
fully stripped **17588432 B**. Against measured 1E, strip-debug grows **222960 B
(+1.08%)**, fully stripped grows **127744 B (+0.73%)**. The selected font removes
an atlas but new workflows/ownership/recovery code still have a measured binary
cost. No Rust package/version or GUI framework is added; unused panel/font
texture allocations are not resident. Ordinary image payload residency is
0/119808/196608 B for empty/small/prepared windows, separate from the driver.

Full owned raw artifacts are under `benchmark-results/phase1f-*`; compact receipts
are tracked under `benchmarks/phase1f-*.json`. Private production assets and
`.pur` input were not used.

## Verification and findings

The independent verifier did not implement the phase. It reviewed workflows,
ownership/recovery/dirty generations, deferred mutation, original retention,
preferences/keymap, UI/fonts, budgets, dependencies and resource evidence.
Findings led to fixes for seed symlink reclamation, CLI ownership/release,
SaveAs revision high-water and loaded-original history, deferred relink while
editing, note confirmation preserving validation failures, DPI draft preservation,
and the unsaveable failed-open/error-dismiss mode. A final review of the latter
found no remaining concrete issue. A corrected numerical count is 21 production
checks in the initial full native production harness, rather than a larger claim.

Native X11 workflows and regression suites use an owned isolated display and
known owned process/window IDs, with real picker/drop/clipboard input. They
include create/import/save/reopen, mixed annotations/organization, relink missing
shared sources, undo/redo, shortcut changes, multiple boards, crash/Restore,
failed Save/close Cancel, corrupt recovery, future/truncated/locked refusal and
pixel/DPI/window cases. This is an automated realistic native production pass
with inspected captures, not an artist/stylus preference study. Windows native
manual testing and live multi-monitor scale transitions are unavailable.

The historical full desktop freeze remains root-cause unconfirmed; the repeated
`set_title` event-flood defect was fixed earlier. Current idle evidence separately
checks title properties, redraws, submits, main-thread behavior and I/O. It does
not establish the historical machine-freeze cause or rule out system hardware.
See [the incident record](INCIDENT_2026_10_03_DESKTOP_FREEZE.md).

## Quality and CI

`bash tools/check.sh` passes on the final implementation: formatting, all-targets/
all-feature locked check, Clippy with denied warnings, workspace tests (**124
passing test/doc-test executions**), docs, **8 Python tests**, and cargo-deny
advisories/bans/licenses/sources. Explicit hardware GPU execution passes all
seven product tests (five hardware/offscreen checks, two ordinary tests), plus
one GPU smoke test. Streaming persistence/recovery/ownership/clipboard/keymap
and bounded input tests pass. `git diff --check` passes.

Configured Linux/Windows Rust and Linux dependency/software-GPU CI is pending
the implementation push. No Windows CI pass is claimed before its receipt.

## Retained limits and stop

Product views remain overview-only. Distant jumps, high-LOD fairness, final
presentation tails, exact allocator counts/per-process driver VRAM/network-byte
attribution, 50k scaling and the final maximum-performance pass remain unresolved.
Native Windows clipboard images/DPI/helpers and power-loss behavior are not
claimed. Optional Linux picker/clipboard helpers must be available. Arbitrary
slow filesystems can still stall an I/O worker; there is no remote-filesystem
or hard-link collaboration guarantee. Whole snapshots and retained originals
can require substantial disk space. Error-only driver idle cost is recorded
separately from the sleeping application event loop.

No server/auth/collaboration, media playback, presentation, `.pur` import,
cloud/account/DAM/updater/AI/plugin or rich-text branch was started.
Stop after 1F; human review must precede the next major branch.

B — REPEAT
