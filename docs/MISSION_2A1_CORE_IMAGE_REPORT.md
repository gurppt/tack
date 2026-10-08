# Mission 2A1 — core image corrective pass

Large baseline JPEGs now use bounded overview and visible-detail supply, including
the existing 50,000-pixel panorama. LOD selection retains the finest valid
resident pixels across zoom thresholds; small Nearest references request native
pixels without enlarging the decoded representation beyond the source size.

**Technical PASS — 2026-10-09.** Implementation commit
`5c2c0bb90ca5533c13b75634696dd6f7b96f3c7b` is pushed. Full local gate, explicit
RTX2060 GPU tests, native long-churn witness/corrective comparison, 48 paired
ordinary runs, 32 native Phase2A assertions, packaging checks and exact-source
Linux/Windows CI pass. Independent review is PASS. Subjective desktop feel is
queued separately; no technical image-supply blocker is left open.

## Implementation and authority

The existing core model, renderer, source/revision keys, bounded ProductAssets
workers and CPU/GPU/derived caches remain the common local/shared path. Original
JPEG bytes and linked descriptors remain document authority. Derived tiles are
reconstructible display products, never serialized originals or server state.

The [JPEG boundary](design/jpeg_decoder_boundary.md) extends the already pinned
libjpeg-turbo 3.2.0 stack through its unmodified packaged `djpeg` executable. The
safe wrapper exposes reduced DCT and header checks but not bounded scanline or
crop/skip output. A short-lived helper on an admitted image worker provides those
operations without adding unsafe Rust, a decoder framework, a new Cargo package
or a permanent service. It is resolved beside the executable, not through PATH.
`tack-server` neither loads this image crate nor starts the helper.

Routing checks pixels, components/precision, encoded reads, RGB/RGBA output and
native scratch separately. Large eight-bit interleaved single-scan baseline
RGB/grayscale sources stream a reduced-DCT overview first, then only requested
mip/tile outputs. Each tile includes a filtering gutter and is at most 258² RGBA
pixels. Current cache/request/upload budgets still govern admission. Oversized
progressive or split-component sequential scans fail explicitly before native
decode; bounded ordinary progressive images retain their checked scratch route.

A complete sequential pyramid preparation was evaluated but not introduced. The
cropped scanline route was sufficient for the measured panoramas while avoiding
unrequested pyramid construction and writes. **An uncached JPEG region still
traverses the sequential entropy stream from the beginning.** Crop/skip saves
IDCT/output work; it does not provide arbitrary random access. Cached regions
reuse bounded derived products during the same open session. The per-open cache
is disposable and removed on normal close; reopen reconstructs detail.

The source-derived baseline native scratch envelope is `width × 192 + 1 MiB`, checked
against 16 MiB, separately from Rust source/row/output allocations. The pinned
single-scan native path uses MCU-local coefficient handling. Process or child
RSS is not proof of that bound. Active operations own bounded feeder/watchdog
threads and cancellation/deadline handling; after settle, no decoder helper,
JPEG watchdog or codec timer remains. Exact limits and upstream source rationale
are recorded in the boundary document.

## LOD quality valley

The previous policy chose the smallest adequate *tier label*. A newly selected
filtered Thumbnail could therefore replace already resident native pixels when
the camera crossed the 128-pixel band. Nearest sampling cannot restore detail
already discarded while generating that preview. Short label-only convergence
checks considered this adequate even when a visibly better valid resident existed.

A second issue enlarged ordinary PNG/JPEG representations beyond native source
dimensions. This wasted payload space and introduced filtered pixels into small
references. Enlarged historical cache products could also violate a new admission
estimate based on actual source size. Generator IDs 6/7/8 replace those historical
PNG/JPEG/other products; the old enlarged caches are excluded from current repair
lookup. Streamed PNG remains generator 3 and JPEG tiles use generator 5.

