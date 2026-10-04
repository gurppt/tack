# Mission 1D — Spatial organization

Date: 2026-10-04. Local implementation, quality, GPU, persistence, spatial,
idle and native regression gates are complete. Independent evidence review
and configured Linux/Windows CI are successful, including the test-only
follow-up.
No Phase 1E work has begun.

## Starting state and implemented result

Entry was Phase 1C A — PASS, saved on main at
dc66d88a61193572935a40d8eda2f172d6355d8c. Existing gestures, preview/commit,
bounded atomic history, semantic input, source/cache separation and asynchronous
Save remain the owners of their respective behavior.

Implemented: adaptive procedural dots; explicit grid/object/frame snapping with
two transient guides; temporary semantic X bypass; six alignments, two
distributions and row/column packing; flat durable image groups; independent
named rectangular frames with create/name/move/resize/focus/next/previous;
schema-2 persistence alongside old schema-1 reading/writing; crisp square
overlays and bounded bitmap Unicode labels.

[Verified PureRef research](research/pureref_spatial_organization.md) uses the
official 2.1 handbook, with sources and Tack differences. PureRef G toggles grid
visibility and snapping together; Tack G and Shift+G separate them. Neighbor
snapping, arrangement and flat selection use Tack's documented policies.
X bypass, independent frames and simple fixed-gap row/column packing are Tack
choices; no universal PureRef bypass or separate frame object is claimed.
No proprietary implementation, icons or assets were copied.

## Architecture and exact behavior

[Design](design/spatial_organization.md) and [wire schema](design/tack_file_format_v2.md)
contain the detailed rules. The existing DocumentObject gains a Frame variant;
there is no second scene/document. GroupId is a typed durable membership identity,
with a lazily populated inverse lookup. Groups have no duplicated transforms,
contain at least two images, allow one membership per image, and prohibit frames,
group nesting and overlapping memberships. Group hit/marquee expands selection;
move/resize/delete share one Batch and one history step. Ungroup preserves all
transforms and order. Direct removal of a grouped image is rejected; group
deletion explicitly removes membership before images, with exact undo.

Frames are axis-aligned unflipped rectangles and UTF-8 names, nonblank/no control
characters, at most 256 bytes. Spatial containment creates no ownership: frame
move/resize/delete leaves contained images fixed. Images win hit precedence over
frame border/title regions; frame interiors do not steal marquee selection.
Marquee selects a frame only if completely enclosed. Handles precede ordinary
hit selection; middle/Alt pan keeps priority. Navigation follows existing explicit
object order. F2 edits a bounded transient name; Enter commits once, Escape/focus
loss cancels. Native title carries the full selected/editing name.

Grid origin stays world (0,0), with power-of-two spacing adapted to zoom/DPI and
camera-relative f64 phase. No CPU grid geometry is generated. Hidden grid has no
draw/upload and allocates no grid pipeline/uniform until first use. The visible
grid is one fullscreen triangle in the existing canvas pass.

Snapping scans resident metadata and excludes selected objects. Candidates are
world AABB edges/centers and the same adaptive grid lattice. Threshold is 6
logical pixels converted through DPI/zoom; stable object ID/anchor order breaks
ties, objects precede grid, and existing latches hold within 1.5 times threshold.
Each preview is rebuilt from gesture-start geometry. X clears guides and
recomputes the same captured gesture; release restores snapping. Guides vanish
on bypass/commit/cancel. Move uses rotated world AABBs. Axis-aligned resize snaps
its active edge/corner; uniform corners also cap the final projected displacement
at 6 logical pixels, avoiding extreme-aspect-ratio jumps. Rotated resizing stays
unsnapped. No index was added.

Alignment uses world AABBs and treats each group as a unit. Distribution sorts
by leading edge then stable identity, equalizes nonnegative gaps, and preserves
outer span when units fit. Otherwise zero gaps expand right/down from the leading
edge. Row/column pack uses 16 world units and a shared orthogonal leading edge.
Missing images participate by geometry, with no media work or z-order changes.
Ctrl+Shift+D/V also invoke distribution because the native WM can intercept
Ctrl+Alt+Shift arrows; all shortcuts are in README.

Schema 2 is emitted only for actual groups/frames; image-only ungrouped saves
continue schema 1. Existing image records, header and payload machinery remain.
Counts, cumulative membership, byte slices, names, geometry and references are
validated before publishing editable state. Unknown container/schema/record
versions or kinds are refused; newer data is never silently rewritten.
Selection, camera, snapping and name-edit previews are not persisted.

