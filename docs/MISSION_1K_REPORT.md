# Phase 1K — LOD convergence and Preferences/Keymap

2026-10-07. Main Phase 1K brief and Preferences/Keymap addendum implemented.
Technical verification passes; owner acceptance on the original board is
pending. Stop here for human review. Collaboration and timed media are outside
this checkpoint.

Implementation: `e85d0e4b84a72968a3d3b0500c2c7355beb2a5b1`, based on
`9109cb52b2d9ca4bc041188153a003b0df395a5d`. Follow-up commits contain measurement
receipts and review only. Launch the atomically prepared `./bin/tack`; its
`./bin/BUILD.txt` records the exact commit, UTC build time and executable hash.
Its dirty flag remains true for the preserved, unrelated untracked `gfx/`
drafts; the tracked implementation matches the verified commit.

## Proven failure and repair

Two state errors were reproduced:

1. The former selector excluded resident tiers above the desired tier. With
   Thumbnail128 and Detail2048 resident, and Medium512 delayed, monotonically
   decreasing the projected edge through 513, 512, 129, 128 pixels displayed
   Detail, Thumbnail, Thumbnail, Thumbnail. Quality was adequate, inadequate,
   inadequate, adequate. The pre-fix regression failed at 512 pixels. Native
   wheel input also reproduced the intermediate hole: at roughly 329 projected
   pixels the old frame used no detailed representation, despite source 0's
   Detail still fitting in the GPU inventory. The diagnostic current receipt
   shows that same source displaying resident Detail while Medium is pending,
   then selecting Medium when it arrives.
2. Coverage accounting ran before later uploads. A real GPU test primes three
   Detail and four Medium textures (52 MiB) in the 64 MiB refinement partition.
   Inserting the fourth Detail could evict an earlier Detail after it had been
   counted as satisfied. The final inventory then contradicted the decision to
   stop refining. The regression recreates the eviction with the actual LRU.

Selection now chooses the smallest resident tier meeting admitted demand, or
the best lower fallback while refinement proceeds. Before insertion, only
current exact admitted residents are touched. Optional larger fallbacks are
not pinned. All uploads finish before final display/coverage/quality/pending
accounting. The admission plan fits its partition, so obsolete residency can
be evicted without protecting an oversized working set.

The appropriate smaller tier is still requested while a larger resident bridges
its arrival; failed smaller refinement does not discard useful resident detail.
Source/revision/tier identities and relevance checks remain authoritative.
Camera epochs describe diagnostics; they do not invalidate reusable pixels.
No global cache flush, recurrent full decode, forced redraw service, production
watchdog, queue expansion or permanent high-resolution pinning was added.

CPU tests sweep projected edges 1..4096 in both directions. Real worker tests
withhold publication across demand and revision changes, reject stale pixels,
then admit current Medium/Detail. GPU tests cover late eviction, lower-tier
insertion and repeated thresholds. Existing crop, grouped/shared-source,
source-size, DPI, rotation, fairness and admission tests remain in the gate.
The native fixture has eight PNG sources, a group, rotation, Smooth and Nearest,
rapid wheel reversals, progressive sweeps and a small pan. Every settled stage
must resolve all visible admitted demand; the watchdog belongs to the harness.

## Diagnostics and bounded work

`--lod-debug --output REPORT.json` explicitly enables a single-source trace:
projected size/edge, desired/displayed/resident/pending tiers, source revision,
worker, generation, publication acceptance, camera epoch and suppression reason.
Recent publications are capped at 16; active generations are bounded by workers.
Ordinary execution has no diagnostic heap allocation, timer, worker or output.
Output-only frame and wheel samples remain bounded to 7,200.

Existing defaults are unchanged: CPU display pixels 64 MiB, GPU texture payload
128 MiB, derived disk cache 512 MiB, two codec workers, 16 admitted requests,
one result slot per worker and 16 MiB upload payload per frame. Potato limits
also remain unchanged. These payload caps exclude source input, codec scratch,
driver and allocator overhead; they are not process RSS promises.

Successful clipboard negotiation now uses DEBUG metadata, without clipboard
contents. Normal launch/paste no longer prints its detailed negotiation.

## Flat themes and settings

