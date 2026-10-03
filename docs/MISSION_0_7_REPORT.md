# Mission 0.7 — first-open overview preparation gate

Date: 2026-10-03. Scope: benchmark-only first-open preparation of the existing
128-pixel tier. Phase 1 has not begun. The private PureRef board was not opened,
copied or imported.

## 1. Mission 0.6 baseline and provenance

Mission 0.6 selected turbojpeg 1.5.1 / libjpeg-turbo 3.2.0 DCT for 128-pixel
thumbnails, with two workers and unchanged Rust medium/detail decoding. Its
roughly 2× thumbnail production gain did not fix empty-cache board overview:
about 5.7% recognizable coverage in 12 seconds; retained SSD thumbnails gave
about 96.6%. See [Mission 0.6](MISSION_0_6_REPORT.md).

The new pre-change reference reproduces 5.53% overview, 79.80% normal pan,
77.06% fast pan, 41.48% zoom and 35.71% scan; the distant-jump diagnostic gives
0%. Source/binary snapshots are retained. The final disabled-preparation control
repeats the same six traces. [Control evidence](../benchmarks/mission0_7-controls.json)
keeps both; original reports did not drain assets after the trace, while final
reports do. Navigation coverage/CPU/GPU always describe only the timed 12 seconds.

Primary corpus: 1,000 IDs / paths over 32 hardlinked synthetic 6000×4500 JPEG
contents, seed 20261002, Pillow 10.2.0; manifest SHA256
`8bd27d9631560213365df47b842e37720421703608b13e79ee54f8524ad44af6`.
Hardware: Ryzen 7 2700X, boost disabled, 31 GiB RAM, RTX 2060 6 GB,
NVIDIA 580.173.02, native Linux/X11/Vulkan; Rust 1.95.0.
Kernel page cache is not flushed. Empty-cache means empty derived PNG cache,
not cold physical storage or 1,000 unique JPEG contents.

Final executable SHA256:
`fd92de406510b5c32a5e39b8f2ce1f70709cee0dfcd4b4d434c2da4d5f7723c8`.
The frozen crate sources match the final repository sources byte-for-byte.
Raw binaries, source ZIPs, manifests, stage/frame reports and cache inputs remain
in ignored `benchmark-results/mission0_7-*`; compact evidence lives under
`benchmarks/`. Main final sets are `final-preparation`, `final-navigation`,
`final-four`, `final-control`, and `final-recovery`.

## 2. Preparation implementation and bounds

The standalone `overview_prepare` example requests remaining 128 tiers in
stable asset ID order. `OverviewPreparation` tracks validated cache hits and
successful temporary-write/rename publication independently of RAM eviction.
It separates historical ready IDs, terminal failures and retryable cancellation;
ready/error sets remain disjoint. A stale result can still represent a valid
persisted thumbnail. An unavailable disk cache is a preparation failure but
ordinary RAM delivery can remain useful. “All settled” is distinct from
“100% prepared”; timeout is explicitly identified.

The scripted `--prepare-overview` flag continues preparation through the same
pool. Foreground tiny → medium → detail demands enter first; at most one
background thumbnail per worker counts inside the existing total of eight
pending jobs/results per worker. There is no second pool, frame-path filesystem
probe or wait. Two workers remain default. CPU/GPU payload budgets remain
64/128 MiB, split 32/32 and 64/64 between tiny/refinement; SSD remains four
128 MiB shards, total 512 MiB. Uploads remain ≤8 / 16 MiB per frame, outstanding
submissions ≤3. Profile/progress storage is bounded by existing 20,000-job
telemetry and the ≤10,000-object immutable manifest.

Standalone partial stops cancel and drain explicitly. All scripted reports
also cancel/drain assets after the timed navigation, before cache inventory or
warm copying. Codec calls cannot be preempted; the 30-second drain deadline is
outside the event loop. This proves process-interruption consistency, not
power-loss durability. See [implementation/method details](research/overview_preparation_experiment.md).

## 3. Charged preparation time and progress

The final full two-worker run reaches first thumbnail at 102.4 ms, 10% at
3.767 s, 25% at 9.100 s, 50% at 17.948 s, 75% at 26.825 s, 90% at
32.092 s, and 100% at 35.694 s. Total including drain is **35.694 s**, or
28.02 images/s. The initial healthy implementation repeat was 35.838 s; this is
repeat evidence on one controlled corpus, not a population confidence interval.

