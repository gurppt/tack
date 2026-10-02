# Mission 0.5 — asset streaming and useful coverage

Measured 2026-10-02 on the existing native prototype. The pipeline improved
substantially, especially reuse of a board from SSD, while cold overview supply
still falls behind realistic demand. The renderer remains responsive; empty
placeholders cannot satisfy the product gate. Phase 1 has not started.

## Conditions and reproducibility

Ryzen 7 2700X (8 cores/16 threads), about 31 GiB RAM, NVIDIA RTX 2060 6 GiB,
driver 580.173.02, Vulkan, X11, Pop!_OS 24.04 / Linux 7.1.5. Rust/Cargo 1.95.0,
release + debuginfo. lscpu reported boost disabled. Two decode workers, no
prefetch, physical 1280×720, ordinary window renderer. Controlled final runs
were sequential, without simultaneous compilation/tests or another benchmark.
This is a single-machine experiment, not a statistical confidence interval.
The existing app-local libXi 1.8.3 startup correction remains in place;
see [X11 preparation and diagnosis](X11_STARTUP.md). The retained development
binary has this checkout's native-library RUNPATH; it is evidence, not a portable
application package. On affected hosts, run tools/prepare_linux.sh before testing.

Corpus: 1,000 asset IDs, 6000×4500 RGB JPEGs, seed 20261002, 32 distinct
synthetic sources reused by hardlinks. No decode deduplication by source hash
is performed: each asset ID does real work. OS page cache was not flushed;
“cold” means an empty disposable display cache, not cold physical storage.
Synthetic contents and repeated file data limit photographic/product conclusions.

```bash
cargo build --release --locked -p tack-app
python3 tools/run_streaming.py --workers 2 --output benchmark-results/new-mission0_5
python3 tools/run_benchmarks.py --seconds 8 --workers 2 \
  --scenarios pan-slow pan-normal pan-fast zoom-traverse scan
```

Principal raw evidence: benchmark-results/mission0_5-selected/{cold,prepare-0,warm-board};
matched 8-second comparison: benchmark-results/mission0_5-selected-comparable.
Each phase retains its executed binary and source-snapshot.zip. Compact checked
records are [selected](../benchmarks/mission0_5-selected.json),
[baseline/profile](../benchmarks/mission0_5-baseline.json),
[experiments](../benchmarks/mission0_5-experiments.json) and
[platform smoke](../benchmarks/mission0_5-platform.json). Raw data and executables
are deliberately ignored by Git; compact summaries retain individual raw-report
SHA256 values and the environment. Historical pre-change checkpoints captured
hashes but predate artifact snapshots; their exact executable is not retained.

Selected source SHA256: `680d751c4cd3db49b5dad1982ce0f98077a7cb67646babf346cdbd47e44ff16c`.

Executed binary SHA256: `7425bbc0c018109c163999dc594a4f9721771424c5ffe22ef5624322839c51f2`.

The source digest is SHA256 over sorted relative filenames + NUL + content +
NUL for Cargo/toolchain, crate Rust/WGSL/TOML and Python tools. Recompute from
source-snapshot.zip rather than guessing from the current checkout. All four
selected phases share the same binary/source pair; the running Python harness
also has a separate recorded digest. Report/docs edits do not alter that pair.

## Baseline reproduction and profile before optimization

The initial prototype commit 691d084 was rerun for 12 seconds per original
scenario before these changes. Original distant pan remains 20 jumps/s; it is a
cancellation torture test, not normal pan. Original zoom is distinct from the
new logarithmic overview/detail traversal.

| Scenario, 12 s | Mission 0 images | Selected images | CPU p99 before → after, ms |
| --- | --- | --- | --- |
| cold | 93.7% | 98.1% | 0.36 → 0.61 |
| warm | 99.5% | 99.9% | 0.26 → 0.44 |
| pan | 0.0% | 0.0% | 0.26 → 0.35 |
| zoom | 4.9% | 10.6% | 0.84 → 1.26 |
| pressure | 83.9% | 92.3% | 0.51 → 2.22 |


Instrument the original worker pipeline first, then change it. In the 8-second
normal adjacent trace, completed workers spent 9,414 ms decoding, 4,823 ms resizing,
464 ms PNG encoding, 73 ms source reads, 65 ms headers and 37 ms writes. JPEG
supply dominated. The old jump trace cancelled 62 jobs after decode; no resize
could finish in time to supply visible images. This established the bottleneck
before changing dependencies, cache policy or upload batching.

