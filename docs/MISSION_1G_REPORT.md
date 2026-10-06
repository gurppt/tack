# Mission 1G — updated performance and low-resolution validation

**2026-10-06: automated local/native gates complete; B — human comfort/discovery
review pending.** The updated private brief is implemented and measured on the
current contextual-UI runtime. Linux/Windows/dependency CI is green for exact
commits `76e67c8`, `deeb0fe` and `b519b7f`. No concrete product correctness defect remains
from the independent source review. A human first-use/comfort result and a real
Windows desktop result are not fabricated. Stop for human review; no Phase 2 or
final global maximum-performance pass has started.

## Current result and provenance

Two additional supply issues were corrected: portrait preview urgency now uses
the same longest projected edge as refinement; a repeated SourceId may have at
most two queued jobs and one active codec. Other visible sources retain a free
worker/admission opportunity. Existing 16/4 global request caps, stale source/view
publication checks, preview-reserved worker, payload caps and single result slot
per worker remain. There is no extra scheduler, dependency, thread or cache.
The 25-alias/lower-priority small-source regression test completes the small
source among the first three jobs, then verifies eventual alias progress.

A bounded diagnostic `--window-size WIDTHxHEIGHT` initializes at the requested
physical dimensions and records the OS-granted viewport per frame. Ordinary
launch remains 1280×720 and untimed. This avoids confusing the desktop resolution
with the client viewport in performance reports. UI 1H and its common action/
history implementation are preserved; implementation details are in
[the UI report](MISSION_1H_REPORT.md) and [supply design](design/local_image_supply.md).

All current native receipts use binary SHA256
`4d21e4ea6322f9898abf82682513d9339cd9b0f1abd15ac894d1d76a697af6ee`.
Runtime Rust/configuration commit: `76e67c8c8dec923d0ddbf87ad81efd81cc3ef989`.
Harness/source snapshot commit: `b519b7fc5fdc7aabe2df415c71e0d15fa42df4f6`;
no Rust/configuration change between these commits. The 186-file source ZIP is
`ba0b4aa412ecea5ab3bb8a01dc488ea4e0c63bbf9bcb604ce5c869ff88d28cd5`,
retained at `benchmark-results/phase1g-updated-native/handoff/source.zip`.
`./bin/tack` is rebuilt atomically and byte-identical to the measured executable;
`./bin/BUILD.txt` contains the current finalization HEAD and timestamp. `bin/`
remains ignored and is never installed system-wide.

[Current compact receipts](../benchmarks/phase1g-updated.json) retain 1012 raw
artifact hashes/sizes, distributions, camera epochs, screenshots, exact CI jobs,
source snapshot and build stamp. Raw generated evidence stays under
`benchmark-results/phase1g-updated-native/` and
`benchmark-results/phase1g-updated-validation/`. Failed harness attempts are
explicitly excluded from acceptance: wrong click targeting/container comparison,
an incorrect 2× label, a missed Keymap capture, test-owned orphan picker detection, missing PATH xclip,
and a non-executable historical copy. They remain available for audit.

## Native performance at the three physical client sizes

AMD Ryzen 7 2700X, NVIDIA RTX 2060, Vulkan, isolated Xvfb without a compositor.
All GPU series were serialized. Warm OS caches, fresh per-session Tack derived
caches, generated fixtures with 64 shared image sources; 50k is an object-count
measurement, not 50k independent originals or large-edit readiness. The 800×600
series has 12 runs of 17 s; the other sizes have seven runs of 9 s. Raw frames,
events, GPU timings and RSS samples remain available. No physical-monitor latency
or exact low-end hardware emulation is claimed.

| Actual viewport | Runs | Ordinary 80%-recognizable startup ms | Peak whole RSS MiB | Peak CPU payload MiB | Peak GPU payload MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 800x600 | 12 | 405–520 | 328.3 | 49.22 | 56.60 |
| 1024x768 | 7 | 463–599 | 332.0 | 49.45 | 64.92 |
| 1600x900 | 7 | 534–946 | 340.3 | 49.96 | 65.47 |

The 800×600 p99 table separates query/visibility, supply selection, complete
callback, presentation and GPU pass. Percentiles are separate distributions;
the columns must not be added as though they describe one frame.

