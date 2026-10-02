# Mission 0.6 — Native thumbnail decode gate

2026-10-02. **B — REPEAT.** The selected 128-pixel native path approximately
halves thumbnail production time and improves cold recognizable coverage.
It does not solve first-navigation supply: the cold overview remains mostly
empty, with seconds of first-image latency. Phase 1 has not begun.

## Exact baseline and controlled change

Baseline is local Mission 0.5 commit `a4d8f6d` plus decoder-identity telemetry
and common harness instrumentation/tests. Runtime scheduling, decode behavior
and renderer are unchanged in the reference. It was rebuilt and frozen before
native integration. No historical binary-less comparison is used as the A/B
reference. A's available-native-build metadata describes a local library which
A **does not link or use**; its runtime decoder identity is jpeg-decoder 0.3.2.

Candidate changes only source JPEG decoding for the retained **128** tier to
turbojpeg 1.5.1 / libjpeg-turbo 3.2.0. Medium/detail still use jpeg-decoder 0.3.2.
Both apply power-of-two DCT scaling and the identical final thumbnail/RGBA resize.
Two workers, no prefetch, fixed traces/speeds, queue/cache/upload/submission bounds,
PNG cache keys and renderer implementation are unchanged. No decoder dispatch
flag or implicit fallback remains. Native errors are ordinary failed assets.

Machine: Ryzen 7 2700X (8C/16T, boost disabled), 31 GiB RAM, RTX 2060 6 GiB,
NVIDIA 580.173.02, native X11/Vulkan, Pop!_OS 24.04 / Linux 7.1.5; Rust/Cargo
1.95.0. All six required traces are twelve seconds, fresh processes, initially
empty derived SSD cache. “Cold” does not mean OS page-cache flush; corpus files
were already used in earlier experiments. No compilation/microbenchmark ran
concurrently with controlled end-to-end traces. A ran before B; normal/jump
and pressure were also repeated in reverse variant order to check stability.

Corpus: 1,000 distinct asset IDs/paths referencing 32 hardlinked synthetic
6000×4500 JPEG contents, seed 20261002, Pillow 10.2.0. This is not 1,000 unique
artist photographs. Manifest SHA256:
`8bd27d9631560213365df47b842e37720421703608b13e79ee54f8524ad44af6`.

| Variant | Frozen source digest | Executed binary digest |
| --- | --- | --- |
| A reference | `bef6584c18f8d087fb4dab8f21e03f990bb4ebcc2298670a25a6f3f51b472b91` | `275a1e7902397c89ce0e519423bf6d3042bb2a57f3ea2191be8252954d3fdd4c` |
| B native | `e50452b6761a30a824400c0739d0539dfa772fc13a7aff249ca5a643253bc4b2` | `66042cb6ab783a49319ef150e0557ec353e7af44a2d6cdb03427417d80a73524` |

Common executed harness SHA256:
`f2fcec04401de39b50c3188c272f31bbe1c6851827a2b719dabee201076491aa`.
Each run retains its actual executable and source ZIP, full environment,
raw frames/jobs/demand episodes and hashes in ignored `benchmark-results/mission0_6-*`.
Compact committed evidence is in [baseline](../benchmarks/mission0_6-baseline.json),
[candidate](../benchmarks/mission0_6-candidate.json),
[microbenchmarks](../benchmarks/mission0_6-micro.json),
[warm inputs](../benchmarks/mission0_6-warm-input.json),
[idle](../benchmarks/mission0_6-idle.json) and
[verification](../benchmarks/mission0_6-verification.json).
The collector also verifies that final runtime sources match the measured B ZIP;
subsequent changes are tests/documentation/evidence tools, not runtime behavior.

## Decoder boundary and dependency review

[The decoder review](research/native_thumbnail_decoder.md) records the full
safety invariants, platform/build choices and restrictions. Tack adds no unsafe
code. An established safe wrapper owns a fresh native handle for each job and
borrows one immutable encoded slice across header/decode. Every error destroys
the handle. Width/height, supported colorspace, arithmetic/lossless status and
checked pitch/total are validated before allocating initialized RGB output.
Input is capped at 64 MiB, dimensions at 6000×4500, RGB output at 2 MiB;
progressive scans are limited to 100. Native output is written directly into a
Rust Vec and moved into RgbImage without a duplicate RGB copy. Existing resize,
cache and worker ownership remain. Cancellation works at stage boundaries;
codec calls remain nonpreemptible.