It reads 7,110,791,177 logical source bytes, commits 46,821,740 PNG bytes,
observes 1,000 misses / zero hits / zero errors, and reaches 56.0 MiB peak RSS.
Workers are active for about 99.7% of available capacity. Process CPU time and
stage distributions are in [preparation evidence](../benchmarks/mission0_7-preparation.json).
Worker occupancy includes I/O and is not CPU utilization; source byte length is
not exact Vec capacity. Derived bytes mean successful committed PNG volume;
attempted write sizes are separately recorded, not claimed as physical device I/O.

Independent partial runs create exact 250 / 500 / 750 valid PNG inputs. Their
threshold/drain costs differ slightly from the full-run curve due to separate
trials and cancellation timing. The visible-first timestamp below includes the
fresh navigation process startup; it does not add startup a second time.

| Input | Charged preparation including drain s | App startup, normal pan s | First recognizable content since preparation began s |
| --- | ---: | ---: | ---: |
| 0% | 0.011 | 0.646 | 0.835 |
| 25% | 8.865 | 0.622 | 9.587 |
| 50% | 17.769 | 0.663 | 18.535 |
| 75% | 27.194 | 0.646 | 27.956 |
| 100% | 35.694 | 0.648 | 36.453 |

The harness uses separate processes for preparation and navigation and excludes
cache cloning/inventory from measured application work. This is an experiment,
not a proposal for two-process product startup. Immediate open itself need not
launch the standalone zero-percent preparer; its 11 ms input-creation overhead
is retained here for transparent accounting.

## 4. Partial-preparation navigation

Each trace starts from its own exact copy of the named cache input. No prior
trace enriches the next input. Partial starts continue background preparation;
fully prepared and subsequent-warm traces do not. Navigation speeds are fixed:
normal/fast adjacent pan at 1280/3840 screen px/s; eight-second logarithmic zoom
cycle; scan combines motion/zoom; board-tour traverses every row in 12 seconds.
`pan` is the old 20 distant jumps/s diagnostic.

Recognizable coverage is the visible-object/frame weighted fraction with any
submitted tiny/medium/detail image, not artist-rated usefulness. Placeholder
fraction is exactly 100% minus this table. These percentages describe the
whole trace, not coverage at its first instant.

| Trace | Immediate 0% | 25% | 50% | 75% | Prepared first navigation | Subsequent warm |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| pan-normal | 91.29% | 98.21% | 97.59% | 98.21% | 95.87% | 98.06% |
| pan-fast | 89.37% | 92.88% | 93.20% | 93.02% | 86.11% | 76.57% |
| zoom-traverse | 48.49% | 87.82% | 91.93% | 91.79% | 91.70% | 91.77% |
| scan | 46.27% | 90.18% | 93.85% | 93.84% | 93.52% | 93.64% |
| board-tour | 5.45% | 33.68% | 67.41% | 90.26% | 96.65% | 96.65% |
| pan | 27.41% | 39.76% | 47.27% | 52.57% | 50.83% | 49.66% |

Requested-LOD coverage is a separate, stricter measure:

| Trace | Immediate 0% | 25% | 50% | 75% | Prepared first navigation | Subsequent warm |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| pan-normal | 21.50% | 40.19% | 38.13% | 39.48% | 39.86% | 78.45% |
| pan-fast | 0.41% | 0.42% | 0.49% | 0.47% | 0.28% | 3.32% |
| zoom-traverse | 45.00% | 84.91% | 89.00% | 88.90% | 88.94% | 90.49% |
| scan | 41.99% | 86.23% | 89.93% | 89.93% | 89.59% | 91.63% |
| board-tour | 5.45% | 33.68% | 67.41% | 90.26% | 96.65% | 96.65% |
| pan | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |

After about 8.9 seconds of preparation, normal pan is 98.2% recognizable and
zoom/scan 87.8/90.2%. This is spatially biased evidence: stable IDs correlate
with board rows, so the early prepared set covers the initial pan region.
Results are not monotonic in preparation fraction. Neither full prepared input
nor warm reuse guarantees high detail coverage in fast motion.

## 5. Background progress and fairness

Actual persisted overview file count after each 12-second trace plus separately
charged cancellation/drain:

| Start | Pan-normal: PNGs at end | Pan-fast | Zoom | Scan | Board-tour | Jump |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0% | 52 | 103 | 253 | 224 | 336 | 336 |
| 25% | 250 | 250 | 457 | 427 | 580 | 582 |
| 50% | 500 | 500 | 661 | 628 | 804 | 754 |
| 75% | 750 | 750 | 858 | 826 | 997 | 875 |

This exposes a limit rather than hiding it: at 25/50/75% the normal/fast pan
runs make **no new global preparation progress** while foreground refinement
occupies the pool. Foreground demand is protected; global completion is not
fairly guaranteed under sustained visible work. Standalone preparation can
complete the board first, but the proposed “open immediately and progressively
finish in background” UX is not yet demonstrated for every navigation pattern.
No production UX is selected here.

The app's preparation-ready counter is validated history in that process;
it can be below the initial SSD count if many old entries were not visited.
It is not a live inventory. The harness records both actual input/end cache
counts and the per-process counter. At this corpus size thumbnails fit the SSD
quota and remain retained; larger-board eviction would invalidate that inference.

## 6. Fully prepared first navigation and warm reopen

All 1,000 overview PNGs cost 35.7 seconds to create before the fresh process.
Board-tour then reaches 96.65% recognizable/requested coverage, serves all
1,000 demand episodes with none censored, first-image p50 67.53 ms / p99
872.97 ms, and reads exactly 46,821,740 cache bytes with zero source work.
App startup is still about 0.62 seconds. The subsequent warm board-tour is
96.65% again, with the same input tier and empty RAM/VRAM.

Normal first prepared navigation gives 95.87% recognizable / 39.86% requested
coverage. Its own subsequent warm reopen improves requested coverage to 78.45%
and recognizable coverage to 98.06%. Warm fast pan instead falls from 86.11%
to 76.57% recognizable while requested detail improves only 0.28% → 3.32%.
Cached higher LOD work can occupy both workers and delay newly visible cheap
reads; codec calls remain uninterruptible. That supply-policy limit is not a
renderer stall and is a remaining planning issue, not a universal warm benefit.

Detailed LOD distribution (percent visible-object/frames), resource and stale
counts for prepared first navigation and its own warm reopen:

| State / trace | Tiny / medium / detail % | CPU p99 ms | GPU p99 ms | RSS MiB | Pending peak | Stale |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| prepared-100 / pan-normal | 17.10 / 38.92 / 39.86 | 5.957 | 0.885 | 509.4 | 4 | 0 |
| prepared-100 / pan-fast | 32.40 / 53.44 / 0.28 | 2.270 | 0.189 | 523.8 | 4 | 23 |
| prepared-100 / zoom-traverse | 84.72 / 3.67 / 3.31 | 1.112 | 0.465 | 499.5 | 16 | 7 |
| prepared-100 / scan | 89.97 / 2.86 / 0.69 | 1.935 | 0.609 | 516.5 | 16 | 6 |
| prepared-100 / board-tour | 96.65 / 0.00 / 0.00 | 2.814 | 1.624 | 361.2 | 16 | 0 |
| prepared-100 / pan | 50.83 / 0.00 / 0.00 | 0.523 | 0.104 | 366.4 | 9 | 154 |
| warm / pan-normal | 5.46 / 14.15 / 78.45 | 3.045 | 1.099 | 474.9 | 4 | 1 |
| warm / pan-fast | 18.29 / 54.96 / 3.32 | 2.950 | 0.649 | 584.6 | 4 | 49 |
| warm / zoom-traverse | 83.50 / 4.68 / 3.59 | 1.831 | 1.233 | 435.7 | 16 | 9 |
| warm / scan | 88.47 / 4.18 / 0.99 | 2.317 | 0.579 | 494.2 | 16 | 9 |
| warm / board-tour | 96.65 / 0.00 / 0.00 | 2.843 | 1.708 | 361.3 | 16 | 0 |
| warm / pan | 49.66 / 0.00 / 0.00 | 0.612 | 0.104 | 367.3 | 9 | 155 |

Conditional demand latency distributions below exclude unserved episodes;
“censored” explicitly counts those, including requested LODs never reached
before leaving view or ending the trace. A low successful median with many
censored episodes is not a promise of prompt service for every image.

