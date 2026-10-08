# Phase 2A1 independent core image review

2026-10-09. Reviewer is separate from the JPEG, LOD and native integration authors.
Scope: resident quality selection, prolonged cache churn, native JPEG working
sets and subprocess lifecycle, disposable source/revision keyed supply, packaging,
and preservation of Phase 2A local-first/headless boundaries.

Status: **PASS — independent technical review**. The worker-output and
source-shared resident admission counterexamples below have been corrected.
Final source/gate/GPU, native LOD/JPEG/CAS, ordinary comparisons and exact-source
Linux/Windows CI evidence are green in the reviewed scope. No concrete code
blocker remains; root owns final report/measurement publication. Subjective
artist feel and Windows desktop performance remain separate human evidence.

## Adversarial findings

1. The previous smallest-adequate resident rule can replace sharp resident pixels
   with a filtered lower representation at a zoom threshold. It is a concrete
   quality-valley mechanism; Nearest sampling cannot reconstruct pixels discarded
   by thumbnail filtering. The new finest-valid-resident rule removes that
   historical selection dependence. Current source/revision/edge GPU keys remain
   authoritative; obsolete pixels cannot qualify through an old key.
2. A small Nearest reference must request its native pixels even below the preview
   boundary. The implementation adds a bounded Medium floor for sources at most
   512 pixels on their longest axis and accounts for actual bounded source axes.
   PNG resizing must not upscale smaller sources or that admission estimate would
   understate actual allocated pixels. Native floor remains budget admitted, with
   a preview fallback when the visible set exceeds available detail residency.
3. Progressive rejection alone does not prove JPEG row-bounded decoding.
   Upstream `jdmaster.c` requests whole-image coefficients for any multiple-scan
   input, including sequential per-component scans. The new first-SOS parser
   requires all baseline frame components in that scan, pins/replays the inspected
   header, and the child additionally limits scan count to one.
4. The initial JPEG mip implementation proportionally resized a cropped 511-row
   or column region to 256 samples when the exact mip stride was two. This shifted
   samples and disagreed with adjacent tile origins. The implementation now carries
   exact integer mip stride; nonconstant neighbor and partial-edge tests prove it rather than relying on uniform fixture colors; real native mip-4
   neighbor comparisons are also green.
5. Huge-tile eligibility originally compared displayed edge with the overview tier
   before adjusting demand for crop. A 50k source cropped to 0.5%, displayed at
   800 pixels, needs detail despite its displayed edge being below 2048. Crop-aware
   eligibility now uses the same adjusted density in both its candidate count and
   planning loop. The focused cropped-source test asserts that 800-pixel display
   demand selects tiles at the correct source region.
6. `djpeg -maxmemory` is a virtual-coefficient backing-store budget, not a process
   RSS ceiling or hard bound on every ordinary native allocation. Baseline row
   scratch, parent buffers, encoded input, helper RSS and cache ownership must be
   reported separately. New regions still sequentially consume compressed input;
   successful derived cache hits are the regional reuse mechanism, not JPEG random
   access or a one-time whole-source tiled preparation guarantee.

7. Native JPEG crop alignment must account for legal sampling factor three.
   A fixed 32-pixel alignment fails for 24/12/6/3-pixel MCU columns, expanding
   output unexpectedly. The implementation now aligns to 96, the least common
   multiple covering admitted factors 1..4 at DCT sizes 1/2/4/8. Progressive
   coefficient estimates likewise round each source axis to 96 before applying
   the conservative three-component coefficient cost. A real factor-three
   codec fixture remains desirable beyond arithmetic proof; accepted sampling
   factor-three alignment is now structurally correct.
8. Old normal-PNG repair products could contain upscaled 512/2048 pixels for a
   192-pixel original, contradicting new source-size admission. Normal PNG and
   ordinary JPEG products now use generators 6 and 7; old 4/1/2 are not eligible
   repair cache candidates. A poisoned old-generator regression proves migration
   behavior. New JPEG tiles include a one-pixel neighbor gutter and adjusted UVs;
   slot budgets conservatively account for 258×258 pixels.