Very Dark, Neutral Gray and Light are flat fills. The second background color
and WGSL interpolation were removed; the grid uniform shrank from 80 to 64
bytes. GPU readback checks three positions for each fill. Current design and
feature documentation were updated; earlier phase reports retain their history.
Twenty-four paired AB/BA empty-board pan runs, grid off/on at 800×600,
1024×768 and 1600×900, give slightly smaller median GPU pass costs after the
removal: about 11.4–11.7, 16.4–16.8 and 27.5–28.3 µs respectively. The differences
are under 1 µs in most pairs; this is timestamped pass cost, not CPU submission
or a claim of perceptible speedup.

Preferences provide bounded mouse -/+ and keyboard Left/Right for Handle Size
(3..21) and Hit Radius (5..32), with disabled endpoints and no wrap. Scale and
theme use direct choices. Current values have explicit markers, with cyan for
selection, magenta for editable actions and yellow for shortcuts/conflicts.

Keymap has visible case-insensitive search over the canonical 92-action catalog:
names, categories and active shortcut labels. Row clicks select; Change/Enter
starts capture. Conflicts name the requested shortcut and existing action and
refuse atomically. Cancel preserves assignments. Change, Unassign, Reset action,
Import and Export are visible. Global/category resets require confirmation.
Import/export reuse the existing validator, editable profile format and worker.
Logical AZERTY keys, imported physical controls, profile migration and menu
labels use the same existing semantic catalog/persistence path.

Six native cases cover all three themes at 800×600, 1×/2×, with 62 successful
checks including mouse/keyboard persistence, conflict, reset cancellation and
actual picker import/export. Light uses the French layout and captures a logical
Ctrl+Alt+Q. Selected details remain available when an unselected row truncates.
Search costs on this host: 3.44 µs for an empty scan, 25.45–27.47 µs for five
nonempty queries (10,000 scans each). No permanent widgets, index or thread.

## Before/after measurements

Ryzen 7 2700X, NVIDIA RTX 2060, native Vulkan, owned isolated X11 display.
GPU runs were serialized and CPU compilation excluded from performance runs.
These generated, warm-OS-cache fixtures do not establish low-end hardware or
human input-to-monitor latency. Selection measurements cover the whole existing
supply callback, including uploads, rather than an isolated selector benchmark.

The original baseline lacks dispatched-job/decoded-byte counters. A temporary
baseline from `9109cb5` adds only those counters and output-only wheel timings;
its source patch and hash are retained. Equal-instrumentation comparisons leave
LOD/UI behavior intact. Diagnostic runs are separate from CPU comparisons.
Original supply measurements additionally use both AB and BA ordering.

| View / workload | Supply CPU p50 / p99 before → after (ms) | Requests before / after | Decoded bytes before / after |
| --- | --- | --- | --- |
| 800×600, 5k | .01984 / .37238 → .02080 / .34774 | 32 / 32 | 29,573,120 / 29,573,120 |
| 800×600, sparse jumps | .01122 / .23797 → .01782 / .29140 | 21 / 21 | 16,220,160 / 16,220,160 |
| 1024×768, 5k | .03859 / 1.52828 → .02920 / 1.47648 | 52 / 52 | 34,078,720 / 34,078,720 |
| 1024×768, sparse jumps | .02792 / .44681 → .03040 / .45812 | 29 / 29 | 19,005,440 / 19,005,440 |

No changes in source bytes, cache peaks or queue peaks in these four comparisons.
GPU peaks respectively: 29,327,360; 16,056,320; 33,505,280; 18,841,600 bytes.
Pending peaks respectively: 13, 8, 16, 14. Requests all complete with no discarded
results in these supply runs. The comparable settled detail delays remain
roughly 83–479 ms, with startup/input scheduling variation. The original AB/BA
5k p99 ranges are .307–.384 ms at 800 and 1.366–1.623 ms at 1024. Sparse callback
differences are tens of microseconds and were not uniformly faster.

| Zoom oscillation | Wheel callback p50 / p99 before → after (µs) | Decode jobs / bytes before → after | Uploads / bytes (both) | CPU/GPU peak (both) |
| --- | --- | --- | --- | --- |
| 800×600 | .57 / 15.99 → .54 / 10.29 | 17 / 25,690,112 → 18 / 42,467,328 | 15 / 23,592,960 | 23,592,960 |
| 1024×768 | .41 / 5.23 → .39 / 4.65 | 21 / 61,341,696 → 21 / 61,341,696 | 18 / 42,467,328 | 42,467,328 |

