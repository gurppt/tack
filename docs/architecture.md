# Phase 1B architecture

Five crates separate resident domain (`tack-core`, no dependencies), source and
representation supply (`tack-assets`), explicit snapshot I/O (`tack-storage`),
GPU (`tack-render`) and native/CLI composition (`tack-app`). Phase 1A's document,
command/history and action/keymap kernel remains authoritative. Phase 1B connects
it to real save/reopen/image rendering; no selection/manipulation UI is built.
See [the 1B report](MISSION_1B_REPORT.md), [storage decision](design/tack_storage_decision.md)
and [format](design/tack_file_format_v1.md).

## Product vertical slice

`Source` holds identity, lossless native/foreign descriptor, revision and cheap
optional size/mtime fingerprint; original bytes live outside core. `SetSource`
changes it explicitly with undo and revision high-water protection. Editor dirty
state is conservative: effective edits/undo/redo set it, successful exact-current
save clears it. Persisted history is excluded. IDs use OS 128-bit entropy outside
core; table/reference validation catches imported duplicates.

`tack-storage` reads only bounded header/metadata/directories on open. Original
and preview ranges share an open generation via positional reads, not reopened
paths or shared seek cursors. Authority versions/CRC/reference/geometry failures
refuse editable open; derived-directory/entry/payload failures discard only
reproducible previews. Save streams 128KiB to private exclusive sibling temp,
checks copied stored CRC, syncs, replaces, then syncs the Unix parent. Explicit
post-publication sync errors distinguish a new published file. Failed prepublish
save retains the old target; abandoned crash temps are never auto-promoted.
Whole-file replacement space/I/O, deferred original CRC and untested power loss
remain visible boundaries.

The CLI creates linked or embedded documents through existing commands, prepares
only overview representations, saves, and later opens them in a native window.
Product metadata open and stat/read/decode/cache work run on workers. Two workers
admit at most 16 outstanding jobs/results, CPU RGBA 64 MiB; process-owned repair
SSD 512 MiB reuses the existing cache trim policy under a worker mutex. Stable
asset/revision/generator cache names survive CPU eviction without growing disk
with repeated requests. Preparation can fail if its complete generated set
exceeds the quota; this never overwrites the target. No large-board import UI is
claimed. Missing/changed/foreign/unavailable requested sources remain explicit;
valid last-known previews may still display. There is no synchronous content hash,
watcher, automatic rebinding or product high-LOD decoder.

GPU accepts Copy query metadata, bounded decoded overview pixels, and typed
AssetId+revision keys. Common quad generation honors center/size/rotation, flip,
crop, opacity and Default/Smooth/Nearest sampling with shared textures. It knows
no storage/source/path concepts and does no I/O. Product residence is rechecked
after all uploads; final coverage uses actual residency. CPU query remains the
ordered O(n log n) scan behind `DocumentQuery`, measured at 1k/5k/10k. Initial
camera coordinates are clamped to the existing ±1e8 camera limit with an explicit
report flag; valid document coordinates farther out remain preserved but are
not fully navigable in this slice.

Native open is demand-driven: ordinary idle sleeps after supply/completions;
zero-size/occluded windows stop redraw. Scripted traces intentionally keep a
bounded timer to reach their deadline. Shutdown drains already requested assets
and GPU work outside event/navigation callbacks before report export. Source
counts apply to requested assets only. Product first-content timestamps are CPU
submission before present; frame/present/GPU pass durations remain distinct.

The remainder records the retained benchmark foundations. Their u32 manifest
identities, multi-LOD loader, cancellation, pressure scenarios and performance
telemetry remain explicitly benchmark-only; they are not a competing product
object model or evidence of implemented product selection/high-LOD interaction.

## Document and interaction foundations

`tack-core` has no dependencies. Distinct nonzero 128-bit DocumentId/ObjectId/
AssetId/SourceId values describe private document tables. Image objects reference
assets, assets reference embedded/linked source descriptors; sources contain no
pixels. Commands validate references, metadata limits and order before mutating.
Validated transforms/crop/opacity have private durable fields. Camera remains
local view state, and Lod/ByteCache remain independent streaming policies.

DocumentEditor exclusively owns its document and explicit capped inverse-command
history. Renderer queries only return immutable Copy image metadata, with no
source resolver, I/O, decode, hash or locks. Ordered visibility is initially an
allocation-free O(n log n) scan through BTreeMap identities, not a production
spatial-scale claim. A later measured spatial index can remain behind this query
contract. The old manifest snapshot is explicitly named BenchmarkBoard and
BenchmarkImage: its u32 asset/cache keys are not product identities. The GPU benchmark still draws that validated axis-aligned snapshot; the separate
product composition now renders `DocumentQuery` objects through the common GPU path.

