# Bounded image supply — Phase 2A2

Large static PNGs stream into small display products; PNG regional tiles remain
an opt-in prototype: `--huge-tiles`. JPEG uses overview-first 128/512/2048
representations and bounded local tiles by default. Phase 2A2 batches neighboring
JPEG demand and retains visited raw tiles within a fixed profile SSD quota.
Neither format allocates the full huge-source RGBA frame or constructs a
complete mip pyramid. Current acceptance and remaining limitations belong in
[the Phase 2A2 report](../MISSION_2A2_HUGE_RASTER_STREAMING_REPORT.md).

The implementation extends the existing source/revision supply, cache and
worker system. `png = 0.18.1` was already transitive; the direct dependency
provides header inspection, row decoding, metadata suppression and parser
limits. Its introduction added no package identity; the original PNG validation
and binary-cost measurements are in [the Phase 1L report](../MISSION_1L_REPORT.md).

## Routing policy and memory boundary

Classification starts with checked `width × height × 4`, independent of encoded
file size. The 32 MiB policy is a bound on live pixel buffers in the normal PNG
representation path. It is not a bound on total process RSS or every codec's
scratch allocation.

| Class | RGBA8 estimate | PNG behavior |
| --- | --- | --- |
| Normal | At most 16 MiB / 4,194,304 pixels | Full bounded decode, existing thumbnail filtering; combined pixel-buffer peak at most 32 MiB. |
| Large | Above 16 MiB, at most 32 MiB / 8,388,608 pixels | Stream rows into the requested display representation. |
| HugeTiled | Above 32 MiB | Stream rows for overview/detail; visible tiles only with `--huge-tiles`. |

These class names describe source risk and capability. They do not mean that
all HugeTiled sources automatically request tiles. A 50,000-pixel axis is not a
classification threshold.

Source channel count and precision can override the Normal route. The decoder
also checks `pixels × max(source_bytes_per_pixel, 4) × 2`: if this exceeds
32 MiB, it streams even when the RGBA8 class is Normal. For example, RGBA16 has
an eight-byte source pixel. Palette expansion is included in the four-byte
floor. A PNG wider than 6000 or taller than 4500 also uses rows, including a
very wide source with few pixels. Normal output is normalized to eight bits;
16-bit source precision is not retained in display products.

Explicit, separate admission limits are:

- Nonzero dimensions; each axis at most 262,144; at most 4,294,967,295 pixels.
- At most 2 MiB for a raw scanline and for the normalized caller-owned row.
- An 8 MiB `png::Limits` parser allowance; text and ICC payloads are ignored.
- At most 16 MiB for one streamed display output, or 256 KiB for a full tile.
- At most 256 MiB cumulative encoded reads per PNG representation operation,
  including repeated seeks; input is never buffered as a whole.
- Interlaced or animated PNGs are rejected when the streamed route is required.
  Bounded Normal PNG handling preserves its existing first-frame behavior.

`png::Limits` is best effort and does not count every caller buffer or internal
unfiltering buffer. Explicit row checks bound width-dependent storage;
checked output allocation bounds caller pixels. The parser allowance and these
rows are separate from the 32 MiB pixel policy. Exact native/PNG scratch peaks
are **not measured directly**; RSS and display-cache bytes cannot substitute
for that measurement. This is also why two workers do not imply a 32 MiB total
application working set.

## Crossover evidence and its limits

The measured comparison uses Linux, RTX 2060, native 800×600 windows and the same
generated linked boards. The following receipts illustrate why a conservative
pixel boundary was chosen; they do not identify a universal hardware crossover.

| Source | Old path peak process HWM / first overview | Row path peak process HWM / first overview |
| --- | --- | --- |
| PNG 4096×4096, 64 MiB RGBA estimate | 307.9 MiB / 464.8 ms | 280.4 MiB / 445.2 ms |
| PNG 6000×3000, 68.7 MiB RGBA estimate | 297.2 MiB / 462.7 ms | 257.2 MiB / 457.7 ms |
| PNG 50000×4096, 781.3 MiB RGBA estimate | Unsupported by old source-dimension policy | 236.8 MiB / 653.7 ms |

