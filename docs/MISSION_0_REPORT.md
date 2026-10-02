# Mission 0 — renderer experiment

Status: renderer experiment implemented and quality checks executed. Native
RTX 2060 and earlier software-GPU measurements are available. **The useful
60 FPS image-streaming gate is not established; do not start Phase 1.** GPU draw
cost is small, but fast pan/zoom image coverage remains insufficient.

## Environment

Measured 2026-10-02 on Pop!_OS 24.04 LTS, Linux
7.1.5-76070105-generic, AMD Ryzen 7 2700X (8 cores / 16 threads), 31 GiB RAM.
The measured native baseline uses pinned Rust 1.95.0 (59807616e, 2026-04-14), Cargo
1.95.0, release profile with debug information retained, NVIDIA GeForce RTX 2060
(6 GiB), Vulkan, NVIDIA driver 580.173.02, X11 desktop at 1920×1080 and a
1280×720 canvas. The AMD Radeon R9 290/390 also appears in PCI inventory but
was not selected by the Vulkan renderer. Network and GPU devices became
accessible after the user changed the session permissions.

The earlier restricted-session run used Rust/Cargo 1.99.0 and Vulkan with
llvmpipe (LLVM 20.1.2, 256 bits),
Mesa 26.1.6-1pop0~1787580452~24.04~a5619ea. This is CPU software rendering.
Process RSS therefore includes software GPU allocations as well as decoder
memory. All five scenarios use a 1280×720 offscreen target, paced toward 60
attempts/s, and ran for 12 seconds in separate processes. It predates the
toolchain/decoration changes and new GPU timestamp/native instrumentation;
its artifact records that historical source hash. It is not a measurement of
the final code or a controlled compiler/backend comparison.

## Architecture

See [architecture](architecture.md) for ownership and memory envelopes.
Four crates separate GPU-independent camera/geometry, asynchronous assets,
rendering, and native window/benchmark orchestration. Geometry is immutable;
linear culling and per-image GPU draws form the first measured baseline.
Display LODs are 128/512/2048 px on the longest edge. Lower detail or a placeholder
is shown while the requested LOD is missing. Full decoded originals never enter
the CPU display cache or GPU residency cache.

Two workers each have one job slot and one result slot. Current visible demand
replaces previous demand; atomic cancellation checks discard obsolete work
between codec/resize/cache stages. In-progress JPEG decoding itself is not
preemptible. All file access and image work run in workers, apart from bounded
manifest loading during startup and report writing after the event loop.

CPU/GPU cache payload limits are 64/128 MiB normally and 16/24 MiB under pressure.
Disk representations use per-worker quotas totaling 512 MiB. CPU and GPU caches
are LRU; disk cache eviction is by oldest write. At most two textures / 16 MiB
upload per frame and three outstanding GPU submissions are allowed. GPU pressure
skips submission while camera and asset scheduling continue. Optional timestamp
queries use three asynchronous fixed readback slots with bounded retained samples.

## Corpus and scenarios

Generated with `tools/generate_corpus.py`: 1,000 distinct asset IDs/paths, each
6000×4500 JPEG, arranged on a 32-column board with 6600×5100 spacing. Fixed seed
20261002, 32 generated RGB sources, Pillow 10.2.0, JPEG quality 90 with no chroma
subsampling. Each JPEG is about 6.8 MiB. Source hashes and generator version are
stored in the manifest. Files are hardlinked when supported, otherwise copied.
Each asset ID still exercises its own decode/LOD/display-cache identity.

This reduces physical corpus storage, but 32 synthetic sources do not reproduce
the diversity, compression profiles or OS I/O behavior of 1,000 unique photos.
The generator itself warms the OS page cache. “Cold” means no **Tack display
cache**, not a rebooted machine or dropped kernel filesystem cache.

- Cold: static initial viewport; no previous display cache. Four visible images
  progressively load thumbnails then detail.
- Warm: new process opening the same initial viewport, using only the display
  cache populated by the cold run; CPU/GPU caches start empty. This is warm
  **initial-region** coverage, not a fully cached 1,000-image board.
- Pan: twenty widely separated camera jumps/s; deliberately faster than decode.
  This is a cancellation stress test, more severe than continuous adjacent pan.
- Zoom: repeated overview/detail sweeps with visible demand changing throughout.
- Pressure: detail views over six successive regions, two seconds per region,
  reduced CPU/GPU budgets; eviction is verified explicitly.