9. Initial native `50k-default-01` receipt mislabeled deep detail as its final
   overview: the last frame remained at zoom 64 with six requested/ready tiles.
   Its zoom range went from 0.4 to 64 and did not return. Input after pan was sent
   with the pointer outside the application, so a quiet detector could not prove
   zoom-out or return behavior. The harness must recenter input before each phase
   and assert actual stage camera/zoom/tile states. That run is diagnostic evidence,
   not acceptance of sharp zoom-out or exact regional reuse.

10. Asset pixel dimensions can be malformed yet structurally valid in a saved
    board or shared canonical document. Claiming 1×1 for a real 4k source reduces
    detail reservation to four bytes while its actual Medium decode costs 1 MiB.
    Sufficient distinct sources then force GPU eviction/refinement churn because
    the admitted set cannot fit. Existing cache hard ceilings prevent unbounded
    allocations, but do not prevent this busy loop. The corrected worker validates
    new-result and repair-cache dimensions against declared axes clipped to the
    requested edge, and requires the expected longest edge. Oversized or undersized
    cached products are removed and regenerated; mismatched source output fails
    once before storage/publication rather than repeatedly retrying. Thumbnails use
    conservative square reservation and fixed-size tiles have a separate budget;
    they retain their distinct historical-fallback semantics. Tests cover new
    forged metadata, oversized/undersized valid PNG caches and legacy upscaled
    persisted previews. This closes the worker admission gap without extra warm
    header reads or any server JPEG dependency.

11. The first worker-output guard alone does not cover source-shared cache history.
    Detail CPU/GPU keys use source/revision/tier, not asset identity. Asset A can
    legitimately admit a 512² product, then a malformed same-source Asset B can
    claim 96² and reuse that existing product without another worker job. This
    bypasses fresh/cache worker validation and under-reserves current residency.
    Admission now takes the maximum of declared axes and actual non-thumbnail
    CPU/GPU resident axes for the current source/revision, then aggregates maxima
    across visible aliases. Both caches are queried separately without touching
    eviction order, covering CPU-only and GPU-only residency. The source-alias
    worker test proves A→B reuses the larger actual pixels without decoding; the
    planner test proves their reservation and alias-order independence. These
    lookups require no filesystem work.

12. Classifying only `HugeTiled` jobs as cancellable misses a 50k×128 JPEG:
    its decoded area is in the `Large` class, yet native streaming/tile work is
    required. Cancellation ownership now follows any encoded tile request or
    `requires_streaming` dimensions, including wide sources outside monolithic
    decoder limits. Demand removal and suspend signal the same job token used
    by the feeder/watchdog and stale publication gate. This changes active work
    policy after the measured matrix and needs final native coverage.

## Inspected mechanisms

The huge parser bounds retained header bytes to 1 MiB, validates one or three
components, 8-bit interleaved baseline scan layout, codec axis limits, sampling
factors and total MCU blocks before launching native work. Its conservative
native-row estimate is width × 192 + 1 MiB, checked against 16 MiB; it does not
multiply full width by height. The pinned upstream main controller keeps one
component iMCU row plus bounded context groups; upsampling and PNM output use
rows. The parent retains one RGB row plus at most a 16 MiB representation output,
not a giant source raster. Ordinary progressive and split-scan JPEG has a separate
192 MiB coefficient-plus-row admission estimate before native parsing/decode,
with full iMCU-axis padding. This is not a claim that total process memory or
all in-flight outputs fit the CPU resident-cache budget.

The helper is resolved beside the running executable, including the Windows
`.exe` suffix. No compiled-in repository path is required at runtime. Cargo's
assets build script copies the prepared pinned tool beside profile binaries and
integration tests; the human build script also packages it and records its hash.
There is no new package dependency or helper in the server dependency manifest.