These observations are in `benchmark-results/phase1l/perf-baseline-png4k`,
`perf-baseline-png6k`, and `perf-final-axis-{4096,6000,50000}-png`. Their JSONs
record binary and board hashes. HWM includes graphics/runtime memory; the
overview timestamp is CPU submission, not confirmed display presentation.
The square crossover fixtures 1024², 2048² and 3072² probe below, at and above
the Normal pixel boundary. Complete final metrics and any affected reruns belong
to the mission report; do not infer exact decoder scratch from this table.

The measurements support avoiding full PNG frames above a modest fixed budget.
They do not prove that 32 MiB is an optimal cutoff on every machine, or that the
prototype is viable on a 128 MiB system. Potato mode checks stricter payload
bounds on the development host; it is not a Pentium III performance claim.

## Tile and mip geometry

A tile is 256×256 RGBA8 or smaller at an image boundary. Its identity includes
source ID, source revision, representation tier, mip and integer tile x/y.
The existing representation `edge` field carries a tagged address; the tag is
never used as an allocation size or texture dimension.

Mip dimensions use ceiling division by `2^mip`. The initial mip follows the
source/projected pixel density, with crop accounted for. Screen corners are
mapped back through rotation, flips and crop to obtain a conservative visible
source region. Edge tiles are clipped rather than stretched outside the source.
Only admitted visible rectangles are requested; there is no neighbor prefetch,
page table, sparse texture, atlas or allocation for theoretical tiles.

Tile capacity is the smaller of remaining detail GPU space, a quarter of the
CPU display budget, 64 rectangles, and remaining space in the 10,000 image-quad
limit. This gives at most 64 full tile slots by default and eight in potato mode,
often fewer after ordinary detail reservations. Eligible visible objects share
the remaining slots. If their initial mip needs too many rectangles, the planner
raises the mip until the entire visible region fits its quota.

The whole coarse image remains until every admitted tile for that object is
resident. Only then do its tiles replace that draw. This avoids partial coverage
holes and double compositing opacity over the fallback. Ordinary detail is
requested before tiles; two-worker mode reserves worker zero for overviews.
Dispatch permits one active operation per source. Ordinary representations retain
the two queued jobs per source limit; known JPEG tile demand can fill the unchanged
16-key global cap (four in potato mode) to form a useful regional batch. Each tile
counts as a key, including while its batch is active. An overlapping pan keeps a
batch useful if any of its keys remains wanted; it is cancelled once all become
obsolete. Distant jumps and sharp zoom-out discard obsolete queued demand without
waiting on a worker, while a valid coarse representation remains drawable.

## Codec behavior and prototype limitations

PNG is sequential. **Every uncached tile scans and decompresses the source from
the start through all rows and validates its tail/CRCs.** It retains only the
requested sampled output and bounded row buffers. This keeps memory bounded but
makes distant detail slow and amplifies source reads. A cached return within the
same process is faster. Row-level cancellation and cancellation during reads
discard obsolete work; late results cannot become current detail.

Streamed overviews and mips use nearest sample selection rather than a filtered
mip pyramid. This can alias fine detail. Tiles have no filtering gutters; Smooth
sampling can reveal boundaries. These quality and repeated-scan costs are reasons
to keep tiles opt-in. The prototype proves bounded selected demand and source
authority; it is not a production random-access image pyramid. Optional Bézier
arrows and endpoint attachments were deferred to protect this scope.

JPEG has **default bounded local tiles**, while PNG tiles remain opt-in.
Ordinary JPEG uses the existing safe reduced-DCT wrapper (64 MiB encoded,
32 MiB RGB output); progressive/multiscan coefficient scratch has a separate
192 MiB checked allowance. Larger JPEG routes to the pinned native `djpeg`
helper on image workers: reduced-DCT overview, streamed RGB rows and requested
256-pixel mip tiles. Each JPEG tile includes a one-pixel gutter for Smooth,
at most 258² RGBA pixels. Tile admission conservatively reserves that size.
Crop-adjusted demand also controls tile eligibility.