| State / trace | First image p50 / p99 ms; censored | Requested LOD p50 / p99 ms; censored |
| --- | --- | --- |
| prepared-100 / pan-normal | 16.93 / 297.66; 0 | 932.97 / 1236.68; 5 |
| prepared-100 / pan-fast | 17.05 / 302.65; 0 | 760.48 / 760.48; 174 |
| prepared-100 / zoom-traverse | 0.00 / 386.05; 50 | 0.00 / 502.95; 108 |
| prepared-100 / scan | 0.00 / 369.11; 41 | 17.05 / 486.74; 123 |
| prepared-100 / board-tour | 67.53 / 872.97; 0 | 67.53 / 872.97; 0 |
| prepared-100 / pan | 0.00 / 33.72; 932 | unserved / unserved; 2074 |
| warm / pan-normal | 16.90 / 251.93; 0 | 152.92 / 543.16; 3 |
| warm / pan-fast | 84.15 / 301.75; 0 | 438.05 / 791.48; 156 |
| warm / zoom-traverse | 0.00 / 368.97; 53 | 0.00 / 385.53; 98 |
| warm / scan | 0.00 / 369.38; 42 | 17.01 / 369.38; 97 |
| warm / board-tour | 67.51 / 872.99; 0 | 67.51 / 872.99; 0 |
| warm / pan | 0.00 / 33.81; 924 | unserved / unserved; 2074 |

[Navigation evidence](../benchmarks/mission0_7-navigation.json) contains all
partial LOD distributions, latency cohorts/censoring, disk/source stages, stale
and cancelled-by-stage counts, CPU/GPU/cache/queue bounds and initial/final
cache digests. Post-trace drain profiles are included in supply/I/O accounting;
coverage/frame timing remains restricted to navigation. No JPEG source decode
is needed for a valid prepared thumbnail; medium/detail can still read sources.

## 7. Controlled scheduling comparison

A: stable global background order. B: nearby assets sorted by distance inside
an expanded viewport, then stable IDs. Both keep identical foreground order,
worker count, navigation and input PNGs. The shared comparison executable and
source ZIP are frozen in `mission0_7-comparison-inputs`.

| Input / trace | A recognizable % | B recognizable % | A CPU p50 / p99 ms | B CPU p50 / p99 ms |
| --- | ---: | ---: | --- | --- |
| 0% normal pan | 89.89 | 90.92 | 0.498 / 4.374 | 0.727 / 5.289 |
| 0% board-tour | 5.27 | 5.22 | 1.097 / 2.073 | 2.473 / 3.299 |
| 25% normal pan | 97.32 | 95.60 | 0.515 / 6.594 | 0.668 / 6.086 |
| 25% board-tour | 33.09 | 32.81 | 1.182 / 2.072 | 1.995 / 3.266 |

B has no consistent advantage and adds overview sorting cost. Retain A and
foreground priority; remove B and its selector from runtime/harness. Useful
comparison evidence survives in [scheduling JSON](../benchmarks/mission0_7-scheduling.json)
and ignored snapshots. Small differences are not statistically significant
claims. Final measurements rerun the cleaned implementation.

## 8. Two versus four workers

| Preparation | Two workers | Four workers |
| --- | --- | --- |
| 1,000 baseline JPEGs | 35.694 s / 56.0 MiB RSS | 18.146 s / 74.9 MiB RSS |
| 16 progressive IDs over two synthetic contents | 3.256 s / 249.7 MiB RSS | 1.751 s / 494.2 MiB RSS |
| Concurrent codec calls observed | 2 | 4 |
| Pending bound | 16 | 32 |

Four workers improve immediate normal pan 91.29% → 98.76% and overview
5.45% → 22.18%, but normal navigation RSS increases 532.2 → 710.4 MiB.
CPU p99 stays 2.88 ms there, GPU p99 0.88 ms. Doubling progressive scratch
exposure and the interactive memory increase outweigh a default throughput
selection based solely on speed. Keep **two workers**; four remains an explicit
bounded experiment, not the default.

## 9. Storage, read volume and scale estimates

Exactly 1,000 PNG files occupy **46,821,740 bytes / 44.65 MiB**, average
46,821.74 bytes/image. Min / median / p95 / p99 / max are
46,683 / 46,829 / 46,903 / 46,932 / 46,932 bytes. The RGBA payload per image is
49,152 bytes (128×96), total 46.875 MiB before cache eviction/ownership overlap.
Full cache content digest is
`4cbfcfa8bb632d094417d6d90367ef8e8f9e466cedc39e422cef6bf568c77f0a`.