## Anti-bloat and dependencies

No Rust dependency, framework, network client, worker, index or recurring
application timer was added. Cargo.toml/Cargo.lock dependency graph is unchanged;
core remains std-only. Grid/snapping/groups are transient/lazy or proportional
to actual records. Snap scan allocates no candidate vector; preview reuses the
existing selected-object storage. Group/layout actions allocate bounded command
metadata at invocation, never source-sized data. Allocation counts are not
instrumented, so no numeric zero-allocation claim is made for whole gestures.

Fixed costs are explicitly nonzero. On Rust 1.95/x86-64, isolated baseline core
sizes versus shipping are Document 144 → 192 bytes (+48), DocumentObject
160 → 160, Command 176 → 176. Empty membership/group maps have no nodes.
Shipping ImageInput is 752 bytes. Overlay vertex size 24 → 68 raises the initial
128-quad CPU and GPU buffers by 33,792 bytes each; expanded OverlayQuad
80 → 120 adds 5,120 bytes of reserved initial CPU capacity. Actual buffers grow
only when active content needs them, to at most 2,048 quads (835,584 vertex bytes
each CPU/GPU). Up to 512 visible glyph quads share that cap; selection and two
guides retain priority. No per-object widget or atlas texture exists.

Labels use renamed Tack Label Bitmap, converted from pinned GNU Unifont 18.0.01
BMP and upper-plane HEX under OFL-1.1. Original sources, copyright, licenses and
SHA provenance are retained in assets/pixel-font; the conversion reproduces
exactly. The executable embeds notices via --font-license. Fixed sorted 37-byte
records allow binary search and direct row bits; one glyph is one quad with six
expanded vertices. Integer sampling and physical pixel alignment preserve crisp
UI; user media keeps its selected filtering.

The 4,256,961-byte font table is read-only executable storage, with pages touched
on glyph use, no decoding/heap/system lookup/I/O. Measurement found duplicate
const promotion: two full tables. It was corrected to one shared static array,
confirmed by exact byte search in the shipping binary. Debug-stripped binary
grew 15,406,040 → 19,919,264 bytes (+4,513,224, of which 4,256,961 is font data).
With debug symbols: 103,216,928 → 109,070,424 bytes. This is a deliberate storage
compromise for readable Unicode without a font runtime, not a free feature.
Complex shaping/bidi is limited in compact labels; full UTF-8 names remain in
storage and the platform-rendered title. Unknown codepoints use U+FFFD.

The weak-machine aspiration remains architectural, not a driver/legacy hardware
promise. Native NVIDIA process RSS includes a large pre-existing driver baseline;
we do not attribute all of it to board metadata or claim a new low-RAM minimum.

## Measurement conditions and provenance

All acceptance quantitative runs use one shipping release binary frozen under
benchmark-results/phase1d-spatial-shipping, paired with a source archive containing
Rust/WGSL/Cargo/tool inputs and pinned font assets. Linux/X11, RTX 2060,
NVIDIA 580.173.02, Vulkan, Rust 1.95.0. Native windows run serially, without
simultaneous compilation/GPU tests. Kernel page cache was not flushed.
Percentiles are sorted floor(p*(n-1)); maxima are retained, not discarded.

Tracked compact JSON receipts below include binary/source/harness/input/report
hashes; full ignored raw reports, logs, binary and source archives remain local.
The final native spatial harness was strengthened after the archive freeze to
require a displaced single image before alignment; its individual final harness
hash is authoritative. Additional snap/overlay tests were also added after
the freeze, verified by their local/CI receipts. Production Rust/WGSL/font runtime
stayed identical. Initial failed
native fixtures, WM-intercepted shortcut trials, screenshot failures and
pre-correction binaries remain diagnostic artifacts and are not acceptance runs.

Twelve-second native timings are observations on this host. Metadata action
trials use 64 iterations each; their percentiles are descriptive, not a statistical
tail guarantee. Some already-aligned arrangement fixtures produce valid no-ops;
native alignment/distribution checks explicitly require a changed fixture,
changed result and exact undo. No 50k readiness or artist/tablet feel is claimed.

## Spatial performance — source-free metadata