The large route accepts 8-bit interleaved single-scan baseline RGB/grayscale,
bounded header/input/row/output work, and the codec's own 65,500-axis limit.
Large progressive and split-component scans are explicitly refused before
native decoding. The accepted baseline native scratch envelope is checked
separately from Rust buffers, caches and process RSS. Every uncached region still
traverses the sequential entropy stream from the beginning; crop/skip saves
output and IDCT work without providing arbitrary entropy access. A regional
operation gathers at most 15 demanded tiles in a contiguous band or rectangle
at one mip. Aggregate guttered RGBA output and the native crop footprint charged
at four bytes per pixel are each capped at 4 MiB. Rows scatter directly into
tiles; the crop rectangle is not materialized. Cached holes are excluded and
new contiguous regions are grouped under the same admission checks. There is
no off-screen ring, complete frame, background preparation or persistent pyramid.
The worker's encoded JPEG read ceiling remains 256 MiB per operation, separate
from the 64 MiB ordinary decode and UI import/relink limits. Header, row and
native scratch limits remain independent of the four-MiB regional output.
See the [decoder boundary](jpeg_decoder_boundary.md) for exact limits, source
inspection, cancellation, packaging and scratch accounting, and the
[2A2 report](../MISSION_2A2_HUGE_RASTER_STREAMING_REPORT.md) for measured acceptance.

Ordinary representations now stop at native dimensions. Generator IDs 6/7/8
replace legacy upscaled PNG/JPEG/other cache products; JPEG tile products use
5, and streamed PNG remains 3. After upload/eviction, selection keeps the finest
valid resident for the current source/revision. Small Nearest sources at most
512 pixels request budget-admitted native Medium pixels even below the preview
threshold. Local Default sampling resolves the active preference before demand
and rendering. No LOD diagnostic or codec timer runs on a settled board.

## Shared runtime budgets

| Resource | Default | Potato |
| --- | --- | --- |
| CPU display payload | 64 MiB: 16 overview + 48 detail | 8 MiB: 2 overview + 6 detail |
| GPU display payload | 128 MiB: 64 overview + 64 detail | 16 MiB: 8 overview + 8 detail |
| Derived disk cache, combined | 512 MiB: 64 persistent raw + 448 per-open | 8 MiB: 4 persistent raw + 4 per-open |
| Decode workers | 2 | 1 |
| Pending + queued requests | 16 | 4 |
| Upload bytes per frame | 16 MiB | 1 MiB |
| Uploads per frame | 8 | 2 |
| Concurrent GPU submissions | At most 3 | At most 3 |

These are payload limits, excluding source input, scratch, driver allocations,
geometry and other application state. Oversized admission is rejected; caches
evict without pinning full huge detail. Codec completion wakes the event loop.
When demanded work settles, there is no recurring tile/cache scan, progress
redraw, file-stat loop or GPU submission.

## Authority, cache lifetime and failures

Overview/repair products keep the private temporary `tack-product-open-*`
directory, removed on normal close. Visited raster tiles additionally reuse the
profile's disposable `raster-cache-v1/tile-detail-v1.raw`. The persistent quota
is `min(total_derived_disk / 2, 64 MiB)`; the per-open cache receives the remainder.
Thus disk reuse reduces reopen codec work without increasing the combined quota.

The raw file starts with a checked 16-byte global prefix: `TACKR001`, the
four-byte little-endian slot size and a CRC of the preceding 12 bytes. Each
266,384-byte slot contains a checked 128-byte header and room for at most
258² × 4 raw RGBA bytes, including JPEG gutters. File length is
`16 + allocated_slot_count × 266384`; slots grow only as tiles are visited.
The 64-MiB ceiling admits 251 slots, and potato's four MiB admits 15. Startup
reads the prefix and at most 251 headers into a compact index; it never walks a
directory proportional to possible source tiles or reads every payload.
Payload CRC is checked on a hit, and simple LRU stamps persist in slot headers.
Opening, reads, recency writes, eviction and corruption handling run on image
workers, with no filesystem work or codec wait on the renderer/input thread.