`tack-app` owns Action/Tool, immediate action dispatch, Keymap, normalized physical
events and InputState. The map has at most 256 bindings, assigned held inputs at
most 32; no event queue. Temporary tool gestures capture their semantic action
and opaque token through modifier/keymap changes, auto-repeat, overlapping holds
and focus loss. Pan holds follow current modifiers, preserving late Alt. Unassigned
inputs retain no state; invalid/excess native input is dropped without closing
the window. Menus and shortcuts share Undo/Redo dispatch through DocumentEditor.
Only existing prototype pan/zoom defaults are shipped, reverified against PureRef.

Source size is not working-set size: document/history retain metadata, never
source or decoded buffers. Storage/source I/O is owned by the separate 1B storage/assets adapters. Portable IDs,
unknown newer data retention, non-destructive old-writer behavior and migrations
are specified for Phase 1B in the [compatibility contract](design/tack_document_compatibility.md).
See [the Phase 1A report](MISSION_1A_REPORT.md) for checks and regression evidence.

The static board is an immutable validated snapshot. A linear visibility scan
is measured at 1,000 objects; it is not an arbitrary-board scalability claim.
The same GPU renderer serves the native window and offscreen target. Renderer
code never calls filesystem or codec operations.

## Supply and ownership

Visible demands are ordered tiny → medium → detail, followed by optional tiny
prefetch. Already cached/pending keys are deduplicated. Two workers are default;
only 1, 2 and 4 are accepted. ID modulo worker count gives stable ownership.
Benchmark-only overview preparation shares this pool, admits visible demand
first, and allows at most one background thumbnail per worker within the same
total. Remaining board preparation follows stable ID order; the measured
near-viewport sorting variant was rejected and removed.
Each worker's active accounting includes execution, queued jobs and unpublished
results: at most eight **total** tiny requests, or one medium/detail request.
Large requests only enter an empty worker and prevent further publication until
the outcome is polled. Both channels have capacity eight; capacities are not
added to the eight-request invariant. Frame polling never blocks.

Every demand update marks obsolete jobs cancelled. Workers check at queue,
source read/header, decode, resize, encode and result boundaries. A codec in
progress is not preemptible. Stale encoded output is not written to disk when
cancellation was detected after encoding. Failed or oversized display keys are
suppressed while desired, preventing repeated expensive rejection.

The retained 128 tier uses the safe `NativeThumbnail` boundary backed by
turbojpeg 1.5.1 / statically linked libjpeg-turbo 3.2.0. Each job owns a fresh
native decoder, borrows immutable capped input and writes scaled RGB directly
into a checked Rust allocation ≤2 MiB. Native scratch is separate and depends
on full dimensions/progressive coefficients; see [the decoder review](research/native_thumbnail_decoder.md).
Medium/detail continue to use jpeg-decoder 0.3.2 with optional Rayon disabled.
Both paths validate metadata and use DCT scaling followed by the same image
thumbnail resize and RGBA conversion. Only supported lossy RGB8/grayscale8
JPEG inputs are accepted by the thumbnail path; rare subsampling, CMYK,
arithmetic and lossless inputs are recoverable errors. PNG is
the disposable display-cache format. Display levels have longest edges 128,
512 and 2048; lower levels remain available during refinement. Texture selection
is recomputed after all uploads so an evicted detail can fall back to its retained
tiny image. No tiling, EXIF/color management or production media import is claimed.

The event thread owns the CPU byte caches of Arc<Decoded>. Workers keep another
Arc for each handed-off pixel buffer; before the next job they release those
that have no other owner. Thus eviction drops a cheap reference on the event
thread and returns large allocations on workers. Idle workers can retain evicted
buffers until the next request or shutdown; this is bounded and visible in RSS.
Worker-retention peak counters overlap CPU payload and represent a sum of
individual historical peaks, not simultaneous extra allocation.

## Bounds and accounting