Native scripted runs disable manual camera input and window resizing. An initial
native passage was rejected because the warm viewport changed from 4 to 72
visible objects through unguarded input; that harness defect was fixed before
the final passage. Rejected raw data is retained in the ignored
`benchmark-results/mission0-native/` directory for audit, not used as final proof.

Raw per-frame JSON, stderr logs and environment inventory are in the ignored
`benchmark-results/mission0-software/` directory. The compact committed artifact
is [mission0-software.json](benchmarks/mission0-software.json), including the
hash of the measured Rust/shader/manifests/lockfile source set.

## Native renderer baseline

The compact artifact is [mission0-native.json](benchmarks/mission0-native.json).
Raw frames, GPU samples, logs, environment and disposable caches are in
`benchmark-results/mission0-native-final/`. Source and release-binary SHA256
identities are captured by the harness. All five processes use a 1280×720 canvas
and run for 12 seconds, with FIFO presentation and a 60-attempt/s timer.

CPU time covers session scheduling/culling/uploads/submission. GPU queries
measure **only the canvas render pass**, excluding texture uploads and
presentation. Redraw callback time also includes surface acquisition and present;
interval p99 measures attempt cadence. None of these is a compositor/monitor
presentation timestamp. First image is the first image-bearing submission,
including startup; it is not first displayed photon latency.

Milliseconds unless indicated otherwise.

| Scenario | Startup | First image | CPU p50 | CPU p99 | CPU worst | GPU pass p99 | Callback worst | Interval p99 | Image coverage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cold | 457.2 | 853.5 | 0.136 | 0.389 | 6.04 | 1.454 | 26.61 | 16.93 | 93.8% |
| warm | 438.1 | 535.8 | 0.135 | 0.290 | 6.30 | 1.626 | 10.20 | 16.92 | 99.5% |
| pan | 456.3 | — | 0.149 | 0.269 | 5.35 | 0.055 | 16.40 | 16.93 | 0.0% |
| zoom | 497.3 | 887.1 | 0.199 | 0.880 | 5.33 | 0.846 | 23.75 | 17.13 | 4.9% |
| pressure | 475.3 | 863.8 | 0.134 | 0.708 | 8.81 | 0.197 | 19.95 | 19.43 | 84.1% |

| Scenario | CPU cache MiB peak | GPU cache MiB peak | Jobs peak | Stale jobs | CPU/GPU evictions | Process RSS MiB peak |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cold | 48.19 | 48.19 | 2 | 0 | 0/0 | 503.8 |
| warm | 48.19 | 48.19 | 2 | 0 | 0/0 | 354.9 |
| pan | 0.00 | 0.00 | 2 | 95 | 0/0 | 473.1 |
| zoom | 25.17 | 25.17 | 2 | 37 | 0/0 | 522.7 |
| pressure | 12.09 | 12.09 | 1 | 0 | 10/10 | 418.9 |

Every retained frame passes the harness's cache, upload, job and outstanding
submission limits, and every disposable disk cache remains under 512 MiB.
There are zero decode errors, failed surface acquisitions, backpressure frames
and dropped GPU timings. The 710–712 submissions per scenario have matching
asynchronous GPU samples. Completion throughput is 59.2–59.3/s under the imposed
timer, not proof of monitor FPS. Cold and warm retain four visible objects
throughout, confirming the fixed-camera correction. Warm records eight disk hits.
Pressure forces ten CPU and ten GPU evictions rather than merely approaching
its budgets. RSS is total process high-water usage, not a VRAM residency meter.

### Measured bottlenecks and limits

Native worker processing totals 3,212 ms cold versus 196 ms warm. It totals
22,747 worker-ms in pan (95 stale jobs, zero displayed images) and 19,959
worker-ms in zoom (37 stale jobs, only 4.9% coverage). Workers operate concurrently,
so these totals are not wall-clock run duration. The measured bottleneck is the
cold asset-processing path relative to rapidly changing demand; separate codec,
resize, PNG encoding and cache I/O costs are not yet isolated.

GPU canvas pass p99 stays below 1.7 ms, while CPU session p99 stays below 0.9 ms.
This supports the simple draw/culling foundation on this machine, but says little
about useful pan streaming: the fast pan run draws only placeholders. Presentation
can dominate native callback outliers: the cold worst callback is 26.61 ms,
including 26.42 ms in present, 0.164 ms session CPU work, and zero texture uploads.
The pressure worst callback likewise includes 19.83 ms in present with no upload.
These timings locate stalls at the native presentation boundary; attribution to
compositor, driver, scheduling or other desktop activity requires further profiling.
Driver-dependent upload costs remain visible in CPU worst-frame measurements.