| Fixture | Query ms | Supply ms | Callback ms | Present ms | GPU ms | Peak RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1k | 0.202 | 0.285 | 26.35 | 26.05 | 0.888 | 298.1 |
| 5k | 0.052 | 0.314 | 25.20 | 24.96 | 0.430 | 302.1 |
| 50k | 0.392 | 0.305 | 28.41 | 26.55 | 0.484 | 328.3 |
| 10k-shapes | 1.597 | 0.004 | 27.54 | 24.18 | 0.079 | 249.3 |
| mixed | 0.226 | 0.342 | 24.94 | 24.49 | 0.639 | 316.5 |
| sparse | 0.049 | 0.237 | 26.01 | 25.65 | 0.489 | 284.2 |

At 1600×900, ordinary-image callback p99 is about 67.6–73.1 ms while present is
65.8–68.4 ms, query 0.05–0.29 ms and GPU 0.67–1.70 ms. This is strong presentation-
path attribution, not isolation of the driver versus Xvfb root cause. The 800×600
immediate-present diagnostic still has about 24.8 ms present p99; it does not
establish a platform-tail fix. Oversubscribed overlapping images are different:
GPU p99 remains 23.50/22.90 ms in normal/potato mode. Overdraw is a real deferred
cost, not hidden under the platform explanation. The existing ordered visibility
memo is retained; no spatial index was added.

Camera epochs record first-recognizable and 80%-recognizable delay, queue/pending
before and after, decode/read/upload bytes, stale outcomes and payload endpoints.
For sparse 800×600, the first two supply epochs reach 80% about 21.5/20.6 ms after
the first submitted frame; the analogous 1024×768 epochs take 98.5/93.8 ms.
Later cached epochs often need zero additional frames. These are scripted camera/
zoom epochs, include an initial-open epoch and use shared fixtures; they are not
arbitrary cold-source click-to-photon claims. Unit tests additionally force
obsolete in-flight results and source changes; native shared-source traces need
not produce stale discards to prove the rejection rule.

Default payload/request caps remain 64 MiB CPU, 128 MiB GPU, 512 MiB derived SSD,
16 admitted requests and two workers; potato is 8/16/8 MiB, four requests and one
worker. All native peaks comply. The ordinary 800×600 potato 5k run peaks at
6.33 MiB CPU, 7.97 MiB GPU and four requests. The overlapping potato case takes
about 2.72 s to reach useful startup coverage and settles in about 5.66 s: useful
pressure behavior with a real cost, not an ordinary-workflow speed claim.

## Measured fairness cost and fixed overhead

Three alternated before/after pairs per fixture used the frozen preceding 1H
binary and current binary at the same default 1280×720, warm OS/cold Tack caches.
On a 4096×4096 PNG source, median first detail is **491.84 → 529.33 ms**; useful
preview is **416.68 → 410.53 ms**. Serializing a source's thumbnail/refinement costs
about 37.5 ms here. On sparse, first detail is **566.62 → 546.62 ms**, useful preview
**526.39 → 506.56 ms**. Three samples are not confidence bounds or proof of a
universal speedup. The fairness guarantee is worth the measured single-source
tradeoff; it is not presented as free throughput.

Matched empty 1280×720 observations: preceding 1H RSS 234.2 MiB, current closed
234.3 MiB, open canvas/application 234.3/234.4 MiB, after dismissal 234.5 MiB.
The initial 1G comparison is 264.2 MiB; one observation does not establish a causal
RAM reduction. All six observations have zero late redraw/submits, file I/O and
decode-worker activity. Menu state adds no permanent thread/timer. The preceding
1H offscreen allocation/quad measurements remain narrower historical evidence.

Current release ELF: 114,874,416 bytes; fully stripped comparison:
17,804,776 bytes. Against 1H's 17,794,216 stripped bytes, this is
+10,560 bytes (0.059%). Against initial 1G, +118,936 bytes (0.67%). Human binary
retains debugging data. Cargo manifest/lock unchanged, 276 locked packages,
no new dependency, GUI framework, font engine, daemon or runtime network client.

## Idle, two instances, Save and retained disk debt

Six untimed supply views settle in roughly 1.6–5.7 s, then have **zero main-thread
CPU ticks/switches, decode-worker activity, file I/O, title/property changes and
late redraw/GPU submits** during five seconds. Whole-process RSS is 231.5–287.2
MiB and total CPU 0–8 ticks/5 s. Other threads, including named NVIDIA threads,
retain periodic switches/activity; total-process zero CPU is not claimed. Static
canvas/application menus and their closed state also pass untimed idle checks.