Active decoding owns one child, feeder and watchdog per admitted worker. The
watchdog interrupts blocked pipe reads by killing the child on cancellation or
a 30-second active-work deadline; ordinary settled documents create no helper,
watchdog or timer. The watcher is created before the feeder, and feeder creation
failure kills the child. Result publication remains current-demand/revision
checked on the existing supply path. Final cancellation/error tests and actual
linked/embedded integration cases pass in the reviewed gate receipts; final
native source/packaging validation remains separate.

The GPU harness exercises real LRU pressure and readback with filtered/native
patterns, Nearest and Smooth, rotation/crop/flip and old/current source revision
keys. The worker harness separately exercises 1,200 supply/cache transitions,
derived reuse, native image dimensions, bounded CPU/request peaks and exact
save/reopen metadata. Explicit GPU receipts are recorded below. Group flattening
shares the same render-data authority, with its separate native interaction
receipt in the final closeout section.

## Independent arithmetic crosscheck

A deterministic standalone Python audit (seed `0x2a1`) evaluated 10,000 random
codec-axis layouts, all possible mip bands, random valid tiles and partial edges.
With 96-pixel crop alignment every gutter coordinate was monotone and in bounds,
first/last logical samples matched the exact mip lattice, and origins aligned for
all admitted horizontal factors and power-of-two DCT sizes. This is an arithmetic
crosscheck, not real codec/pixel evidence or replacement for native tests.

## Accepted serial native matrix

Reviewed all 13 `*-v2/summary.json` receipts under
`benchmark-results/phase2a1/`, bound to measured binary SHA-256
`c9f7e4eae976ab6f49b8cc012cf1d7273848385187b61b62d2553aeca30a43f5`.
All have zero image-supply errors and finish with no requested detail tiles after
zoom-out. First recognizable CPU-submission time is approximately 0.51–0.64 s
in these runs; this is neither physical present timing nor a cold-disk guarantee.
Raw callback/GPU/cache/child/RSS data remains available with explicit limits.

For `axis-50000-v2`, independent stage/frame inspection shows:

| Settled stage | Camera x / zoom | Ready / requested tiles | Completed codec jobs | Source bytes (MiB) |
| --- | --- | --- | ---: | ---: |
| Cold open | 750 / 0.4 | 0 / 0 | 3 | 6.04 |
| Deep detail | 750 / 64 | 6 / 6 | 13 | 22.59 |
| Distant pan | 764.0625 / 64 | 6 / 6 | 17 | 30.14 |
| Exact return | 750 / 64 | 6 / 6 | 17 | 30.14 |
| Zoom-out | 750 / 0.001 | 0 / 0 | 21 | 35.73 |

The exact return requests no additional decode or source bytes. Derived products
remain at 421,343 bytes across far-pan and return. CPU/GPU resident peaks are
about 2.55 MiB. Potato and cold-reopen 50k runs also converge; the potato run
uses about 2.83 MiB resident CPU/GPU payload. Sampled child RSS is around 2.4 MiB
where observed, but 50 ms sampling misses short-lived helpers and cannot prove a
scratch bound. Matrix idle samples show 0 frames, 0 I/O and 0–2 CPU ticks over
approximately two seconds. The rejected pilots must not be combined with these
accepted serial timings because their inputs/clock boundaries were contaminated.

Multiple-JPEG and JPEG-plus-PNG cases likewise return to the exact previous
camera with unchanged completed decode/source-byte counters. Both the JPEG and
PNG visible detail sets converge (10/10 tiles in the mixed PNG case); ordinary
references remain useful. The reviewed cold/deep screenshots contain source
patterns instead of a flat placeholder. Nonconstant RGB neighbor and authority
tests provide stronger pixel evidence than screenshot appearance alone.