Stage distributions below compare the old 8-second profile with the selected
12-second normal run to show operation costs, **not equal-duration job totals**.
The warm column uses the 12-second overview tour and only PNG hits. CPU insertion
and retirement microseconds round to 0.00; exact values are in JSON. Old
source decode did not instrument output-byte counters; zero there means absent
instrumentation, not zero decoded pixels.

| Stage | Old normal p50/p99 ms | Selected normal p50/p99 ms | Warm overview p50/p99 ms |
| --- | --- | --- | --- |
| source_read | 1.22/9.32 | 1.47/9.31 | —/— |
| header | 0.89/9.80 | 0.01/0.02 | 0.00/0.01 |
| decode | 225.90/249.41 | 128.97/151.16 | 0.26/0.36 |
| resize | 65.10/223.49 | 7.54/138.70 | —/— |
| encode | 0.13/35.04 | 1.68/28.25 | —/— |
| cache_write | 0.09/3.10 | 0.27/2.38 | —/— |
| cache_read | —/— | —/— | 0.03/0.06 |
| cache_maintenance | 0.13/0.27 | 0.09/0.15 | 0.00/0.00 |
| queue_wait | 0.02/0.04 | 0.02/154.19 | 1.11/2.64 |
| cpu_insert | 0.00/6.32 | 0.00/0.00 | 0.00/0.00 |
| retire | —/— | 0.00/3.54 | 0.00/0.01 |


New normal decode p50 is ~129 ms. CPU insertion p99 is 0.00385 ms, versus
6.32 ms in the old profiled run. Retirement p99 is ~3.54 ms on workers rather
than the frame thread. Active worker time includes measured job work and
retirement; queue wait is separately recorded and is not added to utilization.

## Experiments and selected pipeline

[Asset experiments](research/asset_pipeline_experiments.md) document measured
64/96/128 overview sizes, DCT scaled/full decode, native Pillow reference,
PNG/JPEG/raw display formats, 256/512 highest-LOD tiles, three prefetch policies,
1/2/4 workers and warm handoff bottlenecks. Pillow results are not application
Rust results. No unmeasured tiling/format rewrite was introduced.

The selected pipeline preserves the four crates and native wgpu renderer:

- Metadata validation and jpeg-decoder 0.3.2 DCT scaling before resize; no nested
  Rayon and no new native/unsafe runtime boundary. The maintenance-mode dependency
  exception, license and cargo-deny review are recorded in the experiment document.
- Separate retained tiny 128 tier; regular CPU 32+32 MiB and GPU 64+64 MiB
  tiny/refinement partitions. Detail remains 2048 whole-image, medium 512.
- Visible tiny, medium, detail ahead of bounded optional offscreen tiny prefetch;
  default none. Deduplication, cancellation between stages and no stale writes
  after cancellation observed at encode completion.
- At most eight total tiny requests per worker, or one large request alone;
  nonblocking result poll. Two workers remain default. Four increase memory and
  normal CPU p99 without resolving cold-board blankness.
- At most eight uploads and 16 MiB/frame; reject oversized texture payload before
  staging; at most three outstanding submissions. Recompute lower-LOD fallback
  after eviction during uploads.
- Four stable disk shards, 128 MiB each independent of worker count. Prepare and
  trim once before the loop, avoid directory scans on warm hits, preserve tiny
  files preferentially. PNG remains disposable cache, originals unchanged.
- Worker-retained Arc ownership moves large pixel frees off the event thread.
  Bounded stage/coverage/platform telemetry exposes costs and censored demand.

The worker comparison repeats three cold 12-second traces with the **same
archived selected binary/source pair**. Two-worker values come from the principal
cold suite; one/four-worker trials were separately rerun after provenance checks.

| Workers | Normal images | Aggressive images | Scan images | Normal CPU p99 ms | Normal RSS MiB |
| --- | --- | --- | --- | --- | --- |
| 1 | 78.4% | 8.7% | 16.0% | 0.46 | 356.7 |
| 2 | 74.5% | 68.1% | 24.3% | 2.89 | 553.5 |
| 4 | 83.6% | 76.7% | 36.6% | 6.11 | 652.3 |