A fresh standalone all-warm verification reads all 46,821,740 bytes, has
1,000 hits / zero source attempts / zero writes and finishes in 116.99 ms
including manifest/cache startup. It fills the ordinary bounded CPU cache,
not an unbounded RAM preload. Cache open/read stage timings are in recovery
JSON; path lookup was not isolated from other warm operations, so no precise
lookup-only claim is made. GPU uploads/cadence explain why navigation availability
is slower than standalone cache validation.

Linear estimates on **this measured synthetic baseline**, not new measurements:

| Images | Estimated overview files | Estimated derived MiB | Estimated preparation s at measured two-worker rate |
| --- | ---: | ---: | ---: |
| 1,000 | 1,000 | 44.65 | 35.69 |
| 5,000 | 5,000 | 223.26 | 178.47 |
| 10,000 | 10,000 | 446.53 | 356.94 |
| 50,000 | 50,000 | 2232.63 | 1784.71 |

5k/10k PNGs fit the nominal 512 MiB quota if distributed across fixed shards;
refinement competes for remaining space. 50k needs about 2.18 GiB and exceeds
that quota by 4.36×, so the current cache cannot retain a fully prepared 50k
board. The manifest itself currently stops at 10k objects. No timing/coverage
claim is made at 5k/10k/50k. Unique image content, dimensions, progressive scans,
physical storage and CPU pressure can change these estimates substantially.
A 64 MiB GPU thumbnail partition also cannot hold 5k simultaneous 128×96
textures; this gate does not prove larger-board overview scale.

## 10. Memory

Preparation adds no full-resolution RGBA bank: two-worker baseline peaks at
56.0 MiB RSS, with 31.97 MiB observed CPU thumbnail payload. Per-job maxima are
7,123,639 encoded bytes, 1,266,750 decoded RGB bytes (750×563), and 49,152 final
RGBA bytes. Decode concurrency never exceeds selected workers. Channels,
unpublished results and worker-owned retained references remain inside the
existing pending/ownership bounds.

Native progressive coefficient/scratch allocation is not measured by those
RGB output counters; the independent progressive fixture reaches 249.7/494.2
MiB RSS at two/four workers. The codec output cap does not cap every native
allocation. Full-dimension coefficient costs from Mission 0.6 still apply.
Worker-retention peaks overlap CPU payload and are sums of historical peaks,
not simultaneous extra allocation; do not add them to RSS as disjoint buffers.
Source byte length does not equal Vec capacity, especially the oversized sentinel.

Navigation's two-worker maximum RSS is 584.6 MiB across partial/prepared/warm
traces; four-worker normal pan is 710.4 MiB. These include driver allocations,
source/refinement buffers and telemetry. CPU/GPU payload limits remain 64/128
MiB; they are not process-RSS or physical-VRAM bounds. No universal codec-leak
or arbitrary-image memory proof is claimed.

## 11. Failure and interruption

- Soft stop at 25% cancels/drains remaining work and preserves 250 entries.
  Final restart has exactly 250 cache hits / 750 source attempts, reaches
  1,000 ready in 26.589 s and reproduces the full cache digest.
- SIGKILL at 9.019 s leaves 250 valid published PNGs, each verified by Pillow.
  Restart reuses exactly 250 / decodes only 750 and completes in 26.449 s;
  no temporary files remain. Interrupted in-worker telemetry is unavailable,
  so source bytes before the kill are not invented.
- One malformed source among 999 prepared valid assets settles at
  999 ready / 1 error / 999 hits in 117.64 ms; the 100% milestone is absent.
  Additional Rust tests generate new valid thumbnails beside malformed input.
- One missing and one corrupt cache PNG in a complete corpus repair in
  270.08 ms: 998 hits / exactly 2 source attempts / 93,636 new committed bytes;
  final digest matches the original complete cache.
- Tests also cover RAM eviction, persisted stale delivery, cache unavailable
  while ordinary image delivery succeeds, historical-ready/error disjunction,
  capped oversized reads, BG/FG duplicate suppression and admission bounds.

See [recovery evidence](../benchmarks/mission0_7-recovery.json).
Rename publication is recoverable process consistency, not fsync/power-loss
persistence. Concurrent processes sharing one cache remain unsupported.
Missing source paths during initial manifest canonicalization still reject
that manifest; worker failures after a valid snapshot are recoverable. This is
not production import/error UX.

## 12. Renderer regression and quality checks