Selection now considers the current source/revision's post-upload inventory and
retains the finest valid resident. Budget admission uses bounded native dimensions,
with maximum dimensions across aliases of the same source and actual CPU/GPU
resident axes. A small alias cannot undercount an already cached larger raster.
Workers reject inconsistent new ordinary raster sizes once and regenerate
incorrect disk-cache products before publication. Small Nearest sources
at most 512 pixels refine to budget-admitted native Medium pixels even below the
preview threshold. Valid previews remain loading/pressure fallbacks. Default
sampling resolves the active local preference before demand and drawing.

Opt-in `--lod-debug` receipts expose projected demand, selected/resident/pending
tiers, actual CPU/GPU dimensions, revision, request generation and stale-result
publication. Ordinary execution allocates no diagnostic owner or recurring timer.
This reproduces and fixes the owner's **quality-valley class**; it does not claim
to have recovered the exact original many-hour session or its historical cache.

A final native witness uses the same 256×128 pixel-art source plus the huge
JPEG. The Phase2A binary records 79 frames violating best-resident selection:
settled Nearest/Smooth stages at 99.18 projected pixels display Thumbnail while
Medium is still resident. The corrected binary records zero counterexamples
across 263 churn traces and 17 reopen traces. Nearest retains native 256×128
Medium pixels; Smooth requests Thumbnail at that demand but displays valid
resident Medium. Each run sends 2,520 wheel events, 32 pan actions and five
filter changes. Native event coalescing is explicit: these are input events, not
2,520 completed decodes or cache evictions. Actual pressure/eviction coverage
comes from the separate Rust worker and GPU tests.

Focused checks observed:

- Supply-policy tests pass, including native pixel-art admission and zoom
  threshold/reversal behavior.
- The worker harness cycles 16 actual source identities through 1,200 views: at
  least 2,400 completed representations, over 2,000 CPU evictions, bounded residency
  and exact saved/reopened canonical metadata. Historical enlarged-cache
  readmission and delayed source/revision publication have explicit regressions.
- The explicit GPU harness performs 1,200 real eviction/reentry cycles,
  12,000 band comparisons and 200 byte-exact readbacks. Nearest and Smooth,
  crop/rotation/flip and stale source revisions retain the correct current pixels.
  Grouped native interaction remains a separate final receipt.

## Serial JPEG measurements

The compact [measurements](measurements/phase2a1.json) are regenerated by
`tools/summarize_phase2a1.py`. Full frames, process samples, source manifests,
board/binary hashes and screenshots remain under `benchmark-results/phase2a1/`.
The accepted 13-run `*-v2` matrix used binary SHA256
`c9f7e4eae976ab6f49b8cc012cf1d7273848385187b61b62d2553aeca30a43f5`.
Final-source replays use implementation commit
`5c2c0bb90ca5533c13b75634696dd6f7b96f3c7b` and raw client SHA256
`df026f025fe7c35e44d3994b0f0229e1f04e051ab15346ad8e9d1caa67060ab5`.
The earlier matrix remains a distinct measured identity. Later changes cover
preview metadata mapping, a policy-call refactor, publication/admission guards,
actual-resident shared-alias accounting and narrow-source cancellation.
Final replay receipts verify these changes separately.

Runs were serial Linux/RTX2060 native 800×600 on an owned isolated X11 display.
Page cache was warm or unspecified. Recognizable/detail timestamps are CPU
submission receipts, not measured physical presentation. Callback distributions
include presentation waits; GPU pass timestamps are recorded independently.
External quiet-stage timestamps and native frame timestamps have different
origins and are not joined by nearest wall-clock comparison. Stable camera,
zoom, tile and decode counters provide the stage evidence instead.

| JPEG source | First recognizable | Peak process HWM | Peak CPU/GPU display payload |
| --- | ---: | ---: | ---: |
|4096×2304|637.0ms|359.8MiB|9.60MiB|
|6000×3500|513.6ms|368.2MiB|9.96MiB|
|8192×2048|632.9ms|337.5MiB|2.04MiB|
|12000×1024|507.6ms|337.3MiB|2.12MiB|
|16000×1024|601.0ms|337.0MiB|1.85MiB|
|24000×640|592.8ms|336.9MiB|1.81MiB|
|32000×512|625.8ms|336.9MiB|2.05MiB|
|50000×512|615.2ms|337.5MiB|2.55MiB|
|50000×512potato|577.7ms|337.5MiB|2.83MiB|
|50000×512reopen|620.6ms|337.7MiB|2.55MiB|