The GPU and surface instrumentation was added before this final passage, rather
than treating submission time as GPU time. No sampling profiler was installed.
The earlier software measurements below remain historical evidence with different
compiler/instrumentation, not an apples-to-apples native speedup experiment.

## Earlier software baseline

Milliseconds unless stated otherwise. CPU frame time covers animation, culling,
channel polling, residency/upload orchestration, vertex preparation and GPU
submission. It is **not** GPU execution time. First content means first submitted
frame with a resident image; it is not monitor presentation latency. No GPU
timestamp precision is fabricated: that field is explicitly null.

| Scenario | Startup | First image submission | CPU p50 | CPU p99 | CPU worst | CPU cache MiB peak | GPU cache MiB peak | Jobs peak | Stale jobs | Image coverage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cold | 86.3 | 441.1 | 0.146 | 1.984 | 69.80 | 48.19 | 48.19 | 2 | 0 | 93.6% |
| warm | 88.6 | 158.4 | 0.141 | 4.538 | 53.10 | 48.19 | 48.19 | 2 | 0 | 99.4% |
| pan | 80.2 | — | 0.150 | 0.244 | 61.41 | 0.00 | 0.00 | 2 | 88 | 0.0% |
| zoom | 90.3 | 434.3 | 0.172 | 2.462 | 59.05 | 24.89 | 24.89 | 2 | 38 | 4.2% |
| pressure | 86.7 | 436.0 | 0.134 | 8.280 | 64.44 | 12.09 | 12.09 | 1 | 0 | 82.6% |

Image coverage is the aggregate fraction of visible object instances rendered
with an image rather than a placeholder. This prevents a fast placeholder-only
run being reported as useful image-streaming performance. All five runs have
zero decode errors. Pressure causes **10 CPU and 10 GPU evictions**. The runner
checks every sample against all cache, upload, job and submission limits.

Completed GPU submissions average 59.3–59.6/s under the artificial pacing; this
is offscreen completion throughput, not displayed FPS or a 60 FPS pass. Attempt
interval p99 is 16.8–17.3 ms except pressure at 24.5 ms. Peak process RSS ranges
from about 240 to 374 MiB across the five software runs, exceeding display-cache
budgets because decode scratch and software GPU memory are separate resources.

## Profiling findings from the earlier baseline

Worker processing wall time totals 3,542 ms in cold versus 214 ms in warm, with
eight disk-cache hits in warm. This locates a substantial cost on the cold asset
path, but does not isolate JPEG decode from resizing, PNG encoding and storage.
Pan spends 23,011 worker-ms on mostly cancelled work; zoom spends 19,428
worker-ms with only 4.2% image coverage. Current full-original decoding and
cancellation granularity cannot populate new regions fast enough in these tests.

The largest software CPU frame in every scenario is frame index 1 (53–70 ms).
Cold/pan/zoom/pressure have no image uploads in that frame. Further large frames
occur on or immediately after detail uploads; a 2048×1536 RGBA upload is 12 MiB.
Driver lazy initialization and upload handling are plausible contributors,
**inferences**, not isolated profiler measurements. Steady per-frame CPU work
is generally small; worst frames still matter. No sampling profiler was
available (`perf` was not installed). Native GPU timings/presentation intervals
and profiling are required before attributing these stalls or changing budgets.

## Review and discarded approaches

Independent verifier inspected thread boundaries, cache/queue limits, stale
work, benchmark validity and error behavior, and executed five core, four asset
pipeline and two corpus tests. Four material defects in the first implementation
were fixed rather than retained as alternate paths:

1. Pending GPU completion triggering continual idle repaint: completion polling
   is now separate from submission; an unchanged idle window sleeps.
2. Decoder allocation hints failing to bound encoded JPEG input: a capped read
   now precedes decoder construction, including protection against file growth.
3. Disposable disk-cache failures hiding valid sources: cache is disabled on
   failure while original decode/display remains available.
4. Native backpressure presenting an untouched surface: only rendered frames
   are presented and the dirty camera state remains pending after a skipped draw.

Telemetry now rechecks residency after uploads/evictions, and removed an
unnecessary RGBA clone from PNG encoding. There is one renderer/loader path;
no orphan alternate implementations remain.