At 800 one transient Detail job completed after rapid input made it obsolete:
one extra decode and 16 MiB of pixels, rejected rather than uploaded. This is
real bounded extra work, not claimed as a necessary corrective decode. Repeating
the pair in reverse order gives current 17 / 25,690,112 and baseline 18 /
42,467,328, with the same 15 uploads and cache peaks. Both versions therefore
span 17–18 jobs in these 800 runs. The separate diagnostic current run has 17
jobs / 25,690,112 bytes. Async admission depends on coalesced camera events.
At 1024 the work counts match exactly. Every current zoom
stage converges; final quality is 1/1 at 800 and 2/2 at 1024. Maximum observed
quality delay within a stable camera epoch, including initial loading, was
216.2 ms at 800 and 376.5 ms at 1024 (before: 223.5 and 377.1 ms). Interrupted
epochs are censored; these are CPU submission clocks, not display latency.

## Idle, footprint and verification

Eighteen final UI idle intervals (Preferences, Keymap, closed panel across six
cases) show zero redraw, zero I/O, zero RSS delta and no thread change. Fourteen
additional paired observations cover empty boards, application menu open/closed,
a 5k-image board, its menu and two simultaneous boards. They likewise show zero
redraw/submission, zero I/O, unchanged RSS within each interval and eight threads
per process in both versions. Heavy settled process RSS: 302.06 MB before,
301.43 MB after; empty: 242.98 MB before, 242.57 MB after. This is host/driver
inclusive RSS, not a low-end memory claim. Across these idle observations the
aggregate process reports 0–2 scheduler ticks; native/driver activity prevents a
claim of literally zero process CPU. No recurring Tack frame/codec service was
introduced. Post-zoom idle is also quiet in the ordinary untimed harness.

The stripped executable grows from 17,954,152 to 18,028,136 bytes: +73,984 bytes,
+0.4121%. The lockfile still contains the same 276 package identities/versions;
zero new external dependencies. Existing transitive tracing is reused directly.
The grid uniform is smaller; no new permanent GPU texture exists. Diagnostics
allocate only when explicitly enabled; new ordinary counters and empty sample
containers are fixed small state, not per-action or per-source services.

Measured stripped SHA256:
`20c8f368f51898e2a676a9e9be3192b0e2f0144c8bc1a46d0919ba7bee9ff45f`.
The human release executable retains debug information; its `.text` hash
matches the measured stripped executable:
`6ea0ba339c1dccec4821af05d3a23fb15c634d668aafffcf65b48fe21a7c0422`.
Raw release SHA256:
`f974402403a0e3b0c46d860b677d99c326ec0ce2db07078ed3c42b1703b9fc18`.

Local full gate passed: fmt, locked workspace all-target/all-feature check,
Clippy warnings denied, 176 Rust tests, docs, 13 Python tests, and cargo-deny
advisories/bans/licenses/sources. Ten GPU tests are explicit opt-ins in the
ordinary workspace test run. Explicit final Vulkan runs pass ten GPU tests
(12 suite cases including two geometry-only cases), including both LOD eviction
and threshold regressions. Native harness syntax checks also passed.

Exact implementation [Quality CI](https://github.com/gurppt/tack/actions/runs/37594215458)
passed for `e85d0e4b84a72968a3d3b0500c2c7355beb2a5b1`: Linux and Windows fmt,
check, Clippy, tests and docs; Linux software-Vulkan GPU tests including the new
LOD regressions; dependency/advisory/license checks. Windows runtime/GPU behavior
was not observed locally and is not claimed.

Reproduction tools: `lod_fixture`, `keymap_filter_cost`, `tools/run_lod_zoom.py`,
`tools/run_settings_checks.py`, `tools/run_lod_idle.py`, existing
`tools/run_supply.py`. Local raw logs/JSON/screenshots live in the ignored
`benchmark-results/phase1k-prefs/`; compact
[measurements](measurements/phase1k.json) and the
[counter-only baseline patch](measurements/phase1k-counter-baseline.patch)
accompany this report. Shared build targets were reused, phase-generated data
stayed below 512 MiB, and about 42 GiB remained free. Temporary counter-baseline
worktree/executable are removed after preserving hashes, patch and raw receipts;
one original baseline and one current measurement executable remain.
Independent review: [phase1k_lod.md](reviews/phase1k_lod.md), technical PASS on
the exact implementation with no unresolved production findings.

Use [HUMAN_TEST_1K.md](HUMAN_TEST_1K.md) to check the original problematic board
and final aesthetics. Automated generated-board convergence is verified; owner
acceptance of that original board remains pending. No next phase was started.
