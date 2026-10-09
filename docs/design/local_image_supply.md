# Local image supply, Phase 2A2

The product renderer asks for projected display need: 128px overview,
512px medium or 2048px detail. The need includes crop fraction and stops at
source resolution. Smooth/Nearest remains an image sampling decision. No
source read, stat, decode, PNG encode or cache maintenance runs in a redraw.
Ordinary bounded PNG can decode a small frame; larger PNG streams rows, with
regional tiles still opt-in. JPEG uses the existing scaled decoder for ordinary
sources and bounded native regional supply for huge sources. Demand can gather
neighboring tiles into one operation and reuse visited raw tiles after reopening.
No operation allocates a full 50k² decoded raster or prepares a full pyramid.
See [huge-image supply](huge_images.md) and the
[Phase 2A2 report](../MISSION_2A2_HUGE_RASTER_STREAMING_REPORT.md).

## Admission and identities

A pure, deterministic plan admits detail by decreasing projected need, with
stable SourceId ties. Its conservative square RGBA reservation cannot exceed
half the GPU budget. An unadmitted refinement falls back to overview; it is
resolved at that admitted quality and cannot cause endless redraw/eviction.
Overview edge decreases through 128/64/32/16/8 on dense views, reserving room
for sixteen near-view candidates. The other half of GPU residency holds these
previews. Requests for all 10,000 visible supported image objects can fit.
The renderer's existing 10,000-visible-image ceiling still applies.

Stored overviews are asset-specific, so overview keys contain AssetId and
edge. Refinement keys contain SourceId, revision and edge; several objects or
assets referencing one source share the same CPU/GPU detail. Distinct sources
with coincident size/mtime are not conflated. No new content-hashing preload.

Only schema-compatible 128px derived previews enter PreparedOverview/Save.
Higher tiers and adaptive tiny previews are derived cache data, never document
or recovery authority. After uploads/eviction, selection retains the finest valid
resident for the current source/revision, preserving the Phase 2A1 quality-valley
fix. If no adequate tier is resident, the best lower fallback is shown while
current demand stays admissible. Small Nearest sources at most 512 pixels can
request admitted native Medium pixels below the overview threshold.

## Fairness and distant jumps

Demand ordering is large visible previews, smaller visible previews, visible
refinements, sixteen near-view previews. Background generation is an explicit
lowest class and is disabled during ordinary local idle. CLI repair remains
an explicitly requested background preparation operation.

The main-thread queue is replaced each redraw, at most 16 total queued plus
active representation keys (four potato). A JPEG regional batch can contain
multiple keys, and each counts against that cap. There is no hidden FIFO behind
a worker: each has one admitted active operation and one result slot. With two
workers, worker zero is reserved for previews and the other serves remaining
sorted demand. A running
codec is not interrupted unsafely; its completion must match current source
revision and current wanted representation before CPU/state publication.
Obsolete completions are counted, never uploaded. Pending descriptors from the
old viewport disappear without waiting for a synchronous flush.

For an eligible JPEG, at most 15 currently demanded neighboring tiles at one mip
form a contiguous rectangle or band. Aggregate output, including one-pixel
gutters, and the native crop footprint charged as RGBA are each at most four
MiB. Scanlines scatter directly into individual tiles; the rectangle is not a
temporary full bitmap. Cached holes are excluded from new regional decode.
There is no off-screen ring. Overlapping pan preserves an active batch while any
of its keys remains wanted; a distant jump or zoom-out cancels it when all are
obsolete. A valid coarse image remains until every admitted tile for the object
is resident, and zoom-out returns immediately to appropriate overview demand.

A single huge active decode can delay one worker; it cannot monopolize both
preview lanes indefinitely. Single-worker potato mode deliberately accepts
one active-codec delay. Missing representations have revision-specific negative
states and settle instead of retrying forever. Hidden/zero-size views and
shutdown revoke queued demand and drain only already active codecs.

## Budgets

| Budget | Default | Potato |
| --- | ---: | ---: |
| Owned CPU display pixels | 64 MiB (16 preview / 48 detail) | 8 MiB (2 / 6) |
| GPU cache texture payload | 128 MiB (64 / 64) | 16 MiB (8 / 8) |
| Derived SSD cache, combined | 512 MiB: 64 persistent raw + 448 per-open | 8 MiB: 4 persistent raw + 4 per-open |
| Decode workers | 2 | 1 |
| Queued + active requests | 16 | 4 |
| Result slots | 1 / worker | 1 |
| Upload payload / frame | 16 MiB | 1 MiB |
| Upload count / frame | 8 | 2 |
| Maximum refinement | 2048 | 512 |