Native source is the official 3.2.0 archive, hash-verified and compiled locally
static/Release/SIMD, with no system modification or runtime download. The wrapper's
older bundled 3.1.0 source is not used. Wrapper/binding MIT licenses and native
IJG/BSD/zlib notices were reviewed and retained in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Cargo-deny passed but does not
inspect the external C library. Windows/MSVC preparation and CI are configured,
including library-name alias and DLL CRT selection, but Windows/remote CI were
not executed. This creates a native compiler/NASM/CMake prerequisite. The native
archive and static-library hashes are preserved separately from the application
source digest. Existing Linux libXi RUNPATH remains local to this checkout.

The wrapper does not expose a hard scratch-memory limit or header precision.
Progressive full-image coefficients and ICC marker copies remain possible;
12-bit decoding is rejected during decompression. CMYK, lossless, arithmetic
and new unsupported subsampling codes are recoverable compatibility failures.
Neither path applies EXIF orientation or ICC color management. These prototype
restrictions must not be mistaken for a production import/media stack.

## Direct decoder measurements

Fresh process per codec, eight distinct synthetic JPEGs ×20 cycles. Timing
excludes source I/O and PNG encode/write, includes identical resize/RGBA. Native
`decode_ms` includes handle destruction while reference's does not; total is the
fair primary comparison. `/usr/bin/time -v` supplies process peak RSS; per-cycle
RSS is post-resize and misses scratch peaks. Additional photographic variants
and full-size progressive stress cases use the same isolated executable.

| Input set / variant | Samples | Header p50/p95/p99 ms | Decode p50/p95/p99 ms | Total p50/p95/p99 ms | Peak RSS MiB |
| --- | ---: | --- | --- | --- | ---: |
| micro / reference | 160 | 0.007/0.010/0.016 | 135.945/142.797/148.341 | 137.657/144.570/150.075 | 18.2 |
| micro / native | 160 | 0.009/0.012/0.020 | 65.927/67.525/69.964 | 67.646/69.307/72.222 | 18.3 |
| photo / reference | 120 | 0.004/0.008/0.010 | 38.633/92.635/96.943 | 39.342/93.334/98.517 | 23.6 |
| photo / native | 120 | 0.005/0.011/0.014 | 20.410/65.751/73.793 | 21.108/66.469/74.503 | 24.5 |
| progressive / reference | 40 | 0.007/0.018/0.019 | 467.454/486.356/487.817 | 485.521/505.886/505.977 | 171.5 |
| progressive / native | 40 | 0.010/0.012/0.023 | 370.002/377.029/378.706 | 371.764/378.853/380.469 | 170.6 |

Synthetic total median improves **137.66 → 67.65 ms (2.03×)**; total p99
150.08 →72.22 ms. Both decode 750×563 RGB (1,266,750 bytes), then retain
128×96 RGBA (49,152 bytes). Native does not make entropy decode disappear.
Synthetic thumbnail agreement PSNR is 47.02–47.11 dB; no channel-order defect
was observed. No production fidelity claim follows from this number.

## Cold end-to-end coverage

Percentages are visible-object/frame weighted, with tiny/medium/detail separated
and exclusive. Requested coverage means the requested level or a higher one.
These are all six prescribed native traces; no faster camera was slowed.

| Variant / trace | Placeholder % | Tiny % | Medium % | Detail % | Useful % | Requested % | CPU p99 ms | RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A / pan-normal | 24.3 | 21.3 | 36.7 | 17.7 | 75.7 | 17.7 | 2.529 | 520.6 |
| A / pan-fast | 31.2 | 51.2 | 17.6 | 0.0 | 68.8 | 0.0 | 1.555 | 418.6 |
| A / zoom-traverse | 70.5 | 23.2 | 3.0 | 3.3 | 29.5 | 26.9 | 0.631 | 478.5 |
| A / scan | 74.4 | 22.8 | 2.1 | 0.7 | 25.6 | 21.7 | 1.724 | 501.4 |
| A / board-tour | 97.5 | 2.5 | 0.0 | 0.0 | 2.5 | 2.5 | 1.567 | 347.2 |
| A / pan | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.314 | 341.4 |
| B / pan-normal | 19.4 | 20.0 | 37.8 | 22.8 | 80.6 | 22.8 | 4.698 | 563.1 |
| B / pan-fast | 22.5 | 40.7 | 36.5 | 0.4 | 77.5 | 0.4 | 1.382 | 497.9 |
| B / zoom-traverse | 58.6 | 34.6 | 3.4 | 3.3 | 41.4 | 38.5 | 0.868 | 490.3 |
| B / scan | 62.8 | 33.8 | 2.7 | 0.7 | 37.2 | 33.2 | 1.762 | 504.4 |
| B / board-tour | 94.3 | 5.7 | 0.0 | 0.0 | 5.7 | 5.7 | 1.277 | 353.2 |
| B / pan | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.468 | 339.5 |