| Objects | Query p50/p99/max ms | Input p99/max ms | CPU p99 ms | GPU p99 ms | Callback p99/max ms | RSS MiB |
| --- | --- | --- | --- | --- | --- | --- |
| 1000 | 0.054/0.094/0.431 | 0.107/0.436 | 0.490 | 0.041 | 3.490/16.397 | 319.5 |
| 5000 | 0.296/0.434/0.462 | 0.498/0.825 | 1.235 | 0.047 | 11.622/15.948 | 322.1 |
| 10000 | 0.601/0.946/6.927 | 1.080/6.943 | 2.250 | 0.043 | 14.805/18.477 | 324.5 |

[Spatial receipt](../benchmarks/phase1d-spatial.json). Input includes begin/commit/undo; query is a subset. The 10k maximum query/input reaches 6.93/6.94 ms despite sub-1.1 ms p99. No index was added. Snap cycles commit once, undo exactly, read zero source bytes and keep queues bounded.

## Grid and frame native rendering

| Case | CPU p50/p99 ms | GPU p50/p99 ms | Callback p99/max ms | Redraws/12s |
| --- | --- | --- | --- | --- |
| grid-hidden | 0.233/0.386 | 0.023/0.041 | 2.953/15.400 | 679 |
| grid-visible | 0.229/0.368 | 0.049/0.070 | 3.140/14.897 | 678 |
| frames-10 | 0.118/0.210 | 0.013/0.025 | 3.227/17.244 | 678 |
| frames-100 | 0.170/0.455 | 0.014/0.026 | 3.142/14.642 | 677 |
| frames-1000 | 0.279/0.446 | 0.014/0.025 | 3.090/16.647 | 678 |

These are forced benchmark redraws, not idle behavior. Visible dots add about 0.02 ms GPU median. Frame cases contain 10/100/1000 records with viewport culling; they do not claim 1000 simultaneously readable labels. Global overlay caps apply.

## Arrangement actions — build and execute

| Selected | Worst action p99 ms | Worst maximum ms |
| --- | --- | --- |
| 10 | 0.006 | 0.020 |
| 100 | 0.064 | 0.069 |
| 1000 | 0.499 | 0.895 |

All ten actions, 64 iterations each; exact undo checked outside timing. Per-action distributions are retained. Already-aligned valid no-ops are possible in these fixtures; native checks require changed alignment and distribution.

## Group manipulation

| Members | Gesture p50/p99/max ms |
| --- | --- |
| 10 | 0.003/0.005/0.009 |
| 100 | 0.026/0.040/0.049 |

Begin/update/commit included, exact undo checked outside timing.

## Frame persistence — exact round-trip

| Frames | Create ms | Save ms | Reopen ms | File bytes |
| --- | --- | --- | --- | --- |
| 10 | 0.005 | 7.300 | 0.020 | 940 |
| 100 | 0.048 | 3.843 | 0.044 | 8590 |
| 1000 | 0.317 | 6.573 | 0.370 | 85990 |

Save includes durable file work; single observations are not throughput guarantees.

## Idle — 10 seconds after settling

| Case | CPU % one core | RSS initial/final MiB | Redraw/GPU submits | Read/write bytes | Worker ticks/switches |
| --- | --- | --- | --- | --- | --- |
| 1C baseline | 0.400 | 313.7/313.7 | 0/0 | 0/0 | 0/0 |
| 1D hidden | 0.300 | 314.1/314.1 | 0/0 | 0/0 | 0/0 |
| 1D visible | 0.500 | 314.0/314.0 | 0/0 | 0/0 | 0/0 |

[Idle/debugger receipt](../benchmarks/phase1d-idle.json). Ordinary open has no application duration/interaction/tour timer. Main thread accrued zero CPU ticks; both overview workers zero ticks/switches. No new/departed thread, source/file byte change or GPU submission during either 1D interval. Total event-loop waits 9/10 include startup/shutdown; context switches are proxies, not exact timer/wakeup counts.

A separate same-binary GDB observation locates periodic unnamed/[vkrt]/[vkps] threads in libnvidia-glcore.so.580.173.02 timed condition waits. They also recur in 1C baseline. Main thread blocks in calloop/rustix epoll_wait; workers block in std channel recv/futex. Eight threads in both versions. The debugger observation was separate from accepted idle timings. Tack has no network client; sockets include X11/driver IPC. Network bytes were not instrumented; no numeric zero-network-byte claim.

## Phase 1C interactions — seven 12-second scenarios