All 36 partial/prepared/warm traces plus 2 four-worker traces and 6 final
controls pass recorded bounds; hardware completion is about 59.0–59.3/s,
consistent with the 60 FPS-class scripted cadence. Among two-worker navigation
traces: CPU p99 max 6.639 ms, CPU worst 8.860 ms, GPU pass p99 max 1.708 ms,
GPU pass worst 2.026 ms. Native present worst is 34.279 ms: canvas timestamps
exclude acquisition/presentation/monitor latency, so this is not a proof that
the platform never jitters.

Disabled-control normal CPU p99 is 5.086 ms versus initial 2.995 ms;
recognizable coverage is 80.50% versus 79.80%. Single trials and completed
upload workloads do not establish the cause of that percentile difference.
It remains below the frame budget; no renderer source was changed, and final
controls are preserved rather than claiming identical timing. Preparation
adds scheduling CPU cost; it does not wait for I/O/codecs on a frame.

Passed: fmt, workspace all-target/all-feature locked check, Clippy with warnings
as errors, 24 Rust tests, docs, 6 Python harness/corpus tests, dependency advisory/
ban/license/source checks, and explicit RTX GPU readback smoke (1 test).
The recovery harness also asserts correct hit/miss/ready counts and publication.
Windows CI/native startup remain unexecuted here. No new dependency was added.

## 13. Independent verification

Independent agent `mission0_verifier` reviewed architecture and the final code.
It found and helped correct ready/error overlap after late failure; incomplete
asset drain on fully-prepared/warm reports; admission high-water undercount;
non-reentrant progressive fixture location and missing per-run provenance;
ambiguous timeout completion; oversized-read byte undercount and committed-versus-
attempted write labeling. Relevant tests and final evidence were rerun after
runtime fixes.

Final independent recomputation passes 58 raw traces / 41,161 frames (36
navigation, 2 four-worker, 6 final controls, 6 initial controls and 8 scheduler
comparisons), with source ZIP/binary/harness/manifest hashes checked. It
recomputes timing distributions, censure, byte volumes, concurrency and every
recorded frame bound; validates 10 preparation reports, 3 recovery reports,
preparation caches, navigation inputs and 38 outgoing navigation caches;
reconstructs the pre-SIGKILL published subset from restart hits with an identical
digest; and checks report tables against evidence. Independently rerun asset
tests pass (18 tests), as do 6 Python tests and rustfmt. The
[verification record](../benchmarks/mission0_7-verification.json) preserves the
audit scope and quality-gate log digest. No material defect remains. The planning-only PASS is
consistent with explicit preparation cost; background starvation and warm fast
pan are retained limitations, not an untested promise of progressive opening.

## 14. Cleanup, risks and product implications

Rejected nearby sorting and its selectors are removed, as is unused geometry
state. No predictive scheduler, import framework, runtime dependency or renderer
fork was introduced. Four workers are not selected by default. Existing native
thumbnail path and bounded ownership remain intact.

This is a single synthetic hardware target with two healthy full-preparation
trials, not representative media/artist validation. Stable row IDs bias partial
preparation; hardlinks and OS cache limit storage realism. Warm higher LOD reads
can delay cheap visible supply. Background global completion can starve during
sustained foreground refinement. The current SSD/GPU budgets and 10k manifest
cap do not prove 50k scale. No EXIF/ICC/orientation, arbitrary codec support or
production persistence is added.

The result supplies concrete planning choices: charging ~35.7 seconds before
full overview of this new 1k board; navigating after ~8.9 seconds with strong
local pan/zoom coverage; or eventually paying preparation at import and storing
cheap tiers for later first navigation. It does not select final UX between
progress indication, progressive preparation or embedded `.tack` tiers.
[Future private fixture rules](local_test_fixtures.md) remain optional and untouched.

## 15. Recommendation and stop gate

**A — PASS, for planning Phase 1 only.** The renderer remains within frame/
resource bounds, full-board cheap supply has a measured bounded cost and useful
prepared overview is demonstrated. That supports planning a local Tack product;
it does not certify immediate progressive opening, fast detail availability or
production readiness. Background fairness and warm high-LOD contention belong
in that planning and later validation.

**STOP after Mission 0.7.** No Phase 1 implementation, `.pur` importer, final
`.tack` persistence or production UI has begun. Leave these changes tested,
documented and commit-ready; wait for human review of this report.