These are specific bounded-height fixtures, not a 50k×50k source or arbitrary
high-area memory/performance promise. All 13 matrix runs have zero supply errors,
fully resolved final visible sets and zero idle redraws/I/O. Approximately
two-second idle observations record 0–2 CPU ticks. Native child RSS sampling is
around 2.4 MiB where observed, but 50 ms sampling can miss short-lived helper peaks.
It does not measure exact decoder allocation maxima.

For the 50k default run, cold overview precedes requested detail. The first fully
admitted visible tiled set is recorded at 2894.9 ms in the native clock. Stable
camera receipts show:

| State | Camera x / zoom | Ready/requested tiles | Codec jobs | Logical source reads |
| --- | --- | ---: | ---: | ---: |
|Cold overview|750 /0.4|0/0|3|6.04MiB|
|Deep detail|750 /64|6/6|13|22.59MiB|
|Far pan|764.0625 /64|6/6|17|30.14MiB|
|Exact return|750 /64|6/6|17|30.14MiB|
|Sharp zoom-out|750 /0.001|0/0|21|35.73MiB|

The exact return adds zero codec jobs or logical reads. Derived output reaches
421,343 bytes and stays unchanged through far-pan/return. Cold reopen succeeds
with a fresh per-open cache; warm operating-system pages do not imply persistent
derived tiles. Mixed huge JPEG+ordinary references, huge JPEG+huge PNG and multiple
huge JPEGs also converge within the existing worker/cache limits. Mixed PNG detail
remains its explicitly enabled prototype path. The independent review inspected
source patterns in actual cold/deep captures and checked the matrix's revision,
requested/resident and source-read evidence.

## Final verification and ordinary regression

The detached Phase2A baseline gate and final-source `gate-03.log` pass Rust
format/check/Clippy, workspace tests/docs, 26 Python tests and cargo-deny.
JPEG/Product library suites contain 28 tests each. `gpu-final.log` passes all
14 explicit GPU tests, including pixel readbacks and both short/long LOD churn.
The measurement aggregation's complete mode validates the final receipts and
binary identities; the final Python suite also passes after its changes.
The independent [review](reviews/phase2a1_core_image.md) records adversarial
scan-layout, scratch, cancellation, gutter, cache-generation, forged-dimensions,
shared-alias, source/revision and LOD cases and their verified corrections.

The final binary replays four representative huge-image cases:

| Final replay | First recognizable | Process HWM | Peak GPU payload |
| --- | ---: | ---: | ---: |
| final-50k | 496.0 ms | 337.52 MiB | 2.55 MiB |
| final-potato | 611.5 ms | 337.65 MiB | 2.83 MiB |
| final-color | 483.9 ms | 336.11 MiB | 1.32 MiB |
| final-mixed | 573.4 ms | 341.60 MiB | 6.65 MiB |

All four converge, return to the same detail camera without new codec work or
logical source reads, and finish with zero idle frames/I/O. Color uses a real
50,000×128 RGB 4:2:0 JPEG; the original panorama is 50,000×512 grayscale. Final
mixed detail is 10/10 requested tiles. Root process RSS includes GPU/driver state
and is distinct from the bounded raster payload. Four standalone decoder
`time -v` receipts measure process HWM of 2.25–2.50 MiB for overview/crop cases;
this is measured process high-water RSS, not a universal allocation proof.

The 36 same-host local comparisons cover six boards, three baseline/current
pairs each; 12 additional comparisons cover ordinary 4k/6k JPEGs. All 48 have
zero IP sockets, zero idle redraws and zero idle I/O. Local-only windows start no
LAN/shared worker. The owner board is opened read-only; originals are preserved.