Rejected for this checkpoint: all-original tiling/file multiplication, changing
PNG from a foreign encoder benchmark, choosing 64-pixel tiny solely for taste,
unbounded worker growth, and declaring success from zero-cost placeholders.
Prefetch switches remain explicit experiment controls; no abandoned alternate
loader or codec dispatch framework remains in application code.

## Matched continuous navigation before/after

Pan speed is fixed at 320/1280/3840 screen px/s (0.25/1/3 viewport widths/s)
for slow/normal/aggressive traces, zoom 0.1. Motion follows adjacent rows with
continuous turns; it never waits for images. Zoom traverses 0.006→0.6→0.006
in 8 seconds; scan adds 6,400 world units/s adjacent motion. This does not slow
the camera to accommodate the loader.

This comparison reruns the selected binary for **8 seconds**, matching the
original instrumented trace duration, two workers and empty display caches.
It separates real improvements from the longer 12-second final suite.

| Adjacent trace, 8 s | Before images | After images | CPU p99 before → after, ms |
| --- | --- | --- | --- |
| pan-slow | 80.0% | 93.4% | 5.73 → 2.23 |
| pan-normal | 36.6% | 74.2% | 5.94 → 2.42 |
| pan-fast | 5.7% | 61.5% | 0.39 → 1.62 |
| zoom-traverse | 11.9% | 23.3% | 1.60 → 2.16 |
| scan | 8.8% | 20.2% | 1.72 → 1.74 |


These gains combine scaled decoding, scheduling, retained overview and handoff
changes; do not attribute all of them to the decoder microbenchmark alone.

## Cold continuous navigation and full-board warm reuse

Coverage is visible-object/frame-weighted. Tiny/medium/detail are the levels
actually selected after upload/eviction. Requested LOD is independently counted:
a tiny image avoids blankness but does not count as detail. GPU canvas timestamps
exclude uploads and native presentation; CPU frame includes upload submission
but excludes surface acquisition/present. This is not monitor photon latency
or an artist assessment of recognizable reference quality.

Cold final runs, 12 seconds each, two workers:

| Trace, 12 s | Placeholder | Tiny | Medium | Detail | Requested LOD | CPU p50/p99 ms | GPU p99 ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| cold | 1.9% | 2.5% | 4.8% | 90.8% | 90.8% | 0.11/0.61 | 1.73 |
| warm | 0.1% | 0.2% | 0.9% | 98.7% | 98.7% | 0.12/0.44 | 1.67 |
| pan | 100.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.17/0.35 | 0.07 |
| zoom | 89.4% | 9.3% | 0.6% | 0.7% | 9.9% | 0.17/1.26 | 1.20 |
| pressure | 7.7% | 7.7% | 16.2% | 68.4% | 68.4% | 0.11/2.22 | 0.20 |
| pan-slow | 5.6% | 8.8% | 13.8% | 71.8% | 71.8% | 0.15/2.17 | 1.24 |
| pan-normal | 25.5% | 21.5% | 36.3% | 16.7% | 16.7% | 0.18/2.89 | 0.59 |
| pan-fast | 31.9% | 50.9% | 17.2% | 0.0% | 0.0% | 0.19/2.09 | 0.21 |
| zoom-traverse | 70.1% | 23.6% | 3.0% | 3.3% | 27.3% | 0.19/1.04 | 1.46 |
| scan | 75.7% | 21.7% | 1.9% | 0.7% | 20.6% | 0.19/1.75 | 0.53 |
| board-tour | 97.6% | 2.4% | 0.0% | 0.0% | 2.4% | 0.65/1.22 | 0.19 |


Prepare a shared SSD cache through the ordinary overview loader, close the
process, assert every expected ID has its 128 entry, then reopen fresh CPU/GPU
caches. One 120-second preparation was sufficient: exactly 1,000 overview files,
46,821,679 B (44.65 MiB). No medium/detail entries existed at the start of the
first warm tour. Initial cold tour and first warm tour both last 12 seconds.
Preparation's 120-second average coverage is not compared to a 12-second trace.

Warm final runs:

| Trace, 12 s | Placeholder | Tiny | Medium | Detail | Requested LOD | CPU p50/p99 ms | GPU p99 ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| board-tour | 3.4% | 96.6% | 0.0% | 0.0% | 96.6% | 0.78/2.99 | 1.68 |
| pan-slow | 0.4% | 8.7% | 15.3% | 75.6% | 75.6% | 0.18/2.32 | 1.15 |
| pan-normal | 1.2% | 13.0% | 30.5% | 55.2% | 55.2% | 0.17/5.87 | 1.01 |
| pan-fast | 8.6% | 24.2% | 40.2% | 26.9% | 26.9% | 0.20/5.85 | 1.19 |
| zoom-traverse | 8.2% | 83.1% | 5.0% | 3.6% | 90.9% | 0.16/2.37 | 1.27 |
| scan | 6.4% | 87.7% | 4.7% | 1.2% | 92.5% | 0.19/2.44 | 1.06 |


The warm **board-tour is first** and is the clean overview-only comparison:
2.4%→96.6% submitted-image coverage, 1,000 SSD hits, zero source reads, all
1,000 demand episodes eventually supplied. This establishes materially cheaper
reuse of yesterday's board. Its GPU payload is 46.88 MiB for all tiny images.

Later warm scenarios reopen separate processes but share an evolving SSD cache,
in order slow → normal → fast → zoom → scan. Slow adds medium/detail entries,
so later results describe sequential board use, not independent identical
“overview-only” starting states. They still use the same camera trace as their
cold counterparts. Normal image coverage rises 74.5%→98.8%, while requested
detail remains only 55.2%; aggressive warm images are 91.4% with 26.9% detail.
Cold overview/zoom/scan remain far below credible reference-board supply.

## Demand latency and censoring

Episodes begin when an object appears or its requested level changes. First
image records any submitted lower level; requested latency requires the demanded
level. Existing tiny images can yield a zero first-image delay at a LOD change.
Unresolved episodes are **censored**, never assigned zero or dropped from counts.
Percentiles include resolved episodes only; they must be read with the censored
column. These are demand episodes, not independent user actions.

| State / trace | First image p50/p99 ms | Resolved / censored | Requested LOD p50/p99 ms | Resolved / censored |
| --- | --- | --- | --- | --- |
| cold / pan-normal | 335.87/570.03 | 49 / 1 | 1109.29/1347.01 | 32 / 18 |
| cold / pan-fast | 168.40/418.53 | 172 / 4 | —/— | 0 / 176 |
| cold / scan | 0.00/2667.40 | 181 / 493 | 436.41/2667.40 | 116 / 558 |
| cold / board-tour | 4661.61/9704.16 | 137 / 863 | 4661.61/9704.16 | 137 / 863 |
| warm / pan-normal | 16.86/17.56 | 50 / 0 | 626.81/990.97 | 47 / 3 |
| warm / pan-fast | 13.12/301.48 | 175 / 1 | 170.58/825.73 | 80 / 96 |
| warm / scan | 0.00/369.15 | 632 / 42 | 17.16/369.15 | 601 / 73 |
| warm / board-tour | 67.50/872.96 | 1000 / 0 | 67.50/872.96 | 1000 / 0 |


Normal cold first image is ~336 ms median; requested detail ~1.11 s among
resolved episodes. Cold overview median ~4.66 s is already poor and hides 863
of 1,000 censored episodes unless their count is shown. Warm overview is ~67 ms
median / 873 ms p99 with no censored episodes. The first hundreds of images
arrive progressively under the bounded eight-upload cadence, not instantaneously.

## Torture, resources and memory

The retained 20 distant jumps/s test supplies 0% images, CPU p99 0.35 ms,
GPU canvas p99 0.068 ms, and 1,372 stale observed outcomes. Of these, 175
completed source decodes consume ~22.1 worker-seconds; others cancel earlier.
Pending never exceeds 16, uploads/disk writes are zero, source reads are bounded,
and the camera does not wait. This is bounded failure under adversarial demand,
not a useful-coverage success. Its 2,074 first-image episodes are censored.