Each 80-byte identity contains a 16-byte namespace (SHA-256 of document ID and,
for linked/shared originals, the canonical physical path, truncated to 128 bits),
SourceId, revision, actual 20-byte size/mtime fingerprint, embedded-original CRC,
source dimensions, tagged mip/tile address and generator. This distinguishes
copies of the same document whose relative links resolve to different files,
even when size/mtime match. Actual fingerprints are checked before and after
reuse/derivation, including when the document has no recorded fingerprint.
The namespace hash does not preload or hash the whole source.

A stable `tile-detail-v1.lock` provides one non-blocking cooperative lease per
profile, shared by its image workers. There is no database, mmap or daemon.
Unsafe paths are refused before cache mutation; Unix additionally rejects
multiple hard links and writable-by-other-users paths. On every platform an
existing raw file must have the valid global prefix before resizing or writing,
so an ordinary JPEG, PNG or board cannot be mistaken for disposable cache bytes.
Invalid/empty existing files are left untouched; global-header corruption
disables reuse until the disposable file is deleted and reopened. Slot corruption
is a miss and can be regenerated. An unavailable, deleted or separately leased
cache falls back to source decoding and does not discard successfully derived
pixels. Cache state is never required for reopening or saving a document.

Source ID/revision and linked fingerprint validation govern acceptance. Relink
and revision changes clear obsolete capability/state, change product keys and
reject stale completions. A missing, changed, corrupt or unsupported source may
leave an overview/placeholder with an explicit failure; a quiet failed source
must never be counted as successful detail convergence.

The `.tack` authority remains original encoded embedded bytes or linked source
identity/path, plus document transforms, crop and style. Tiles are not serialized
into the board. Save Original As streams those originals without decoding or
recompression; exported SHA-256 must match the original. Save/recovery/reopen
does not depend on retaining derived tiles.

## Reproducible fixtures and observations

Phase 2A2 adds a real deterministic 50,000×50,000 baseline grayscale JPEG,
33,425,392 encoded bytes, generated with bounded procedural rows. Its regional
release comparison is in `benchmark-results/phase2a2/region-comparison-release.json`:

| Twelve useful deep tiles | Helpers | Elapsed | Logical source reads |
| --- | ---: | ---: | ---: |
| One decode per tile | 12 | 7.623 s | 395,579,232 bytes |
| Three horizontal bands | 3 | 1.875 s | 98,894,808 bytes |
| One demanded rectangle | 1 | 0.629 s | 33,161,544 bytes |

The rectangle retains 3,195,072 output bytes and has a charged crop footprint of
3,354,120 bytes, each below four MiB. The comparison validates source-coordinate
pixels; it is a serial release measurement on the development host. A 15-tile
neighbor-column comparison takes 0.737 s without improving the twelve useful
pixels, so the product keeps demand-only gathering.

The release raw/PNG roundtrip receipt is
`benchmark-results/phase2a2/cache-comparison-release.log`. With warm OS pages,
64 alternating trials measure raw-hit median 0.110/0.104 ms versus PNG
0.247/0.334 ms for flat/noisy 256² tiles. Raw avoids codec work but reserves
266,384 bytes per slot; the flat PNG is only 1,915 bytes. These are host cache
roundtrips, not cold SSD latency or an old-CPU benchmark.

`benchmark-results/phase2a2/square-final-{default,potato}/summary.json` records
native Linux/RTX2060 800×600 fit/deep/pan/extreme-jump/return/zoom-out stages.
Fit and sharp zoom-out request zero tiles; both reopen returns read six raw
tiles with zero additional regional jobs or logical source bytes. CPU display
payload peaks at 6,971,744/6,172,976 bytes and GPU payload at 6,971,744 bytes,
while whole-process HWM is 366,809,088/362,577,920 bytes (about 350/346 MiB).
Thus roughly 360 MB process residency is distinct from roughly six-MiB display
caches, and neither is a measurement of every decoder allocation. Sampled child
HWM is about 2.3–2.5 MB; 50-ms sampling can miss short-lived peaks. Settled stages
record zero I/O and zero/one CPU tick. Remaining native/regression evidence and
the final acceptance decision belong in the mission report.