| Board | First recognizable median, 2A → 2A1 | Idle RSS median, 2A → 2A1 |
| --- | ---: | ---: |
| empty | empty canvas → empty canvas | 333.59 MiB → 333.63 MiB |
| test_file | 593.7 ms → 612.3 ms | 358.68 MiB → 346.69 MiB |
| mixed_prod_1000 | 581.7 ms → 540.7 ms | 359.71 MiB → 339.89 MiB |
| stress_1000_paths | 622.4 ms → 572.3 ms | 380.82 MiB → 380.92 MiB |
| stress_250_unique | 622.7 ms → 556.0 ms | 379.56 MiB → 379.88 MiB |
| axis-50000-png | 809.3 ms → 829.2 ms | 336.75 MiB → 336.78 MiB |
| axis-4096-jpeg | 577.8 ms → 556.3 ms | 355.06 MiB → 355.28 MiB |
| axis-6000-jpeg | 615.3 ms → 573.1 ms | 355.80 MiB → 355.80 MiB |

These three-pair startup samples show no material unexplained regression, not a
statistically established speedup. Callback p99 for ordinary 4k JPEG is
21.96→22.86 ms and 6k is 21.84→21.99 ms, including presentation waits. GPU payload
and ordinary-JPEG RSS remain essentially unchanged. The paired owner-board and
50k PNG recognizable changes are +19/+20 ms; mixed/pressure boards improve in
this sample. Scope remains Linux/X11/Vulkan, warm or unspecified OS page cache.

The grouped/rotated eight-source native sweep converges at all sampled stages
and remains idle. The mixed pixel-art/JPEG witness adds real save/reopen under
Nearest and Smooth; runtime pressure and stale authority are separately stressed
by the Rust cache/renderer tests, rather than inferred from input counts.

`native2a` passes 32/32 assertions with three real clients: verified CAS pixels,
revision replacement, undo, deduplication, restart/rejoin and canonical convergence
remain intact. A concurrent local board remains independent and causes no disk
mutation. Server binary identity is unchanged.

[Linux/Windows Quality CI](https://github.com/gurppt/tack/actions/runs/37857273202)
passes all three jobs for the exact implementation commit, including software
Vulkan tests on Linux. Windows desktop interaction/performance is not claimed.
The owned missing-helper package probe reports an explicit refusal, two expected
failed representations, zero pending work, quiet idle and healthy shutdown.
That fixture has no stored overview and proves no visible fallback.

The measured stripped client is 19,137,672 bytes versus 19,182,856 bytes for the
Phase2A baseline (−45,184 bytes). The additional packaged decoder is 890,896 bytes;
combined client/helper growth is 845,712 bytes before unchanged artwork. The headless server SHA256 remains
`3e43ef7addba4eae197761f87e6287c2b570bb6ab2121a5950c93c8cd0417108`.
The Cargo lockfile/package graph is unchanged. Final sizes/hashes are recorded
in the measurements and `final-build/` receipts. The human build stamp names
the verified implementation commit; report-only commits do not change these
208 tracked source/configuration hashes or the measured executable.
`bin/tack`, `bin/tack-server` and `bin/tack-jpeg-decoder` are ready.

Rejected pilots and input-overlap runs (`pilot-50k`, `50k-default-01/02`, old
unsuffixed axis runs) remain diagnostic evidence and are excluded from accepted
performance aggregation. The original sweep is reused. No user corpus is copied
or removed; generated color/pixel fixtures, compact receipts and per-open caches
stay bounded. Subjective visual feel is queued separately in
`HUMAN_REVIEW_PENDING.md`; technical image-supply failures are not delegated to
owner testing.

## Human check and storage

Launch `./bin/tack` or open a board with `./bin/tack open FILE.tack`. Keep
`tack-jpeg-decoder` and `tack-about.png` beside the client. The only remaining
human checks are perceived pixel-art sharpness in a normal long session, huge
JPEG pan/zoom feel, and actual Windows desktop input/DPI behavior. A reproducible
technical regression can reopen this corrective pass.

The reused sweep and owner originals are preserved. New phase receipts and tiny
color/pixel fixtures occupy about 113 MiB, below the 512 MiB generated-data
budget; Rust uses the existing shared target. Disk reserve remains about 19 GiB,
above the required 10 GiB. No broad cache wipe or corpus regeneration was used.