| State / trace | CPU/GPU payload peak MiB | RSS peak MiB | Pending / stale / hits | SSD MiB / files | Observed worker utilization |
| --- | --- | --- | --- | --- | --- |
| cold / pan-normal | 31.3/66.0 | 553.5 | 4 / 1 / 0 | 244.4 / 127 | 94.1% |
| cold / pan-fast | 36.3/49.8 | 412.8 | 6 / 2 / 0 | 47.1 / 166 | 94.0% |
| cold / scan | 34.7/64.8 | 495.0 | 16 / 52 / 0 | 65.5 / 100 | 67.6% |
| cold / board-tour | 6.7/6.4 | 348.4 | 16 / 265 / 0 | 6.4 / 143 | 98.0% |
| warm / pan-normal | 29.3/66.1 | 539.2 | 4 / 0 / 100 | 388.6 / 1097 | 66.7% |
| warm / pan-fast | 36.3/68.2 | 592.6 | 4 / 15 / 280 | 456.6 / 1154 | 66.7% |
| warm / scan | 46.7/79.0 | 435.8 | 16 / 5 / 407 | 457.3 / 1155 | 7.6% |
| warm / board-tour | 32.0/46.9 | 359.4 | 16 / 0 / 1000 | 44.7 / 1000 | 1.3% |


Worker utilization is completed observed active job time divided by duration ×
workers: it excludes jobs still active at exit and is a lower bound. Source
read/write/cache-read byte totals and every stage's p50/p95/p99 are retained
in JSON. Normal cold reads ~910 MB across repeated LOD requests and writes
256 MB; overview warm reads 46.8 MB PNG, writes zero and reads zero originals.
No upload/backpressure/cache/queue bound was violated in any selected raw frame;
no decode errors or dropped worker/coverage telemetry occurred.

The payload budgets are not a RAM/VRAM promise. Largest per-job RGB output is
20,250,000 B instead of full 81,000,000 B; a tiny output is 1,266,750 B. Encoded
corpus buffers are about 7 MB; RGBA detail is 12 MiB. Codec scratch is not directly
sampled: original-dimension coefficient storage can remain large (roughly 162 MB
for a worst-case RGB progressive representation at these dimensions), despite
scaled output. Vec capacities, PNG/resize scratch, worker results, shared Arc
retention, telemetry and driver contribute additional transient memory.
The JSON buffer counters log returned pixel/resize lengths and source byte length;
cached encoded input uses a conservative two-times-length estimate. They are not
allocator-sampled allocation peaks, despite the historical `peak_bytes` field names.

The worker-retention metric sums independent historical peaks and shares buffers
with the CPU cache; do **not** add it as an independent allocation to that cache.
Idle workers may retain retired buffers until their next job. Whole-process peak
RSS in principal 12-second runs reached ~593 MiB; the final four-worker comparison
reached ~652 MiB in normal pan. RSS cannot be exactly decomposed into these counters.
GPU payload excludes swapchain, staging, allocation alignment and driver overhead;
bounded uploads/submissions control backlog but are not actual VRAM telemetry.

## Presentation and platform smoke

| State / trace | Callback p99/max ms | Acquire p99 ms | Present p99/max ms | Callbacks >16.67 ms |
| --- | --- | --- | --- | --- |
| cold / pan-normal | 14.41/16.00 | 0.04 | 14.11/15.86 | 0 |
| cold / scan | 13.75/23.20 | 0.03 | 13.52/23.01 | 2 |
| cold / board-tour | 15.14/19.76 | 0.04 | 14.25/18.78 | 3 |
| warm / pan-normal | 15.51/18.68 | 0.05 | 14.82/16.81 | 3 |
| warm / scan | 11.62/20.70 | 0.03 | 11.40/13.34 | 1 |
| warm / board-tour | 12.77/17.55 | 0.03 | 12.01/17.02 | 1 |


Across the selected 17 native 12-second cold/warm traces, 18 redraw callbacks
exceeded 16.67 ms; none exceeded 25 ms. Acquire p99 stays below 0.06 ms in those
traces; maxima reach 2.46 ms. Present is the main residual callback cost, with
23.0 ms maximum in cold scan. The earlier ~26 ms present outlier was not repeated
in this suite, but occasional 60 Hz misses recur. Session CPU has no >16.67 ms
frame in these runs. Warm normal CPU p99 (~5.87 ms) includes ~5.63 ms upload
submission p99. This is upload/driver work, not synchronous asset I/O. GPU canvas
pass remains small; do not equate these metrics with monitor presentation timing.
No window-system rewrite is justified by this evidence.