Reuse the existing `test_file/phase1l_generated` corpus. After additions, its
measured physical size is 411.719 MiB. The generator enforces a 512 MiB data
reservation and at least 10 GiB free, refuses pre-existing output, and journals
each owned path under a sentinel. It never cleans the owner's adjacent boards.

`stress_1000_paths` uses 25 JPEG payloads and hardlinks to 1000 distinct paths;
`stress_250_unique` uses 250 genuinely distinct JPEGs; `mixed_prod_1000` uses 96
payloads across 1000 paths and varied dimensions/layout. Hardlinks share physical
inodes and OS cache behavior. High-pixel-count JPEG fixture generation uses
grayscale where Pillow's RGB storage would exceed 64 MiB, including the nominal
6000×3500 stress sources. These are procedural references, not a photographic
production bank or square 50k-image benchmark.

The axis fixtures isolate JPEG/PNG sources at 4096, 6000, 8192, 12000, 16000,
24000, 32000 and 50000 pixels, with documented bounded heights. Additional
boards exercise a single giant map, separated huge PNGs, deliberately overlapping
huge PNGs, a giant plus 100 ordinary references, and a compact giant-plus-four
visible-reference layout. Source bytes are reused between these boards.

For a new corpus only, after inspecting `plan` and available space:

```sh
python3 tools/generate_phase1l_corpus.py plan
python3 tools/generate_phase1l_corpus.py generate
cargo build --release --locked -p tack-app --example phase1l_corpus
python3 tools/generate_phase1l_corpus.py boards --builder target/release/examples/phase1l_corpus
du -s -B1 test_file/phase1l_generated
```

The example trusts controlled manifest dimensions and writes metadata-only linked
boards without preparing previews. This allows the old binary to encounter its
own unsupported-source boundary. Exit status alone is not performance evidence.

The native observation tool requires an isolated X11 display explicitly owned
by the caller. With an already running owned display `:99`, for example:

```sh
DISPLAY=:99 TACK_NATIVE_NO_WM=1 python3 tools/run_phase1l_supply.py \
  --binary ./bin/tack \
  --board test_file/phase1l_generated/boards/single_huge.tack \
  --output benchmark-results/phase1l/manual-huge --tiles --zoom
```

Use a new output directory for each run. Add `--potato` for strict mode. Run
measurements serially with no concurrent builds/corpus generation. The tool
records raw frames, process observations, screenshots, board/binary hashes and
a summary. Its external quiet detector is not a LOD assertion: inspect requested
versus resident tiles, desired versus resolved detail, errors and actual visible
source counts. The final harness also records p99; older summaries provide p95, so the compact
receipt computes p99 from their raw frames.

After review, generated fixtures alone can be removed with:

```sh
python3 tools/generate_phase1l_corpus.py cleanup
```

Cleanup validates the sentinel, complete ownership list, path types and symlink
ancestry before removing any entry; unowned additions cause refusal. Save any
fixture edits worth keeping outside that generated subtree before cleanup.
Never remove the adjacent owner `test_file/test_file.tack` or `gfx/` assets.

Subjective detail/interaction review remains separate from technical acceptance.
Unsafe allocation, corruption and major regressions remain technical blockers.

Worker publication validates actual ordinary raster dimensions against admitted
asset axes and edge. Incorrect cached products are repaired and inconsistent
source metadata fails once without repeated publication. Shared source aliases
reserve the maximum declared dimensions and independently peek current CPU/GPU
resident dimensions, so a small alias cannot undercount a larger cached image.
All streamed jobs and tile jobs carry cancellation, including narrow JPEGs whose
pixel count alone classifies them below the huge-image tier.