Normal improves 75.7→80.6%, fast 68.8→77.5%, zoom 29.5→41.4%, scan
25.6→37.2%. That is a meaningful supply gain, but zoom/scan remain mostly
placeholders. Fast requested detail remains only 0.4%. The jump torture test
still delivers essentially no useful coverage at 20 distant jumps/second.

In the overview, **142→303 of 1,000** assets obtain a submitted recognizable
thumbnail during the trace; **858→697 remain censored**. Frame-weighted useful
coverage is only **2.5→5.7%**, not 30.3%. Most demand still waits too long for
images to be available while visible. Cumulative IDs served and live coverage
answer different questions and cannot be substituted for each other.

## Demand latency, including unresolved episodes

Percentiles below are conditional on resolution; every row also shows resolved
and censored counts. `—` means there was no resolved sample. Zoom/scan first-image
medians of zero can reflect a retained lower level on a new LOD-demand episode,
not instantaneous source decode. None/censored values are never converted to zero.

| Variant / trace | First image p50/p95/p99 ms | Resolved / censored | Requested LOD p50/p95/p99 ms | Resolved / censored |
| --- | --- | ---: | --- | ---: |
| A / pan-normal | 335.370/502.727/520.060 | 49 / 1 | 1091.823/1260.546/1261.231 | 32 / 18 |
| A / pan-fast | 167.910/385.822/438.575 | 171 / 5 | —/—/— | 0 / 176 |
| A / zoom-traverse | 0.000/1425.031/2397.254 | 156 / 416 | 284.979/1810.670/2397.254 | 114 / 458 |
| A / scan | 0.000/1677.205/2616.355 | 190 / 484 | 418.920/1744.807/2616.355 | 123 / 551 |
| A / board-tour | 4772.221/9648.549/9698.053 | 142 / 858 | 4772.221/9648.549/9698.053 | 142 / 858 |
| A / pan | —/—/— | 0 / 2074 | —/—/— | 0 / 2074 |
| B / pan-normal | 268.209/420.032/451.738 | 48 / 2 | 1042.988/1331.478/1341.395 | 39 / 11 |
| B / pan-fast | 100.813/285.042/302.245 | 173 / 3 | 826.627/826.627/826.627 | 2 / 174 |
| B / zoom-traverse | 0.000/1392.507/1997.213 | 227 / 345 | 326.170/1611.021/2047.427 | 178 / 394 |
| B / scan | 0.000/1544.946/2300.042 | 271 / 403 | 425.453/1813.179/2350.489 | 192 / 482 |
| B / board-tour | 5389.620/9638.089/9705.696 | 303 / 697 | 5389.620/9638.089/9705.696 | 303 / 697 |
| B / pan | —/—/— | 0 / 2059 | —/—/— | 0 / 2059 |

Normal first-image median improves 335→268 ms; fast 168→101 ms. However,
overview conditional median **worsens 4.77→5.39 seconds**, with p99 still
about 9.7 seconds: B resolves a larger and later-demand cohort. This is not a
latency win for cold overview, nor an uncensored comparison of the same assets.
The reduction in censoring and throughput improvement do not satisfy that gate.

## Repeats, renderer, cancellation and idle

The reverse-order repeat confirms normal coverage 73.1% A vs79.9% B.
Normal CPU p99 increases from 2.27–2.53 ms A to 4.70–5.02 ms B. More completed
images/refinement increase existing upload/render work; claiming no CPU cost
would be false. Across ten candidate traces, CPU max is 10.58 ms, GPU canvas
p99 ≤1.602 ms / max 1.825 ms. There is comfortable renderer budget despite that
increase. Renderer code was not modified.

All twenty native traces pass payload, queue, upload and SSD bounds. Regular
CPU/GPU payload ≤64/128 MiB; pressure ≤16/24 MiB; pending≤16, outstanding
submissions≤3, ≤8 uploads and≤16 MiB/frame, SSD≤512 MiB. No decode errors,
coverage/job telemetry loss or GPU backpressure were observed. Pressure repeat
coverage is 92.3% A vs 95.5% B. Stale work still terminates/discards at existing
stages; jump traces demonstrate cancellation, not successful navigation.