Heavy+heavy total RSS is 586.4 MiB (297.1/289.4); heavy+small is 553.1 MiB
(284.9/268.2). NVIDIA process graphics allocation is 88+88 MiB and 88+56 MiB;
these include driver allocation and are distinct from payload cache bytes.
Both pairs perform alternating pan/zoom, committed annotation input, separate
recovery and normal Save. Each has a completed recovery and Save, clean dirty
state, zero main-thread ticks and zero file I/O in the final five-second idle.
Other threads retain 1–2 ticks. No shared authority/cache coordinator is added.

The current storage-only 5k example repeats the 32 MiB synthetic blob measurement:
34,229,985-byte recovery snapshot, 112.29 ms synchronous storage-operation duration,
34,234,368 allocated bytes in the old unlinked container while history retains it.
The relinked current file is 675,365 bytes; Undo/Save verifies the original. Explicit
harness handle drop is not automatic production compaction. Whole-board recovery
and retained embedded-original history allocation remain measured debt.

## Low-resolution workflows, quality and review

**30 native context assertions** cover 800×600, 1024×768 and 1600×900: real size,
visible import, contextual edit, shared Undo/Redo, a long drag restored by one
Undo, a visibly opened Keymap panel, no source error and verified 2× UI scale. Captures include menu corners,
submenus, Preferences and Keymap at ordinary 1×/2×. The bitmap text and transient
layout were inspected; this is agent/automated visual evidence, not uncoached
human comfort/discovery. Some temporary panels scroll at 2×. Keymap physical/matcher labels can truncate;
search/rebind/export work, but their readability is not awarded a human comfort
pass. 4×/8× remain layout
stress rather than readability passes. No permanent panel is introduced.

**21 production assertions** run on a full native **800×600 desktop**, including
pickers, XDND, PNG clipboard, linked/embedded mode, note editing, preferences,
keymap export, Save As, close/reopen and recovery. One resize/unmap stress step
intentionally requests 820×560, so that step is not claimed as an 800×600 fit
check. **19 recovery assertions** run on a **1024×768 desktop**, including shared
missing-source fallback, relink Undo/Redo, failed/cancelled Save, close lifecycle
and invalid recovery. Actual 800×600 picker captures are 800×384 and 800×372;
there is no inaccessible file-picker acceptance control in these flows. Native
automation proves access; the new-user discovery checklist remains pending.

Selected captures: [800 image menu](../benchmarks/phase1g-updated-ui/800x600-image-menu.png),
[800 Preferences 1×](../benchmarks/phase1g-updated-ui/800x600-preferences-1x.png),
[800 Preferences 2×](../benchmarks/phase1g-updated-ui/800x600-preferences-2x.png),
[800 corner 2×](../benchmarks/phase1g-updated-ui/800x600-corner-2x.png),
[1024 Keymap](../benchmarks/phase1g-updated-ui/1024x768-keymap.png),
[1024 Preferences](../benchmarks/phase1g-updated-ui/1024x768-preferences-1x.png).