| Scenario | Input p99/max ms | CPU p99 ms | GPU p99 ms | Callback p99/max ms | Commit/cancel |
| --- | --- | --- | --- | --- | --- |
| drag | 0.126/0.155 | 0.469 | 0.076 | 2.738/17.623 | 10/0 |
| resize | 0.008/0.037 | 0.472 | 0.092 | 2.376/6.632 | 10/0 |
| rotate | 0.131/0.234 | 0.849 | 0.073 | 3.102/16.991 | 10/0 |
| crop | 0.007/0.018 | 0.600 | 0.076 | 2.656/16.837 | 10/0 |
| multi10 | 0.127/0.237 | 0.557 | 0.092 | 2.996/16.798 | 10/0 |
| multi100 | 0.188/0.225 | 0.543 | 0.077 | 16.937/33.350 | 10/0 |
| cancel | 0.116/0.132 | 0.480 | 0.100 | 3.096/16.798 | 0/10 |

[Interaction receipt](../benchmarks/phase1d-interaction.json). Same prepared 1k input as 1C. All history/source-zero/queue/upload/residency/drain gates pass. The first multi100 callback maximum 33.350 ms is retained. A paired repeat was prompted by that concern, with no code change and both trials kept:

## Paired multi100 repeat

| Binary | Input p99 ms | CPU p99 ms | Callback p99/max ms | Present p99/max ms |
| --- | --- | --- | --- | --- |
| 1C | 0.225 | 0.623 | 2.019/15.556 | 1.595/15.185 |
| 1D | 0.206 | 0.576 | 2.755/17.818 | 2.298/17.180 |

Presentation tails remain variable and unresolved. Historical 1C multi100 callback p99 was 13.657 ms, versus first 1D 16.937 ms and repeat 2.755 ms. Small input/render cost does not prove stable presentation latency.

## Renderer regression — fresh cache/process per case

| Scenario | CPU p99/max ms | GPU p99 ms | Callback p99/max ms | Recognizable coverage | RSS MiB |
| --- | --- | --- | --- | --- | --- |
| pan-normal | 4.284/7.352 | 0.633 | 14.475/16.716 | 0.816 | 534.3 |
| pan-fast | 2.249/6.006 | 0.180 | 12.781/17.293 | 0.779 | 498.8 |
| zoom-traverse | 0.776/6.280 | 1.483 | 9.404/23.291 | 0.409 | 480.2 |
| pressure | 2.855/6.140 | 0.171 | 12.786/14.874 | 0.955 | 446.8 |
| pan | 0.404/0.733 | 0.053 | 9.041/24.860 | 0.000 | 341.0 |

[Renderer receipt](../benchmarks/phase1d-regression.json). Same generated 1000-object/32-source corpus. Surface/queue/residency/upload/drain gates pass. Pan is the retained distant-jump diagnostic: zero coverage is the known supply limitation, not normal continuous pan or a solved gate. High-LOD/fairness and pressure/presentation-tail variation remain open.

## Product persistence and query regressions

| Operation | Wall ms | Result |
| --- | --- | --- |
| create-linked-1k | 36959.219 | errors=0 |
| reopen-prepared | 6296.858 | errors=0 |
| reopen-warm | 6285.423 | errors=0 |
| reopen-tour | 12257.955 | errors=0 |
| reopen-missing | 6328.919 | errors=0 |
| repair-three | 410.306 | errors=0 |
| reopen-repaired | 6321.510 | errors=0 |
| create-embedded | 21.074 | errors=0 |
| reopen-embedded-after-deletion | 3175.923 | errors=0 |
| query-scale | 195.735 | 1k/5k/10k query |

[Product receipt](../benchmarks/phase1d-product.json). Prepared 1k metadata 4.908 ms; recognizable 727.686 ms; ordinary 80% view 744.522 ms. Tour reuses 1000 overviews with weighted recognizable coverage 0.954 (1C 0.953), source bytes 0 before detail. Missing-source previews survive, repair regenerates 3/reuses 997, embedded original survives external deletion.

## Metadata query scaling

| Objects | Query p50/p99/max ms | Reopen ms |
| --- | --- | --- |
| 1000 | 0.045/0.062/0.071 | 0.714 |
| 5000 | 0.384/0.412/0.412 | 3.847 |
| 10000 | 0.871/0.906/0.919 | 7.817 |

## Native Linux observations

[Native receipt](../benchmarks/phase1d-native.json): 24 spatial assertions at
desktop DPI, 25 at forced 2x DPI including missing-link prepared imagery, 16 image
regressions and 8 low/high zoom assertions. All pass on owned generated fixtures,
with screenshots and exact saved geometry.