Actual X11 smoke used the selected binary in interactive mode, own windows,
and graceful WM_DELETE_WINDOW close. Scale factors 1.0, 1.5 and 2.0 were set
through WINIT_X11_SCALE_FACTOR and confirmed by application initialization events.
Each window resized 1280×720→640×480→1280×720, was iconified (WM_STATE Iconic
and _NET_WM_STATE_HIDDEN), then restored to Normal, with exit code zero and
no surface timeouts/errors. These are forced scale-factor checks, not physical
multi-monitor DPI migration or an artist UI layout assessment. Winit reported
no Occluded(true) event for this WM; do not claim backend occlusion pause was
verified. Idle directional-prefetch runs consumed 4/5/6 process clock ticks in
a 2-second interval (100 Hz, at most 3% of one CPU core), with only 16–17
submitted attempts over the complete smoke sequence: no continuous redraw storm.
Wayland was not tested: no Wayland session/display or weston/cage/labwc available.

## Quality, cleanup and independent verification

The full tools/check.sh gate passed: rustfmt, workspace check all targets/features,
Clippy -D warnings, 12 Rust unit/integration tests, docs and two Python corpus tests.
The explicit Vulkan GPU smoke also passed, rejecting an over-quota texture before
staging then reading back a red rendered pixel. cargo-deny 0.20.2 passed advisories,
bans, licenses and sources; 14 upstream duplicate-version warnings remain and
are not suppressed. The normal application dependency tree contains jpeg-decoder,
no zune JPEG or Rayon. JPEG image support is test-only. No Tack-owned unsafe added.

Independent verifier /root/mission0_verifier reviewed ownership, cancellation,
cache accounting, demand fallbacks, idle prefetch and evidence provenance. Its
initial findings were fixed: cached offscreen prefetch triggered endless redraw;
evicted detail failed to use retained tiny fallback; changed worker counts broke
SSD quota accounting; over-budget representations could retry/stage before
rejection; old warm comparisons had unequal duration; large buffer retirement
hurt event CPU. Regression tests cover cache identity across worker counts,
oversized suppression, stale batches, censored episodes and pre-staging GPU rejection.
A subsequent review found no new material defect, reran seven focused Rust
tests and the native GPU smoke, and checked 29 raw reports / 25,848 frames.
Source ZIP/binary pairs, bounds, coverage/LOD equality and report tables agreed.
Final evidence audit is recorded in
[verification](../benchmarks/mission0_5-verification.json).

Application code has one loader/cache implementation. Isolated experiment
harvesters and their records remain for reproducibility; no alternate production
loader, dead format/tiling implementation or unused runtime dependency remains.
Changes are confined to asset/render/app telemetry and scheduling, benchmark
tools, focused tests and documentation. User assets are untouched. See
[architecture](architecture.md) for invariant-level ownership and limits.

## Remaining experiment and stop gate

High normal warm image coverage and bounded responsive rendering support keeping
the architecture. However, ~25% normal cold blankness, ~70–76% cold zoom/scan
blankness, almost-empty cold overview and high requested-detail censoring cannot
be called a credible product supply gate. No flattering aggregate FPS/coverage
threshold was invented. Real photographic diversity, color correctness and
artist-rated tiny usefulness also remain unvalidated.

The **one specifically required next experiment** is an isolated safe-wrapper
libjpeg-turbo scaled JPEG decoder for the retained 128 tier, comparing identical
cold normal/fast/zoom/scan/overview traces against this source/binary pair. The
~70 ms native reference versus ~130 ms Rust tiny decode gives a measured reason
to try it; it does not establish a pass. Keep two workers, existing queue/cache/
upload bounds and camera speeds. Review native lifetime/error/allocation bounds,
malformed JPEG behavior, licensing/maintenance, and cancellation before measuring
end-to-end coverage, censoring, stage costs, RSS and warm reuse. If that bounded
experiment cannot materially improve cold overview supply, re-evaluate supply
strategy before product work. Tiling and production UI are not this next step.

STOP after Mission 0.5; human review of this report is required before Phase 1.

**Recommendation: B — REPEAT.** One bounded native scaled-thumbnail decoder
experiment is still required; the promising streaming architecture has not yet
passed the cold useful-image gate.