Presentation is a separate limit: candidate jump had one 81.35 ms native
present (callback 81.57 ms) while CPU canvas stayed below 0.67 ms. Reverse-order
repeat present max 20.56 ms B vs 21.49 ms A; other traces also have native-window
outliers. These measurements localize the delay outside the tiny canvas pass;
they do not establish an exact compositor/driver root cause or monitor latency.
The outlier is retained in evidence rather than excluded. GPU pass timing is not
whole-frame or presentation timing.

Ordinary interactive idle smoke, after six-second settling, sampled all-process
CPU ticks for three seconds: 0.02 CPU seconds A, 0.01 B. No continuous expensive
redraw was apparent. This short diagnostic is not a power profile; its owned
processes were terminated after observation because synthetic windowclose did
not deliver application exit. No user window or process was targeted.

## Memory and repeated-cycle behavior

Thumbnail source payload in these traces is ≤7,123,639 bytes; Vec capacity may
be larger. RGB output is 1,266,750 bytes and final RGBA 49,152 bytes. The native
write uses the Rust allocation directly: no additional native RGB buffer/copy,
although native marker/coefficient storage is separate. Resize can transiently
hold RGB thumbnail plus RGBA; PNG buffers are worker-local. Medium/detail can
still decode 20,250,000-byte RGB and retain 12,582,912-byte RGBA buffers.

Native baseline-JPEG microbench peak 18.3 MiB is comparable to reference 18.2 MiB.
Full-size progressive 4:4:4 / 4:2:0 stress reveals the real limit: native 170.6 MiB,
reference 171.5 MiB in fresh single-worker micro processes. Full coefficients
remain proportional to original dimensions; the 2-MiB output cap is not a native
scratch quota. Both workers can allocate scratch; large ICC/source inputs add
more. Conservative three-component 16-bit coefficient estimate is ~162 MB plus
rounding/auxiliary storage. This mission has not proven an absolute adversarial
process-RSS ceiling.

Twenty decode cycles per input set show no persistent monotonic native growth:
last ten synthetic RSS samples remain 17,504 KiB, photo 25,220 KiB, progressive
16,368 KiB, after allocator warmup/transient peaks. This supports correct handle
and buffer release for tested inputs, not a universal absence-of-leaks proof.

End-to-end maximum across all traces is 532.4 MiB A vs 574.8 MiB B. Cold overview
is 347.2 vs 353.2 MiB; faster normal/fast supply increases cached/refined working
sets and retained references. Bounds remain respected, but total RSS has not
fallen and remains a several-hundred-MiB prototype cost. Worker-retention peaks
are sums of historical worker peaks and overlap CPU payload; they are not
simultaneous extra allocations and must not be added to the cache budget.
Driver/swapchain, telemetry, scratch and allocator memory prevent exact RSS
attribution from these payload counters alone.

## Warm SSD regression

The exact same 1,000 Rust-derived Mission 0.5 PNGs (46,821,679 bytes) were copied
into separate A/B caches, without medium/detail representations. Each variant
uses a fresh process, empty RAM/VRAM and the identical input hashes. Original
Mission 0.5 cache is untouched. Both resolve 1,000/1,000 episodes, with 1,000 SSD
hits, **zero original source reads/decode**, no errors or censored demand.

| Variant | Useful/requested % | First image p50/p95/p99 ms | CPU p99 ms | RSS MiB |
| --- | ---: | --- | ---: | ---: |
| A | 96.63 | 67.365/839.265/873.229 | 2.784 | 359.5 |
| B | 96.61 | 67.560/839.473/889.930 | 2.701 | 359.7 |

The cached 128 PNG representation remains semantically compatible; no cache
version invalidation is needed. Native and reference scaled pixels differ
slightly but represent the same source/LOD. Native-generated cache reuse is also
covered by the loader test; this strongest regression uses the actual old entries.

## Malformed and photographic validation

Nineteen Rust tests pass, including five native integration tests, checked
layout rejection and seven ordinary loader tests. Native cases cover empty,
invalid header, truncated header/entropy/EOI, 65535×65535 dimensions, 12-bit
metadata, invalid scan marker, malformed ICC marker, >64-MiB input,
CMYK/lossless/arithmetic rejection, baseline/progressive grayscale/color,
odd wide/tall/small dimensions, RGB channel order and 1,000 deterministic mutations.
Valid mutated decodes must remain within output bounds; error or bounded valid
output is acceptable. Overflow/zero/quota are separately tested before allocation.
A malformed ordinary worker source is suppressed and its worker proceeds to a
valid next asset, without repeated retry. All test processes survived. This is a
bounded mutation pass, not exhaustive native fuzzing or a formal FFI proof.