CPU/GPU LRU caps count owned pixel/texture payloads, not process RSS, allocator
metadata, originals, codec scratch or driver allocations. At most two decoded
operation results can wait outside CPU LRU ownership; a regional result contains
at most four MiB of guttered tiles. Ordinary source input and UI import/relink
are bounded at 64 MiB; streamed PNG/JPEG work has a separate 256-MiB cumulative
encoded-read ceiling. Normal PNG's codec allocation hint remains 192 MiB;
streamed row/parser limits and native baseline JPEG's checked
`width × 192 + 1 MiB ≤ 16 MiB` scratch envelope are separate bounds.
Up to three GPU submissions may retain evicted resources until completion.
These values must not be described as a hard total-process/driver-memory cap.

SSD maintenance happens on workers, including quota enforcement on reopen.
The temporary `tack-product-open-*` overview/repair cache retains high-LOD-first
eviction and is retired on close. The profile's `raster-cache-v1` additionally
keeps visited raster detail in `tile-detail-v1.raw`, with simple persistent LRU.
It receives `min(total_disk / 2, 64 MiB)`; the temporary cache gets the remainder.
A cache-write failure does not discard successfully decoded display pixels.

Raw storage uses a checked 16-byte global prefix (`TACKR001`, little-endian
266,384-byte slot size, CRC) followed by fixed slots: 128 checked header bytes
and at most 258² × 4 RGBA bytes. Headers and padding count against quota. Startup
reads the prefix and at most 251 slot headers, never a directory of theoretical
tiles; payload CRC is checked lazily on hits. File length is
`16 + allocated_slots × 266384`; slots grow only for visited demand.
Four MiB holds 15 slots. All cache I/O and recency writes stay on image workers.

Its 80-byte key combines a 128-bit truncated SHA-256 namespace of document ID
and canonical linked/shared source path, SourceId, revision, actual 20-byte
size/mtime fingerprint, embedded-original CRC, source dimensions, tagged mip/tile
address and generator. The namespace distinguishes document copies with equal
IDs/stat values but different resolved relative sources without hashing source
contents. Actual identity is checked before and after reuse/derivation, even if
the document has no recorded fingerprint. Canonical originals remain authority.

One stable `tile-detail-v1.lock` lease covers the profile cache and its workers;
another owner simply disables optional reuse. Unsafe paths are rejected before
mutation, with Unix hard-link/private-permission checks and a portable global
prefix check before any existing raw-file write or truncation. Invalid/empty
files remain untouched. Global corruption disables reuse until deletion/reopen;
bad slots are regenerated. Missing, deleted, corrupt or unavailable cache state
falls back to source decoding. There is no database, mmap, daemon or cache
dependency in document save/recovery or the headless server.

The release receipts under `benchmark-results/phase2a2/` separate costs:
`region-comparison-release.json` reduces twelve deep JPEG tiles from twelve
helpers/7.623 s to one demanded rectangle/0.629 s; `cache-comparison-release.log`
trials measure roughly 0.10-ms raw hits versus 0.25–0.33-ms PNG hits with warm
OS pages. `square-final-{default,potato}/summary.json` observes six reopen tile
hits with zero extra regional jobs/source reads. Display payload is roughly
six MiB, while whole-process HWM is roughly 360–367 MB, including other runtime
and graphics state. These Linux/RTX2060 800×600 measurements do not establish
old-CPU performance or exact codec allocations; complete regression/acceptance
evidence belongs in the Phase 2A2 report.

## Large-board query

Native measurements found repeated ordered BTree lookups cost about 12ms p99
at 50k objects, versus about 1ms at 5k. For boards above 4096 objects, a lazy
ordered bounds memo keeps only ObjectId + AABB. This is a contiguous O(n) scan,
not a spatial index or a duplicate document. Only resulting candidates need
render metadata lookups. Smaller boards keep the allocation-free baseline.