| Resource | Selected policy |
| --- | --- |
| Manifest | ≤4 MiB; ≤10,000 objects |
| Workers | 2 default; explicit comparisons 1/2/4 |
| Pending | ≤8 × worker count, tiny batches; large jobs alone |
| CPU display payload | 64 MiB = 32 MiB tiny + 32 MiB refinement |
| GPU display payload estimate | 128 MiB = 64 MiB tiny + 64 MiB refinement |
| Pressure CPU/GPU | 16 MiB (4+12), 24 MiB (12+12) |
| Disposable SSD | 512 MiB across four fixed 128 MiB shards |
| Encoded source read | ≤64 MiB + oversize sentinel |
| Source dimensions | ≤6000×4500, validated before JPEG decode |
| Native thumbnail RGB output | ≤2 MiB, checked before allocation; progressive scan limit 100 |
| Medium/detail codec output hint | 192 MiB/worker; neither path's output bound limits all scratch |
| Upload | ≤8 textures and ≤16 MiB RGBA/frame |
| Submissions | ≤3 outstanding; saturated renderer skips submission |
| Telemetry | 7,200 frame/GPU samples; 20,000 job profiles/coverage episodes; 256 platform events |
| GPU readback | Three fixed asynchronous timing slots; busy slot drops sample |

Memory budgets count payload, not RSS or actual driver VRAM. Source Vec capacity,
codec coefficients (up to roughly 162 MB for this dimension limit), RGB output,
resizer/encoder scratch, unpublished results, retained worker references and
telemetry contribute to process RSS. DCT 1/8 output is much smaller without
eliminating entropy decoding or all progressive JPEG coefficient storage.
Per-job telemetry observes output/source/resize buffers, not allocator internals.
Source counters use byte length; cached encoded input uses a conservative
two-times-length estimate. These are not exact allocation high-water marks.
The report keeps RSS separate rather than subtracting a misleading exact total.

GPU accounting excludes staging, aligned allocations, fixed vertex/timestamp
buffers, placeholder, swapchain and driver overhead. Capacity is checked before
texture creation/write. At most three outstanding submissions and bounded
upload bytes prevent an unbounded backlog of staging and evicted textures.
No claim that 128 MiB payload means 128 MiB physical VRAM is made.

Fixed SSD shard identity is ID modulo four, independent of worker count. Powers
of two give exclusive shard ownership without filesystem locks. Startup creates
and trims all four shards before the frame loop. Warm hits do not repeatedly
scan directory metadata; writes evict refinement entries first, then thumbnails,
oldest-written first. Temporary write then rename publishes entries. Corrupt
cache entries fall back to originals; an unavailable cache disables itself.
Manifest hashes provide identity; this prototype trusts them without rehashing
originals. Multiple simultaneous processes sharing one cache are not supported.

## Presentation and evidence

Native acquisition, present and redraw callback are separately timed. GPU
queries cover only the canvas render pass, not texture uploads or monitor
latency. CPU frame timing includes scheduling, GPU polling, upload submission,
visibility and coverage telemetry, but excludes native acquisition/present.
No frame path performs source/cache I/O or waits for workers. Startup and final
report writing are explicit I/O boundaries; shutdown GPU draining is bounded
to five seconds. Worker shutdown signals cancellation and detaches rather than
waiting for an uninterruptible codec on the event thread.
Scripted report export first cancels/drains asset work outside the event loop
(30-second deadline), so subsequent warm-cache cloning does not race an active
write. The 12-second navigation metrics exclude this separately reported cost.
Preparation progress consumes validated hit/committed write profiles and
survives RAM eviction. It is historical, not a live persistent-cache inventory;
see [the preparation experiment](research/overview_preparation_experiment.md).

Interactive redraw coalesces input and sleeps when demand/loading finishes;
zero-size/occluded windows skip acquisition until restoration. Scripted traces
fix dimensions and camera speed without consulting loader progress. Adjacent
pan is 320/1280/3840 screen pixels per second at zoom 0.1. Zoom traversal is an
8-second logarithmic cycle from 0.006 to 0.6; combined scan adds fixed adjacent
motion. The old 20 distant jumps/s trace remains a backpressure torture test.
Board-tour spans the width and traverses every row in 12 seconds, returning in
another 12. Cache preparation uses this ordinary loader, never preloaded RGBA.

Episode metrics record first submitted lower image and first requested LOD,
with unresolved episodes explicitly censored. Source/binary snapshots and SHA256
are retained in ignored raw results; compact summaries are tracked under
benchmarks/. Synthetic hardlinked JPEGs, OS page-cache reuse, single trials and
lack of artist-rated usefulness limit product conclusions. See
[the decoder report](MISSION_0_6_REPORT.md), including the increased CPU p99
under faster cold supply and the remaining native presentation outliers.
The current [preparation gate](MISSION_0_7_REPORT.md) charges first-open work
before interpreting prepared-navigation coverage.