Final review also checked the timestamp ring and native window code. Surface
acquisition failures are counted, presentation uses winit's pre-present
notification, and hidden/zero-size event states suppress redraw. An interactive
X11 check requested resize to 640×480 and restoration without a crash; returned
geometry was not independently confirmed. A two-second
idle sample consumed two CPU ticks with a 100 Hz clock. This is limited evidence
of sleeping, not a proof of zero wakeups. `xdotool windowminimize` did not produce
the window-manager HIDDEN state, so **actual iconification remains unverified**;
X11 may not emit the occlusion/zero-size events on every compositor. No synchronous
X property polling was added to the hot path.

Final independent verification recalculated source/binary hashes, compared all
five summaries with raw JSON, recomputed content coverage, checked complete
timestamp submission IDs, and verified every frame's resource limits. Table
values and the recommendation agree with that evidence; no further material
defect was found.

## Executed quality checks

The final baseline below ran with Rust 1.95.0; earlier offline checks used 1.99.0.

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --all-targets --all-features --locked`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
- `cargo test --workspace --all-features --locked`: nine tests passed; explicit
  GPU test is excluded from the ordinary run.
- Explicit software Vulkan GPU readback test: passed; uploaded red pixels reach
  the render target, exercising shader, upload, geometry and submission.
  The final explicit RTX 2060 test also passed, including asynchronous timestamp
  readback when supported.
- `cargo doc --workspace --no-deps --locked`: passed.
- Python corpus tests: two passed; shell script syntax checks passed.
- Full 1,000-image generator and all five software/native scenarios: executed.
- cargo-deny 0.20.2 licenses/sources/bans/advisories: passed with freshly updated
  advisory data; [audit artifact](benchmarks/dependency-check.json). Fourteen
  multiple-version warnings remain in upstream dependencies under the explicit
  `multiple-versions = "warn"` policy. Accepting these reports is a documented
  prototype exception for upstream dependency graphs; warnings remain visible
  and no forced cross-major substitutions were made. Compiler/Clippy warnings
  are still rejected.
  Path dependencies now have explicit versions. Optional Wayland client
  titlebar/font features were removed to eliminate the unmaintained ttf-parser
  dependency ([RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192)),
  without an advisory-ignore entry. Wayland relies on compositor decorations.
- Linux/Windows CI is configured; neither remote CI nor Windows execution is
  claimed as passed. Application license remains intentionally undecided.

## Subsequent X11 startup correction

The user's ordinary interactive launch subsequently crashed in system libXi
1.8.1 during X11 device enumeration. Core analysis identified reentrant
XI_RawMotion processing reading an uninitialized pointer. An optional app-local
libXi 1.8.3 preparation helper and Linux runtime search path now apply the
upstream fix without replacing system packages. Twelve corrected launches and
the exact interactive Cargo command succeeded on the RTX 2060; workspace quality
checks passed again. Synthetic motion did not reliably reproduce the original
crash in the old binary, so those launches alone are not a trigger regression
test. See [diagnosis and validation](X11_STARTUP.md).

The performance tables and their source/binary hashes above describe the
baseline before this startup correction. The separate startup validation does
not replace those measurements or change the Phase 1 recommendation.

## Risks and recommendation

**Do not proceed to Phase 1 on these results.** The renderer/thread/cache
separation is a useful working foundation, and the final native run tests a
real discrete GPU, but the fast-jump test's image starvation, low overview-zoom
coverage and native frame-cadence outliers leave the performance hypothesis
unresolved. Fast placeholder rendering is not sufficient validation.

Required next evidence: component profiling inside worker decode/resize/cache
write, controlled continuous navigation, full-board warm reuse, and compositor
presentation timing. Actual minimization and Wayland/DPI behavior remain to test.
Cache payload accounting is not total physical memory accounting; worker scratch
and driver allocation overhead remain risks.

The next renderer experiment should compare scaled/tiled image decode and a
retained/prefetched thumbnail tier, using both adjacent continuous pan and the
existing jump test. Measure image coverage alongside frame-time distributions
and full-board warm reuse. If draw/cull costs become significant, compare
texture-array batching and a spatial index against this baseline; do not assume
either is the bottleneck. Extend detail beyond 2048 only with bounded tiles or a
measured upload strategy. No collaboration, production persistence, media or
polished UI work has begun. Human review of this report is the stop gate.