Rows follow document order. Selected objects remain candidates before applying
live gesture previews, so movement into the viewport is not lost. Doc identity,
edit generation and document replacement invalidate the memo, including Restore
and Save As. Rebuild is synchronous O(n log n), once per committed generation;
select-all can remove its benefit. At 50k it owns about 3 MiB, and is bounded by
the document's existing 100k-object authority. Stable pan gains must not be
claimed as constant-cost edits or proof of 50k production readiness.

## Optional diagnostics

`--output REPORT.json` retains bounded raw frame, event and GPU-pass samples.
Query, supply, scene, encode, queue-submit, nonblocking poll, acquire and present
have separate fields. Callback is inclusive; `cpu_ms` is also inclusive of
query/supply/render. Do not add these inclusive fields to their components.
Worker `decode_ms` measures total load (I/O, decode, encode, cache), not codec-only.
The historical `source_bytes_before_detail` field now includes all linked-source
reads, including refinement; use that scope when comparing prior missions.
Likewise, historical `overview_reused` counts successful display results without
a new PreparedOverview, including refinements; it is not an overview-only count.
`--present-immediate` is a diagnostic AutoNoVsync comparison, not monitor latency.
Ordinary product windows disable timestamp/readback profiling and have no
persistent profiling framework. No telemetry or network client was added.

## Updated 1G source fairness and physical-window measurements

Preview urgency and representation need both use the longest projected edge,
including portrait images. The bounded view queue admits at most two ordinary
jobs per SourceId; known JPEG tile demand is exempt from that per-source queue
limit so it can form useful batches within the unchanged global 16/four-key cap.
Dispatch admits only one active operation per SourceId, and reserves worker zero
for previews in two-worker mode. There is still one result slot per worker.
Remaining wanted aliases enter on subsequent demand updates; this can serialize
thumbnail/refinement work on a single source. No extra fairness thread or
background preparation scheduler is introduced.

Developer `open/new FILE --window-size WIDTHxHEIGHT` starts at a bounded physical
size (320x240..8192x8192). Actual OS-granted size drives surface and camera; each
optional report/frame records it. Ordinary launch remains 1280x720. Native
800x600, 1024x768 and 1600x900 receipts distinguish viewport size from Xvfb's
1600x1000 desktop and from UI integer scale. No benchmark timer or dimensions
reporting is added to an ordinary untimed launch.

## Phase 1K convergence repair

Before insertion, touch only exact admitted GPU keys. The plan reserves enough
space for every unique wanted refinement and adaptive preview in its partition.
LRU therefore evicts older, obsolete entries before wanted residents. Optional
higher fallbacks are deliberately not protected, so memory pressure can reclaim
them. This changes neither budgets nor worker/queue/upload limits.

Upload all ready representations before selecting draws or computing resolved
quality. Phase 2A1 extends the convergence repair to retain the finest valid
current resident, then use the best lower fallback during refinement. Recompute
retry eligibility from the final GPU inventory. The
previous per-image accounting could declare a texture satisfied before a later
upload evicted it, then sleep with no refinement path.

Demand relevance uses source/revision/tier/edge, not camera epochs. A rejected
old completion cannot overwrite current pixels or clear a different successor.
Production keeps the existing active-supply wake path and sleeps after settle;
there is no new polling watchdog or global refresh/cache flush.

`open BOARD --lod-debug --output REPORT.json` opts into one debug source (the
first visible source, held for that receipt), physical projected width/height,
camera epoch, admitted/displayed/resident tiers, CPU hits, queued/active codec
worker, request generation, recent accepted/stale publication and suppression
reason. History is bounded to sixteen publication records and 7200 frame
receipts. Diagnostics have no file, timer, worker or heap allocation when off.
Normal supply statistics also count actually dispatched codec requests and
successful decoded RGBA bytes, including discarded outcomes, so measurements
count work rather than only the final cache payload. Wheel callback samples
are collected only by explicitly requested output receipts.

`tools/run_lod_zoom.py` owns the test-only convergence watchdog. It tests normal
untimed windows, progressive sweeps, rapid crossings, small pan and final idle.
Comparison runs use `--no-diagnostics` for equal instrumentation; diagnostic runs
are kept separate. Native generated fixture creation is the finite developer
example `lod_fixture`, with eight caller-owned 2048px PNGs; geometry includes a
group, rotation and alternating Smooth/Nearest. CPU tests sweep 1..4096 pixels
in both directions. Actual GPU tests recreate late eviction and reuse higher
residents while a lower refinement is delayed; ProductAssets tests withhold real
worker publication across demand and source revision changes.
