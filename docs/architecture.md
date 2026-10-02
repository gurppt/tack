# Mission 0.5 architecture

Four crates retain the renderer/document separation established in
[Mission 0](MISSION_0_REPORT.md): core geometry/LOD/byte-cache policy, assets
(manifest, scheduling, decode and disposable caches), render (textures and
submissions) and app (native input, traces and evidence). There is no production
UI, document persistence, server or collaboration implementation.

The static board is an immutable validated snapshot. A linear visibility scan
is measured at 1,000 objects; it is not an arbitrary-board scalability claim.
The same GPU renderer serves the native window and offscreen target. Renderer
code never calls filesystem or codec operations.

## Supply and ownership

Visible demands are ordered tiny → medium → detail, followed by optional tiny
prefetch. Already cached/pending keys are deduplicated. Two workers are default;
only 1, 2 and 4 are accepted. ID modulo worker count gives stable ownership.
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

JPEG sources use jpeg-decoder 0.3.2, with optional Rayon disabled, metadata
validation, DCT scaling followed by image's thumbnail resize and RGBA conversion.
Only RGB8/grayscale8 JPEG source inputs are accepted in this prototype. PNG is
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
| Codec output limit hint | 192 MiB/worker; does not bound all codec scratch |
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
[the measured report](MISSION_0_5_REPORT.md).