Local full quality passes: fmt, locked all-target/all-feature check, Clippy
warnings denied, **144 workspace/doc-test passes**, rustdoc, **nine Python tests**,
fresh cargo-deny advisories/bans/licenses/sources. Hardware Vulkan product tests
**7 PASS** and smoke **1 PASS** were rerun (two product tests overlap workspace
coverage). Configured Linux/Windows/dependencies Quality is confirmed green at
[76e67c8](https://github.com/gurppt/tack/actions/runs/37380755187),
[deeb0fe](https://github.com/gurppt/tack/actions/runs/37420440802) and
[b519b7f](https://github.com/gurppt/tack/actions/runs/37421904943). Initial `fae987c`
CI is now also confirmed green; the old access/CI blocker is closed.

Independent verifier `/root/phase1g_updated_verifier` reviewed the source fixes
and found no additional concrete issue. It caught the initial false 2× label,
and the initially missed Keymap panel, both fixed and asserted before acceptance. Final evidence review is recorded
in [VERIFICATION_1G.md](VERIFICATION_1G.md). Human Linux first-use/comfort,
physical monitor/DPI switching and actual Windows desktop observations remain
pending in [1G](HUMAN_TEST_1G.md) and [1H](HUMAN_TEST_1H.md) checklists. This hardware
and its 230+ MiB fixed RSS do not establish 128 MiB/Pentium III readiness.
No ellipse, online feature or final global optimization branch was introduced.

---

## Historical initial 1G evidence — preceding runtime

The following original results use binary `13b294…`, not the current contextual
UI binary `4d21e4…`. Its then-pending Git/CI status is historical and superseded
above. Preserve these receipts as the before/after and implementation record.

# Mission 1G — local supply/performance consolidation

**B — implementation and local verification complete; final CI confirmation and
report commit/push blocked by the resumed session's permissions.** Runtime and
measurement code are already pushed through `fae987c1f9006881b846b3ac171c6f10faa5a686`.
The independent verifier found no unresolved concrete correctness issue. A — PASS
requires confirmation of that exact commit's configured Quality run. Stop here
for human review after the gate; Phase 2 and the final maximum-performance pass
have not started.

## Result and implementation

Ordinary product views now supply projected 128/512/2048 image representations
asynchronously, including crop demand, source-shared refinement, obsolete-view
rejection and stable budget admission. Dense views can reduce overview edges to
64/32/16/8px. Smooth/Nearest remains independent of representation selection.
Only schema-compatible 128px previews may enter Save/recovery authority.

The bounded main queue is replaced by current demand, without a hidden worker
FIFO. Priority is large visible previews, smaller visible previews, visible
refinement, then at most sixteen near-view previews. Background generation is
disabled in ordinary idle. With two workers, one is reserved for previews.
Already running codecs finish safely, but obsolete/revision-mismatched results
cannot publish to the current CPU/GPU view. Hidden windows and shutdown revoke
queued demand and drain only active codecs. A single-worker potato view accepts
one active-codec delay rather than interrupting an unsafe decode.

An ordered ObjectId/AABB memo above 4096 objects removes repeated BTree lookups
from stable camera scans. It is an O(n) scan, with about 3 MiB at 50k objects,
bounded by the document's 100k-object limit. Document identity, generation and
editor replacement invalidate it. Rebuild after an edit remains synchronous
O(n log n); selecting everything can remove its benefit. Smaller boards retain
the allocation-free scan. No spatial index or duplicate document was introduced.

Contracts, key identity, priority, decoder scratch limits and optional timing
scopes are in [the design note](design/local_image_supply.md).

## Human-test binary and provenance

Launch `./bin/tack` or `./bin/tack open PATH.tack`. Rebuild using
`bash tools/build-test-bin.sh`. The script installs only a successful release
build using atomic replacement, marks a failed/pending build STALE, and respects
Cargo's configured target directory. It requires no root or service. `bin/` is
ignored. The exact last successful build stamp is:

```text
STATUS: CURRENT
commit: fae987c1f9006881b846b3ac171c6f10faa5a686
profile: release
built_utc: 2026-10-05T20:39:56Z
binary: ./bin/tack
worktree_dirty: true
sha256: 13b294987327090b38c563001965b902c1a860529f30688ebd64d87d071a4a71
checkpoint: phase 1G — final local handoff; CI/Git access pending
```

The dirty flag includes untracked user gfx and local report work. User gfx was
not edited/staged; no private images or `.pur` input were used. The binary hash
is identical to the runtime used for all final native receipts. `a1ef665` and
`fae987c` added tests/measurement tools, with no later runtime change.
The final source archive contains182 tracked code/tool/CI/asset/config files,
excluding reports/receipts and user untracked gfx. Archive checksum is recorded
in [the artifact inventory](../benchmarks/phase1g-evidence.json). Raw files are
local in ignored `benchmark-results/`; compact receipts and hashes are prepared
for version control. This is a code snapshot, not a claim that final doc HEAD
has already passed CI.

## Measured larger-board evidence

Generated 64-source 1600×1000 images cover 1k/5k/50k image objects, 10k shapes,
1110 mixed objects with notes/annotations/frames/groups, and 5000 images split
between zones 80 million units apart. Extra5000-visible cases exercise wide tiny
previews and overlapping high-LOD demand. This is a shared-source corpus,
**not 50k independent-original readiness**. Sources and exact board/report
checksums are retained. Reproduction starts with:

```bash
python3 tools/generate_supply_images.py benchmark-results/my-supply-images
./bin/tack supply-scale benchmark-results/my-supply-fixtures benchmark-results/my-supply-images
```

Twelve 17-second final runs were serialized on owned X11 `:93`, Xvfb 1600×1000,
noncomposited xfwm4, Vulkan/NVIDIA RTX 2060. OS file caches were warm. These are
single-run distributions without confidence intervals, not old-hardware timings.

| Fixture | Metadata load ms | First 80% useful ms | Query p99 ms | Peak RSS MiB | Peak CPU/GPU payload MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1k images | 1.20 | 575 | 0.274 | 368.2 | 48.4 /49.3 |
| 5k images | 10.37 | 661 | 0.089 | 373.2 | 48.8 /52.0 |
| 50k images | 67.52 | 609 | 0.467 | 399.5 | 48.8 /52.0 |
| 10k shapes | 9.52 | n/a | 1.654 | 313.3 | 0 /0 |
| Mixed | 1.25 | 791 | 0.246 | 391.5 | 50.0 /65.5 |
| Sparse distant zones | 6.22 | 542 | 0.073 | 348.8 | 28.1 /28.0 |
| Potato5k | 9.52 | 719 | 0.067 | 323.1 | 6.9 /8.2 |
| Overlapping5000, potato | 10.02 | 3896 | 1.292 | 318.9 | 5.0 /5.0 |

First-frame submission was approximately418–513ms for these image views; most
startup cost was GPU/window initialization. All clocks are CPU submission before
present, not monitor latency. Each scripted jump episode starts at its first
observed callback, not physical input delivery. Cached jumps can resolve at 0ms;
unresolved epochs remain null/censored. Sparse jumps reached useful new images
without unbounded obsolete backlog. Epoch rows for static dense/overlapping views
are elapsed-time partitions, **not actual camera jumps**.

The 5000-visible wide view uses tiny previews; overlapping default demand admits
four detail sources /313 object references, potato admits eight medium sources /625
references. Remaining objects resolve at admitted overview quality. Potato's
first80% coverage is 3.90s; fully admitted quality settles at about 5.23s. This is
a finite resource-pressure fallback, not permanent refinement/eviction churn.

A paired pre/post memo comparison on identical physical fixtures measured 5k
query p99 **1.026→0.077ms** and 50k **11.398→0.434ms**. The first 50k memo build
still cost 10.26ms. Final shipping-query p99 is shown above. Snapping still uses
its established bounded candidate scan; this phase does not establish constant
cost for 50k edits/select-all/snapping.

[Supply/startup/latencies](../benchmarks/phase1g-supply.json),
[paired query distributions](../benchmarks/phase1g-query.json).
Raw frame/event/GPU/RSS distributions are retained, including tails and maxima.

## Presentation attribution

Optional diagnostics separate query, supply, scene, encode, submit, nonblocking
poll, acquire and present; event samples include complete callback durations.
Ordinary windows disable timestamp/readback profiling. Inclusive callback/CPU
fields must not be summed with their components. Worker decode timing includes
I/O, codec, PNG encode and cache work. Legacy counter scope is documented in the
design note; it must not be compared as an overview-only read count.

On the final sparse trace, callback p99 is 46.61ms, present 46.20ms, acquire 0.032ms,
encode 0.255ms, submit 0.148ms, GPU pass 1.17ms. Callback-minus-present p99 is 0.98ms.
AutoNoVsync on that same fixture gives callback 45.99ms /present 45.53ms. Avoiding
Vsync does not remove this path's blocking. The visible blocking boundary is
wgpu surface present/driver/window-system processing; component timings do not
identify one exclusive external root cause. The memo reduces a measured Tack
query cost even though the ordinary presentation tail remains.

The overlapping 5000 case is a counter-example to any global external-tail claim:
GPU pass p99 is 25.46ms, present 65.00ms, callback 76.43ms; potato GPU 22.59ms and
callback 69.46ms. Overdraw and several milliseconds of supply/encoding are real
Tack costs. Neither ordinary nor stress measurements prove physical display
latency, low-end readiness, or that all remaining tails belong to Xvfb.

## Working-set and idle audit

| Owned budget | Default | Potato |
| --- | ---: | ---: |
| CPU display LRU pixels | 64 MiB | 8 MiB |
| GPU cache texture payload | 128 MiB | 16 MiB |
| Derived SSD files | 512 MiB | 8 MiB |
| Decode workers | 2 | 1 |
| Queued+active requests | 16 | 4 |
| Upload bytes/count per frame | 16 MiB /8 | 1 MiB /2 |

All 12 runs stayed within CPU/GPU/request caps; queue peaks were 16/4 including
active work. Linked reads remain limited to requested source representations,
not all originals. Source/revision keys safely share refinement; stored previews
remain asset-specific. Negative revision states settle without permanent retry.
Deterministic tests cover obsolete result rejection, eviction, overtaking old
views, reduced-budget first-hit cache trim and suspended/shutdown queues.
Sampled SSD peaks during extra default/potato camera stress were 7,463,279 /
5,087,841bytes, with 0 remaining after close. 50ms sampling is not an instantaneous
peak proof; worker admission/trim tests enforce the policy between samples.
Details regenerate on reopen because this cache remains per session.

Payload budgets exclude codec scratch, compressed inputs, up to one staged
result per worker, allocator overhead, original file handles and driver memory.
Existing input bounds include 64MiB compressed input and 192MiB PNG scratch per
active decode; up to three GPU submissions can retain evicted resources. They
are not total-RSS/driver-memory caps. Decoder and metadata limits remain explicit.

Six final untimed native views waited for an externally observed quiet main
thread/I/O interval before a 5s observation. Empty, small, 1k, 5k, dense-potato and
overloaded-potato each had **0 main-thread ticks/context switches, 0 file I/O,
0 title/property changes and 0 late redraw/submit**. Decode workers also had
0 ticks/context switches. Dense/overloaded potato took 4.58/6.59s to reach the
verified observation start. An earlier fixed 4s probe failed while supply was
still completing; that failed receipt is retained rather than labeled idle PASS.

Matched 1F binaries on the same files/host had 8 threads, as do default 1G windows;
potato has 7. Fixed empty RSS is 300.2→300.5MiB. Small/1k/5k rise from 300.6/301.1/
303.2MiB to 333.2/354.1/356.2MiB, with 21/42/42MB resident display payload versus
old overview-only 82/164/164kB. This is a measured active-board quality cost,
not a claim of lower RAM. Process idle CPU also increases: 1k/5k now 10/9 ticks
in 5s at 100Hz, **2.0/1.8% of one core**, versus 2/1 ticks on 1F. Most new work is
NVIDIA `[vkps] Update`; other driver threads retain recurring wakeups. Tack's
main/decode threads remain asleep, but total process idle cost is not unchanged.

[Idle and matched baseline](../benchmarks/phase1g-idle.json),
[cache/disk audit](../benchmarks/phase1g-disk.json).

## Multi-instance and production/recovery correctness

Two-heavy and large+small native pairs alternated pan/zoom/reversal, resize and
committed annotation edits. Both windows in each pair independently created a
recovery then performed a successful Save, became clean and preserved their own
file authority. Five final idle seconds had 0 main-thread CPU and 0 file I/O in
all four instances; each process used 8 threads and 2 CPU ticks. Heavy+heavy RSS
sum was 749,907,968 bytes, driver allocations 88+88MiB; large+small 727,412,736 bytes,
88+56MiB. These are sums for the two Tack processes, not attributed whole-OS RAM
or driver-global allocation. No shared process, global coordination or daemon
appeared. Actual scoped window raising/focus ensures each intended window was
edited; an earlier automation attempt with a covered first window is not used
as final evidence.

Final runtime repeats 21 native production checks and 19 recovery/error checks,
all passing. They cover actual picker, XDND, PNG/text/file-URI clipboard,
progressive import/note draft, preferences/keymaps, resize/unmap/restore,
SaveAs, ownership conflicts, crash recovery, close Cancel/Discard and corrupt/
future/locked failure paths. Existing deterministic 7 local-production tests
include recovery and Save while a medium representation is actively supplied,
edits during Save retaining dirty generation, relink Undo/Redo and original
retention. Supply completion cannot change authoritative edit generation.
Manual relink/import automation and heavy-instance recovery supplement that
worker test; there is no claim of a timed large-board autosave/input latency
benchmark. Autosave remains a separate worker/snapshot authority.

Storage-only 5000-object measurement uses one fully written **32MiB synthetic
blob**, not a real photo or decoder benchmark. Initial board 34,229,666 bytes;
one recovery snapshot 34,229,985 bytes. After valid PNG relink/save the current
file is 675,361 bytes, while Undo retains the old unlinked container inode with
34,234,368 allocated bytes (`nlink=0`). Undo then Save verifies the original.
Explicit harness drop releases its old handle; this does not prove automatic
history compaction. `recovery_worker_ms=112.68` in this receipt means synchronous
storage-operation duration in the isolated harness, not UI worker/input latency.
Snapshot size is not total two-slot recovery directory or cumulative bytes
written. Native linked heavy recoveries were 1,738,233 /1,198,233 bytes; the small
embedded recovery 57,418 bytes. Whole-board snapshots and retained history disk
cost remain real limitations; no compaction subsystem was added.

[Native workflows/instances](../benchmarks/phase1g-native.json),
[storage example and measured debt](../benchmarks/phase1g-disk.json).

## Gates, dependencies and independent review

Local `tools/check.sh` completed successfully: formatting, locked all-target/
all-feature check, Clippy warnings denied, 133 workspace/doc-test passes, docs,
9 Python tests and cargo-deny advisories/bans/licenses/sources. Existing duplicate
version warnings remain warnings. Hardware Vulkan product GPU tests 7 PASS and
GPU smoke 1 PASS include medium-tier crop/alpha/rotation/flip/Nearest/Smooth checks.
No runtime font engine or new GUI/telemetry/network/scheduler dependency was
added. Cargo manifest/lock remain unchanged; package-version delta 0.

Unstripped release ELF 114,109,328 bytes; strip-debug 20,913,712; fully stripped
17,685,840. Matched 1F sizes 113,568,768 /20,814,152 /17,588,432bytes: debug-stripped
increase 99,560 bytes (0.48%), fully stripped 97,408 bytes (0.55%). The measurement
example is a separate executable, not linked into the ordinary client.

Actual configured Linux/Windows/dependencies CI passed on `a1ef665`,
[Quality37279146426](https://github.com/gurppt/tack/actions/runs/37279146426).
The later measurement-only `fae987c` run
[Quality37280707711](https://github.com/gurppt/tack/actions/runs/37280707711)
was last observed in progress, dependencies successful; its final status cannot
be retrieved after resuming with restricted terminal networking. The branch's
previous green run is not represented as proof of the latest code commit.
The resumed sandbox also mounts `.git` read-only: staging reports fails to create
`.git/index.lock`. Final report/evidence commit and push therefore remain pending.
These restrictions are from the current execution profile, not a request for
new user authorization or a product defect.

The independent verifier `/root/phase1g_verifier`, which did not implement this
phase, reviewed supply/staleness/bounds, memo invalidation/order/live selection,
idle, native hash/receipt consistency, pressure/instances/recovery, retained
inode measurement, binary and CI. Its concrete findings during development were
fixed: overbudget refinement churn, first-hit trim, suspended/shutdown demand,
crop need, near-view priority and acquire-clock scope. Final raw receipt hashes
and peaks matched; no unresolved correctness issue remains. Final PASS is
conditional on latest CI confirmation and saving the completed report.
The final report audit is complete: [independent verdict](VERIFICATION_1G.md).

[Quality/build/dependency receipt](../benchmarks/phase1g-verification.json),
[CI receipt](../benchmarks/phase1g-ci.json).

## Human observations and stop

No human-observed Linux/Windows visual validation is claimed. Automated X11 and
hardware/offscreen tests do not establish monitor latency or native Windows
clipboard/helper/DPI behavior. The Windows CI compile/test gate exercises shared
helper/path/keymap/scale logic; physical Windows manual checks remain pending.
Use [the human checklist](HUMAN_TEST_1G.md) with `./bin/tack`, including actual
DPI/UI scale, high-LOD appearance, sustained use and long idle.

Low-end readiness, 50k independent originals/edits, presentation platform root
cause, overdraw reduction, permanent history compaction and the final global
maximum-performance pass remain deferred. Stop after 1G and human review.
No collaboration/server, account/service, `.pur`, media, plugin, rich-text or
ellipse branch was introduced.
