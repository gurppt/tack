# Mission 0.7: overview preparation experiment

Scope: measure the existing 128-pixel JPEG → PNG tier for the immutable synthetic
board, before/during scripted navigation. This is benchmark code, not an import
framework or a choice of production UX. No dependency or renderer changes.

`OverviewPreparation` owns stable ID-ordered requests and bounded
sets of historically validated ready IDs and terminal failures. It consumes
the loader's bounded worker profiles; CPU eviction does not reset disk progress.
A hit counts only after PNG decode validation; a miss counts ready only after
temporary-file write and successful rename. Cancellation after persistence still
counts ready, even if the RAM result is stale. Cancellation before persistence
is retryable; a write-disabled cache becomes a preparation error while ordinary
image display can still succeed. “All settled” does not mean “all prepared”.

The standalone `overview_prepare` example requests the remaining board in stable
order through the existing two-worker pool. At a target percentage it records
the threshold, cancels remaining jobs and explicitly drains results. The elapsed
preparation cost starts before parsing/loading the manifest and cache startup
cleanup, and ends after that drain. Codec calls cannot be preempted. A partial
input can therefore contain slightly more than the threshold; the harness
records the actual input cache inventory and digest.

During a scripted benchmark only, ongoing preparation shares the ordinary
pool. Visible tiny/medium/detail requests retain their existing priority and
are admitted first. At most one background thumbnail per worker is active,
queued or not yet polled; it counts against the unchanged eight-job total.
Foreground promotion of an active background key does not duplicate work.
There is no second pool, unbounded submission or frame-path filesystem probe.

The controlled scheduling comparison changes only the remaining background
order: stable global IDs versus assets intersecting a viewport expanded by half
its width/height on each side, ordered by distance to its center, followed by
stable IDs. Foreground order and navigation speeds remain the same. This is
geometry preference, not motion prediction.
It showed no consistent coverage advantage and higher CPU overhead in overview;
that variant and its selector were removed. The retained benchmark flag is
`--prepare-overview`, with stable remaining order and visible foreground demand
always admitted first. The old comparison executable/source ZIP remains only
in ignored evidence.

The measurement harness uses Unix `resource` process CPU counters; these
experiments run on Linux. Windows native builds/CI remain unexecuted here.

Each 0/25/50/75/100% input is prepared independently from an empty derived cache.
Every navigation trace gets its own exact copy of that input, so earlier traces
cannot silently enrich later traces. Background preparation continues at partial
starts and is measured explicitly. Fully prepared first navigation has a fresh
process, empty RAM/VRAM and only 128 PNGs on disk. Subsequent warm reopen uses the
same trace's post-navigation cache, including any refinement levels it produced,
with fresh RAM/VRAM again. Kernel page cache is not flushed; “empty cache” refers
to derived files. The 1,000 IDs still share 32 hardlinked contents.

Timings and source/cache byte counts are process observations, not physical
device I/O. Derived write volume counts successfully committed PNGs; attempted
write sizes are recorded separately, not claimed as physical bytes transferred.
Worker utilization is observed active job wall time divided by
worker capacity; it is not CPU utilization. Common-clock decode intervals
measure simultaneous codec calls, including failures, not native allocations.
RSS high water includes source buffers, native progressive coefficients,
unpublished/retained pixel buffers, telemetry and, during navigation, driver
allocations. Worker payload peaks overlap cache payload; they cannot simply be
summed into an exact allocation total. A separate synthetic progressive fixture
checks the two-versus-four coefficient-memory multiplier.

The 512 MiB disposable disk budget has four fixed shards. Preparation progress
is historical, not a live disk inventory: estimates beyond the quota do not
establish that all images remain prepared. A later scalable persistent overview
store requires a separate design; no such persistence is implemented here.

Raw reports, binaries, source snapshots and cache inputs stay under ignored
`benchmark-results/mission0_7-*`; compact evidence and conclusions are
linked from [the mission report](../MISSION_0_7_REPORT.md).