Dots remain readable through wheel zoom. X changes an active snapped preview;
release restores identical pixels; Escape preserves saved state. Group members
move by equal world deltas and ungroup/undo exactly. Displaced alignment and
distribution change geometry and undo exactly. Frame name/focus/edge resize/move/
delete/undo preserve image geometry. Pixel label/borders are quiet and crisp;
images retain their filtering. Native low zoom target/handle/cancel and high zoom
interior drag/undo/cancel pass.

Fractional-DPI threshold/grid parameters and bitmap integer readback have
deterministic tests; a fractional desktop compositor was not manually exercised.
No Windows native input, artist, tablet or stylus evaluation was available.
These concrete observations do not claim human feel equivalence.

The earlier desktop-freeze incident remains root-cause unconfirmed. All accepted final 1D owned
windows exited normally, one GPU process at a time, without another freeze.
Final kernel check finds no new NVIDIA Xid/OOM/hung-task/lockup signal. This does
not establish the historical machine-freeze cause or fix system hardware.
See [incident record](INCIDENT_2026_10_03_DESKTOP_FREEZE.md).

## Checks and independent verification

[Local quality receipt](../benchmarks/phase1d-checks.json): cargo fmt --check,
workspace/all-target/all-feature check and Clippy -D warnings, workspace tests,
no-deps docs, cargo-deny, Python/tool tests and release build pass. Regular suite:
99 Rust test invocations initially, then all 9 app spatial tests pass after
explicit rotated/frame snapping and crowded-overlay coverage additions; 7 Python tests (rerun after final harness changes).
Explicit GPU smoke: 1 pass; product GPU suite: 5 passes including ignored native
readbacks. Bitmap conversion exactly reproduces the pinned table.
No ignored GPU test is claimed without explicit execution.

New spatial integration coverage: 9 app, 3 core and 1 storage tests, plus font and
GPU/grid cases. Covers threshold/ties/hysteresis/DPI/grid/bypass, frame and rotated
bounds policies, modal UTF-8/IME/bounds/reusable F2, uniform corner projection
normal/centered/extreme aspect, frame containment, atomic layout/history, invalid
memberships, rollback/delete/undo, schema-1/2 exact reopen and refusal of future
schema or bad-reference data.

The independent verifier implemented none of the changes and inspected
architecture, parser/security bounds, resource behavior, provenance and measures.
Material findings fixed and retested: containing frame wrongly selected by an
interior marquee; negative-gap distribution protruding beyond promised bounds;
modal names retaining held keys; extreme-aspect corner snapping amplifying a small
delta into a large jump. Binary-size review additionally removed duplicate font
storage. No second model, deep hierarchy, abandoned index, unused dependency/
feature, duplicate transform or per-object widget remains.
Final independent local report/evidence review, including the added tests, is clear. The review also corrected
the design wording: alignment uses the initial AABB as a reference and need not
preserve the resulting AABB. Code and test-only follow-up CI both pass.

Configured remote Quality CI on code commit 78ee494 passes on Linux, Windows
and dependencies: [run 37165287330](https://github.com/gurppt/tack/actions/runs/37165287330).
A test-only follow-up adds explicit rotated/frame snap and 4096-frame overlay-cap
coverage; its [Quality run 37165649336](https://github.com/gurppt/tack/actions/runs/37165649336)
passes on Linux, Windows and dependencies. [CI receipt](../benchmarks/phase1d-ci.json).
Subsequent report/receipt changes are documentation-only and skip redundant CI.
Windows native input remains unestablished.

## Remaining risks and stop gate

Annotation/text/shapes/scribble, camera rotation, media, collaboration,
presentation and Phase 1E are not implemented. Higher-LOD refinement/fairness,
distant-jump supply, native presentation tails and pressure-tail variation remain
open. Windows native spatial/DPI, tablet/stylus and artist feel remain
unestablished. Autosave/relink/import/preferences polish, EXIF/ICC/color management
and the final maximum-performance pass remain pending. Compact labels lack
complex shaping/bidi; full names persist independently. Scanning is measured
through 10k, not 50k or a universal weak-machine guarantee. Allocation counts,
network bytes and exact idle wakeups are not instrumented. System-freeze root
cause remains unknown.

The implemented spatial path passes this phase for human review and authoring
the next brief. All limitations above remain explicit. Stop after 1D; no
automatic 1E start.

A — PASS