The mission's ordinary regression suite, long native LOD harness and final CAS
check remain separate acceptance evidence; this matrix is not a claim that a
50k×50k source is efficient, every detail mip is filtered, or a permanent tile
pyramid has been built.

## Adversarial test receipts reviewed

`gate-02.log` records green format/check/Clippy, all workspace tests/documentation,
Python tests and cargo-deny checks. Its JPEG cases include the actual-helper
blocked-raster cancellation, malformed/truncated inputs, progressive refusal,
nonconstant mip/gutter samples and three JPEG authority tests. Existing server
and verified/deferred shared supply tests remain green. Explicit ignored GPU
execution is separate from the normal gate.

`gate-03.log` additionally records the final worker-size and resident-alias
corrections: 28 asset-library and 28 Product tests pass, including forged source
dimensions, legacy/cache repair and source-shared actual-pixel reuse. The planner
alias-order/actual-resident reservation test passes. Format/check/Clippy,
workspace tests/documentation, Python and all four cargo-deny checks finish green.

`subsampled-tests.log` additionally passes actual 4:2:0 and 4:2:2 neighboring
color-gutter equality on both horizontal and vertical boundaries. The suspected
fancy-upsampling crop-edge discrepancy does not require a production fix for
this lattice: even boundary gutter coordinates retain the appropriate neighbor
context; odd interior samples use the supplied right gutter. This test settles
that hypothesis with real native codec pixels, rather than documentation alone.

The measured candidate precedes the final policy-call refactor, preview metadata
mapping, worker dimension validation, resident-alias cost correction and expanded
streaming cancellation ownership. Its measurements must remain bound to the
candidate SHA rather than be described as final-executable measurements. Exact
source manifests and representative final native reruns determine whether those
later changes preserve measured behavior.

`gpu.log` independently confirms all explicit renderer/readback tests, both short
LOD convergence tests and the new long churn GPU test pass. The long test runs
1,200 real eviction/reentry cycles and 12,000 tier-band revisits, with 200 native
pixel readbacks under Nearest/Smooth and transformed crop/flip data. This proves
current resident selection and pixel identity; it does not substitute for the
separate malformed-admission tests or real native input-churn receipt.

`gpu-final.log` repeats the final-source adapter checks successfully: smoke,
selection, nine product tests, two convergence tests and long churn (2.48 s).
Implementation commit `5c2c0bb90ca5533c13b75634696dd6f7b96f3c7b` has final raw
client SHA-256 `df026f025fe7c35e44d3994b0f0229e1f04e051ab15346ad8e9d1caa67060ab5`.
Its stripped client is 19,137,672 bytes; the packaged helper is 890,896 bytes with
the unchanged pinned helper hash. A fresh SHA audit finds no mismatch among the
208 files in `final-build/source.json`; the server binary SHA remains unchanged.

The four final-native receipts (`final-50k`, `final-potato`, `final-color`,
`final-mixed`) use that final client SHA. Independently matched raw frames show
all requested settled tiles ready, exact camera returns with no additional
codec/source reads, and actual zoom-out with no tiles. Each has zero supply
errors and zero idle frames/I/O. Final 50k recognizable/tiled submissions are
496/2822 ms, CPU/GPU payload peaks 2,673,312 bytes. The nonconstant color fixture
converges to five detail tiles and returns without rescanning; the mixed fixture
converges to ten. These support the final cancellation/admission changes without
relabeling the earlier full 13-run candidate matrix.

`native-lod/summary.json` records 2,496 sent wheel actions, 32 pans and five
filter changes, then save/reopen with Nearest retained. Its 258 churn trace
frames and 20 reopen traces contain zero resident-choice/history valleys;
settled quality equals visible count and both sessions idle without frames or
disk I/O. This run has no CPU evictions and its boundary-offset stages leave the
camera unchanged: it proves real native interaction/coalescing/save/reopen,
while actual pressure and threshold-band completeness come from the worker/GPU
tests above. Sent input events are not completed codec transitions. Nine active
cancellation warnings in the churn log are not idle work or supply errors.