Photographic sanity uses one 3072×2048 [Hopetoun Falls photograph](https://commons.wikimedia.org/wiki/File:Hopetoun_falls.jpg),
**DAVID ILIFF**, [CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/),
with six controlled original/reencoded/cropped/tagged inputs. Derivative images
remain under that license in ignored results, with attribution, and are not
committed. Baseline/progressive, 4:2:0 / 4:4:4, narrow 100×2048 crop and EXIF 6 were
checked. Thumbnail agreement PSNR 43.47–47.01 dB; montage inspected, waterfall
recognizable in both ordinary-aspect outputs. The extreme narrow crop is narrow
in both paths, not magically a useful overview. EXIF 6 and baseline pixels remain
identical, confirming stored-pixel orientation. No private imagery was used;
broader artist-rated usefulness and color-managed fidelity remain unvalidated.

## Quality and independent verification

`cargo fmt`, workspace/all-target/all-feature check and strict Clippy, all
19 Rust tests, docs, four Python corpus/harness tests, configured cargo-deny
and explicit hardware GPU smoke pass. The general suite intentionally ignores
the GPU test; it was subsequently run explicitly and passed on this RTX 2060.
Dependency checks report advisories/bans/licenses/sources OK, with existing
transitive duplicate-version warnings. Windows/remote CI were not run.
Log hashes are in the verification artifact, with retained raw logs.

Independent verifier `/root/mission0_verifier` reviewed wrapper/native lifetime,
checked arithmetic/allocations, same-slice ownership, error/drop behavior,
compatibility restrictions, cancellation, microbenchmark fairness and licensing.
No material finding remains. The verifier independently reran 13 asset tests,
four Python tests and fmt; recalculated 20 raw traces / 14,192 frames, 640 micro
samples, 16 PNG comparisons and all 32 report table rows; checked source/binary/
harness/manifest hashes, runtime equality, partition/queue/upload bounds,
censoring, 1,000 warm PNG identities and license notices. Its final verdict is
B — REPEAT with native selected and STOP before Phase 1. Independent command
outputs are retained in the tool conversation, not a separate disk audit log. The verifier specifically required documenting
scratch/ICC memory, unsupported precision/sampling, warning behavior and decode
sub-timing destruction asymmetry; these limits are explicit above.

## Selected path, rejected work and remaining gate

Retain the native 128 source path: measured improvement is reproducible, no
FFI copy complexity is added, errors recover, warm compatibility is strong and
memory behaves comparably for tested inputs. Retain the old decoder only where
it still serves medium/detail and the isolated measurement reference. Remove
no useful fallback silently; there is no runtime thumbnail fallback to retain.
Do not use the wrapper's older bundled native source, a bespoke unsafe bridge,
a broad decoder rewrite, additional workers, slower camera or prefetch tuning
to make this comparison look better. None of those changes was adopted.

The architectural limit is **availability of overview representations during
first navigation**. Even B produces only roughly 303 thumbnails in twelve seconds,
while 1,000 assets become demanded; the request/order/visibility window causes
most useful results to arrive late. Faster full-JPEG entropy decode improves
service rate but does not eliminate the initial representation deficit. This is
consistent with the same renderer showing ~97% coverage when thumbnails already
exist. Further decoder tuning alone is not the smallest useful next test.

Recommend exactly one experiment: **a bounded preparation of all 128-pixel
representations before first board navigation**, evaluated solely in the benchmark
harness. Measure its total source-to-overview preparation latency, peak memory,
SSD cost and first-display coverage against the existing cold trace, explicitly
charging preparation time rather than relabeling warm navigation as cold.
Use one disposable indexed thumbnail set and the existing two-worker/budgeted
loader; do not build production import, document persistence or UI in this test.
It asks whether moving original decode out of the navigation interval is a
credible product cost, not whether a faster microbenchmark is flattering.

Remaining risks are cold overview delay, heavy requested-detail censoring,
several-hundred-MiB RSS/native progressive scratch, native deployment/Windows
validation, presentation outliers and unvalidated artist media/orientation/color.
No production UI, annotation, server, collaboration, persistence or licensing
implementation was started. Repository changes are prepared for human review.

**B — REPEAT. STOP after Mission 0.6; wait for human review.**