A second run, `native-lod-threshold`, explicitly sends the six-wheel offset:
600 → 99.1793 projected pixels, then repeated 99/134-pixel reversals and return.
Its 2,520 wheel actions, 263 churn traces and 17 reopen traces have zero valleys.
At the settled small band Nearest retains desired/displayed Medium with actual
256×128 pixels; Smooth requests Thumbnail but correctly displays the better
resident Medium. The exact baseline binary run (`native-lod-baseline`) yields
79 counterexamples to the new finest-resident invariant: Thumbnail is displayed
while Medium is still resident, including settled stages at that same band.
This deterministic control proves the concrete selection mechanism; it does
not claim to reconstruct the owner's exact eight-hour cache history. Neither
native run forces CPU eviction; real eviction/reentry evidence remains the
separate worker and GPU stress tests.

`grouped/summary.json` and raw `zoom.json` show one actual group, eight mixed
transformed references visible after dezoom, and all settled references resolved
across strong zoom, reversal, rapid alternation and pan. No supply errors or
idle frames/I/O occur. This closes the separate grouped native interaction gap.
The final ordinary comparison receipt contains 36 serial runs: six boards,
baseline/current, three repeats each. All have zero IP sockets, idle redraws and
idle I/O. Median recognizable submission changes are +19 ms for `test_file`,
−41 ms for mixed1000, −50 ms for paths1000, −67 ms for unique250 and +20 ms for
50k PNG; warm/unspecified OS cache and only three samples prohibit broad
performance claims. Empty-board median RSS is essentially unchanged (333.6 MiB),
while mixed1000 drops about 20 MiB. Observed local IP/socket/thread invariants
are preserved; OS/driver background ticks remain reported separately.

The final `native2a/checks.json` has 32 observed assertions, backed by the final
client/server SHA provenance. Three simultaneous native clients converge on the
same canonical revision-15 hash, display actual verified originals, retain no
unavailable sources, and preserve durable undo/conflict/rejoin/source revision
behavior. The independent local board remains responsive with no shared backend
or disk mutation. CAS original deduplication and explicit source replacement
remain correct. This closes Phase2A preservation in the reviewed scope.

Twelve additional ordinary-JPEG comparisons (4k/6k, three repeats,
baseline/current) retain identical GPU payload peaks and essentially unchanged
RSS, zero IP sockets/idle redraws/I/O. Median recognizable submission improves
577.8 → 556.3 ms and 615.3 → 573.1 ms respectively. These limited observations
support preservation of the ordinary native codec path.

Four standalone helper `/usr/bin/time -v` receipts measure actual wait4 process
high-water RSS, avoiding the native matrix's 50 ms sampling gaps: grayscale/color
50k overview and region cases peak at 2,359,296–2,617,344 bytes. Receipt hashes
match their manifest. They corroborate the narrow-fixture behavior, not a
universal allocation proof or parent/GPU RSS ceiling. The first missing-package
probe failed because the harness window search used a different display from
its child. Corrected `missing-run-v2` and `helper-missing.json` prove an explicit
restore-helper refusal, bounded terminal failure, no pending work, zero idle
frames/disk I/O and healthy shutdown. The fixture has no recognizable fallback
pixels (`first_recognizable_ms = null`, final recognizable count zero), so this
probe does not prove a stored-preview fallback; source authority/fallback tests
are distinct evidence.

GitHub Quality run
[37857273202](https://github.com/gurppt/tack/actions/runs/37857273202) on exact
implementation commit `5c2c0bb90ca5533c13b75634696dd6f7b96f3c7b` completes
successfully for dependencies, Ubuntu and Windows. This validates packaged
decoder/test portability; it is not a Windows desktop performance measurement.
