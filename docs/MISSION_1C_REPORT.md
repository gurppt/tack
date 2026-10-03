# Mission 1C report

2026-10-03. Starting main/origin `3068083d07242ce2e053dcf7f82762959fdd65fe`.
Phase 1B provided the real local Document/typed IDs, indexed `.tack` snapshots,
worker-prepared previews, common image renderer and semantic input foundations.
Required normative/context documents were read in full before implementation;
no conflict found. This phase adds direct image manipulation and Save, without
starting Phase 1D. User-owned `gfx/Untitled.png` remains untouched/untracked.

## Incident and recovery

The user reported a full workstation freeze and restarted it at about 11:20 CEST.
[Incident record](INCIDENT_2026_10_03_DESKTOP_FREEZE.md) retains the chronology,
system evidence and attribution limits. An actual prototype defect repeatedly
called `set_title` in the idle loop, flooding X11 property events; process CPU
was observed at 70.3%. One cached title owner now updates only changed status.
Post-restart idle ticks remained stable, and completed native/GPU/benchmark
processes exited normally without another freeze. The full freeze's root cause
is **unconfirmed**, not declared resolved. No matching OOM/NVIDIA Xid/Tack dump
was recorded. A separate old `/dev/sdc` sector-read fault exists on an unmounted
secondary disk; the repo resides on `/dev/sdb3`, with no proven connection.
No global driver/desktop/disk changes were made.

## Reference research and deliberate choices

[PureRef research](research/pureref_image_interaction.md) was written before code
from authoritative handbook pages and the legally installed PureRef 2.1.3 on
owned generated grid images. Native observations confirmed corner aspect lock,
opposite-corner behavior, non-stretching edge crop and image focus. Alt/center
and Escape observations affected by window-manager interference were marked
inconclusive, not invented. No proprietary source/assets/icons were copied.

Tack uses restrained square resize handles plus a visible rotation stem. Corners
keep aspect, edges resize one axis; Alt on a handle uses the center. Multi-image
resize stays uniform about a world-axis selection rectangle, mixed rotations
are preserved, and group rotation uses that rectangle's center. Crop is single
image, with no square/ratio modifier, snapping or arrangements. Double click fits
the image without PureRef's second-click camera restore. Sampling cycles
Default/Smooth/Nearest; Default currently resolves to Smooth. A global preference
editor and move-axis constraints are deferred.

## Architecture and usable interactions

[Contract](design/image_interaction.md) separates authoritative DocumentEditor,
transient selected ObjectIds, captured gesture/preview, local Camera and output.
There is no second document, transform model, renderer or storage path. Reverse
object order chooses the topmost transformed geometric quad; transparent pixels
are not hit-tested. Selection never changes z-order or enters the file.
Empty click clears, Shift toggles, and empty drag selects by marquee/SAT geometry.
A selected-image drag moves the whole selection with exact relative spacing.

Move/resize/rotate/crop/opacity snapshot only selected existing ImageRenderData
at begin and reuse preview storage on every update. Preview is substituted before
visibility culling. Release applies one flat bounded batch of existing semantic
image commands; failures roll back atomically, undo reverses in order, redo is
exact. A 300-update gesture still creates one history entry. History is bounded
by 200 entries and 32 MiB retained command storage. Crop changes UV, world size
and center together, keeping retained source pixels stationary even with flips
and rotation; it can expand back to original UV bounds. Invalid/nonfinite updates
keep the last valid preview. Source revisions and pixels do not change.

Flips, filtering, Delete, Select All, Undo/Redo and single/group opacity are
reachable through the semantic keymap. [README](../README.md) lists the actual
preset and native create/open commands. Ctrl+S saves committed state; double
click changes only Camera. Escape/focus/capture loss, minimization, DPI/viewport
changes and navigation cancel preview. Middle pan takes priority in both button
press orders; late Alt and wheel continue to work. Alt+left off handles pans;
Alt+left on a handle resizes from center. No scattered raw-key gesture path lives
in the window. Captured release tokens survive binding/modifier changes.

Logical handle size 7, hit radius 9, line 1 and rotation offset 26 are centralized
and multiplied once by native scale. Physical→world/local/UV conversions use f64;
nearest handle wins when screen targets overlap at low zoom. The common GPU
canvas pass draws at most 128 overlay quads using fixed reusable buffers, with
no per-image widget tree, source identity or persistence state in the renderer.

## Save and exact round-trip

One owned worker captures a metadata snapshot plus edit generation on Ctrl+S;
no request queue or pointer-event save work. Acknowledgement clears dirty only
for the exact saved generation. Edits during Save stay editable and dirty. Save
during preview persists only committed state and leaves the preview alive.
Failure reports an error and leaves earlier valid storage intact; close does not
autosave and warns about unsaved edits. The shutdown path also surfaces save errors.

Original bytes and valid persisted previews are streamed from stable opened file
handles. Repaired previews are CRC-checked and pinned on the save worker with at
most 128 extra handles; disappearing/excess disposable previews may be omitted.
This removes the reviewed cache-eviction race without source reimport. Atomic
publication/recovery guarantees remain those documented by Phase 1B; this is
whole-snapshot replacement, not in-place small-blob updates.

Four app persistence tests cover move/resize/rotate/crop/opacity, flips/filtering,
multi-edit, delete/undo then save/reopen, exact-generation and failed-save state,
and unlink after overview pin. Existing storage round-trip/recovery tests pass.
Neither selection, gesture nor camera is serialized. Native generated-board saves
finished clean at generation 14, four undo entries, one acknowledged save, on both
ordinary and missing-source/DPI2 paths. Both edited files were subsequently
reopened in fresh native processes: zero errors, clean document, zero selection
and zero undo/redo state (`phase1c-edited-reopen/`).

## Native interaction observations

Real Linux/X11 pointer/key input and screenshots were exercised on owned two-grid
boards via `tools/run_native_image_checks.py`, then inspected. This is agent-driven
native interaction evidence, **not** an artist/tablet evaluation or measured
PureRef latency equivalence. Final ordinary pass has 16 observed assertions;
DPI2 with a hidden owned linked source has 17, all true, normal process exit.

Concrete observations: click and Shift toggle target the intended image; marquee
selects both and empty click clears; drag moves immediately; corner resize
expands while the visible grid keeps ratio; the discoverable rotation handle
rotates the quad; crop removes right grid columns while left pixels/grid spacing
stay fixed. Escape restores pre-drag screen geometry. Multi-delete clears and
undo restores images. Middle pan works over selection and wheel changes coverage.
Filtering/flips/opacity are reachable, and Ctrl+S title acknowledgement matches
clean editor diagnostics. A missing link keeps its preview manipulable/saveable.
At requested DPI2, screenshots show 14-physical-pixel handles and 52-pixel rotation
offset instead of 7/26; resize/rotate/crop/pan still pass. Deterministic tests also
cover scales 1/1.25/1.5/2 at zoom .001/.1/1/64, rotated/flipped crops and low-zoom
handle overlap. An additional bounded native low/high zoom probe has eight passing assertions
(`phase1c-native-zoom-final/`): an image under 100 screen pixels wide can be resized by its
corner and undone/cancelled; at high zoom with edges outside the window, dragging
visibly shifts interior grid pixels and Undo/Escape restore those pixels exactly.
No input-file Save occurs. The first probe over-scrolled to a few-pixel image,
where selection handles cover the visible pixels; this was not a failed durable
edit, and is retained as a native usability limit. The final probe uses six down
and ten up notches (X11 emits several lines per notch). Fractional-DPI native artist
feel and usability of subpixel-sized images are not claimed.

One added screenshot assertion initially failed because selection overlay hides
one red edge pixel, while the reference had no overlay. It was corrected to compare
two selected states; native geometry and deterministic durable cancellation were
correct. The failed probe remains exploratory evidence, not reported as a pass.
The pre-reboot unmapped-window probes are likewise inconclusive. Window-manager
Alt bindings can intercept application shortcuts; rotation/crop handles give a
normal pointer path without changing desktop settings.

## Measurement method and provenance

Ryzen 2700X / RTX2060 / Vulkan / NVIDIA580.173.02, Linux7.1.5/X11, Rust1.95.0.
One GPU process at a time; no compiler running during final timings. Corpus: 1000
objects over 32 hardlinked generated JPEG contents, not 1000 unique photographs.
Kernel page cache is not flushed. Prepared container and fresh RAM/VRAM processes
are used for product gestures; renderer regressions use isolated fresh SSD caches.
No benchmark Saves its input board. CPU submission clocks are not photon latency.

[Interaction evidence](../benchmarks/phase1c-interaction.json),
[product evidence](../benchmarks/phase1c-product.json) and
[regression comparison](../benchmarks/phase1c-regression.json) retain distributions,
commands, hashes and raw roots. Frozen runtime binary SHA256:
`b11d3e5096c2e33234484afe050d03b89d3cbf9b0efa4dbae1b5c8401c5de978`.
Initial interaction source ZIP SHA256:
`14f95eced8c3bf00d7fd3a524388731ed1eb38c8c5ef2e4cadec4269f0c9da20`.
Final runtime Rust/WGSL/Cargo files match that frozen source. A supplementary
reverse-button-order test and native harness assertions were added afterwards;
they do not alter the measured runtime. Raw JSON/binaries/source ZIP/logs/captures
remain ignored under `benchmark-results/phase1c-*`; desktop logs are not published.

### Seven production interaction scenarios

Each 12-second native run includes startup and waits one second before gestures;
671–682 frames, ten verified commits followed by undo, or ten alternating
Escape/focus-loss cancellations. Percentiles are sorted floor(p×(n−1)). Input
samples include begin/commit/undo and selected-record setup; render CPU excludes
the driver, callback includes it. GPU timestamps cover only the canvas pass.
No marketing event-to-photon latency or frame-rate guarantee is inferred.

All times below are ms; Input/CPU/GPU columns are p50/p99/max, callback/present
are p99/max. RSS is sampled process high-water MiB, not a GPU-payload budget.

| Scenario | Input | CPU | GPU | Callback | Present | RSS MiB |
| --- | --- | --- | --- | --- | --- | ---: |
| drag | 0.002/0.134/0.179 | 0.280/0.439/0.810 | 0.049/0.074/0.084 | 2.838/16.041 | 2.493/15.757 | 319.6 |
| resize | 0.002/0.007/0.008 | 0.267/0.454/1.211 | 0.052/0.077/0.089 | 2.341/16.592 | 2.066/16.306 | 320.5 |
| rotate | 0.003/0.131/0.225 | 0.298/0.468/0.785 | 0.051/0.079/0.128 | 16.942/17.754 | 16.594/17.380 | 319.6 |
| crop | 0.003/0.007/0.009 | 0.320/0.454/0.799 | 0.047/0.069/0.085 | 2.999/15.594 | 2.633/15.231 | 320.2 |
| multi10 | 0.004/0.137/0.276 | 0.275/0.486/0.955 | 0.042/0.082/0.096 | 2.845/15.493 | 2.557/15.204 | 320.0 |
| multi100 | 0.019/0.214/0.299 | 0.360/0.602/1.335 | 0.045/0.084/0.130 | 13.657/16.833 | 13.224/16.432 | 320.6 |
| cancel | 0.002/0.133/0.205 | 0.276/0.525/1.381 | 0.048/0.106/0.166 | 3.079/16.045 | 2.770/15.709 | 320.2 |

Each run verifies one history entry per commit and exact undo/cancelled durable
records. Source reads are zero, peak asset pending 4, navigation-end pending 0,
with four persisted preview reuses. Pointer motion accesses no files, hashes,
decoders or waits. Allocator calls are **not instrumented**: storage reuse and
absence of growing event queues are code/test evidence, not a zero-allocation
measurement. Work scales with selected geometry and metadata, not source bytes;
this does not establish unmeasured 1000-selected/50k interactive performance.

Rotation's callback p99 16.942/max17.754 is visible. A separate identical 12-second
repeat gives p99 3.088/max16.678, render CPU p99 .382/max1.285 and GPU p99 .086.
The original tail is dominated by present (p99 16.594), while geometry CPU stays
small; presentation scheduling/phase is a supported inference, not proof of a
driver root cause. Both trials are retained, not replaced by the better result.
Multi100 callback max16.833 and renderer pan-fast max24.939 are likewise retained.
Most active cadence is in the established 60-Hz class; outliers remain, so no
strict sub-16.67-ms worst-case promise is made.

### Renderer regression against final Phase1B

Both runs use the same established scenario clocks/corpus/two-worker/no-prefetch
settings. This is a noisy desktop comparison, not a timing equality threshold.
CPU/GPU/callback columns are p99 ms, and the current callback maximum is retained.

| Scenario | CPU 1B → 1C | GPU 1B → 1C | Callback 1B → 1C | Current max | RSS MiB |
| --- | --- | --- | --- | ---: | ---: |
| pan-normal | 5.846 → 4.047 | 0.646 → 0.602 | 12.719 → 12.528 | 19.455 | 531.3 |
| pan-fast | 1.746 → 2.236 | 0.184 → 0.165 | 10.079 → 11.847 | 24.939 | 483.3 |
| zoom-traverse | 0.739 → 0.910 | 1.540 → 1.481 | 9.274 → 11.753 | 14.598 | 487.6 |
| pressure | 2.491 → 2.122 | 0.379 → 0.355 | 13.031 → 15.042 | 16.476 | 444.9 |
| pan | 0.367 → 0.430 | 0.053 → 0.066 | 7.952 → 13.496 | 17.231 | 339.7 |

All current runs submit about 59.1–59.3 frames/s and preserve cache/upload/job
bounds. Moderate p99 variation stays inside the previous observed CPU envelope;
no material sustained navigation regression is identified. Native presentation
tails do not disappear. Historical Mission1A pressure-tail risk remains open;
these small-corpus runs do not reset its high-LOD/contention history. Legacy
`pan` is the distant-jump diagnostic: coverage remains zero, as in Phase1B.
That is a known supply/cancellation limitation, **not** a successful visual pan.
Additional unprepared legacy board-tour has only 5.90% weighted content coverage,
CPU p99 1.470/callback p99 13.608/max17.549. Product prepared tour is measured
separately below; neither result is substituted for the other.

### Persistence/product regression

| Open | Native startup ms | Viewport80% ms | Preview reuse | Source bytes | RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| reopen-prepared | 562.6 | 654.7 | 4 | 0 | 314.8 |
| reopen-warm | 563.8 | 692.7 | 4 | 0 | 315.3 |
| reopen-tour | 595.7 | 1682.8 | 1000 | 0 | 375.5 |
| reopen-missing | 600.9 | 713.1 | 4 | 0 | 315.3 |
| reopen-repaired | 616.9 | 718.1 | 4 | 0 | 314.6 |
| reopen-embedded-after-deletion | 604.8 | 682.8 | 1 | 0 | 312.8 |

Initial 1k creation still pays 35.72 seconds application work, 1000 prepared
previews and 75.3 MiB RSS. Ordinary prepared usefulness .655–.718 seconds stays
in Phase1B's .6–.7-second class. Full prepared tour reaches its larger viewport
80% at 1.683 seconds, reuses all1000, weighted recognizable coverage95.34%,
CPU p99/max2.933/3.582, GPU p991.611, callback p99/max11.554/13.298 and 375.5MiB RSS.
Ordinary idle opens record only two/three redraws: their p99 is not representative.
Missing sources keep two explicitly requested missing links safe; embedded source
deleted externally still reopens. Three damaged overviews regenerate,997 reuse,
zero errors, total395.1ms including whole-snapshot Save. Product 10k metadata query
p99/max .888/.900ms, metadata load7.589ms; no spatial-index rewrite is justified.

## Resource contract

History200/32MiB; fixed preview vectors per active selected gesture; overlay128;
asset jobs/results16; two workers; CPU RGBA64MiB; GPU payload128MiB; per-frame
uploads≤8 and≤16MiB; GPU in-flight≤3; one save worker/request,128 repair file pins.
Every measured gesture/product frame respects upload/residency/in-flight bounds.
RSS319.6–320.6MiB for gestures includes driver/runtime allocations, not just
RGBA payloads. Bounded worker codec scratch, metadata snapshots on Save and
whole-file replacement costs remain explicit. No source-sized gesture allocation
or per-pointer task queue was introduced.

## Checks actually executed

Full local successful run in `benchmark-results/phase1c-checks/quality.log`:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo doc --workspace --no-deps --locked
python3 -m unittest discover -s tools -p 'test_*.py'
/tmp/tack-tools/bin/cargo-deny check
cargo test -p tack-render --test gpu_smoke --locked -- --ignored
cargo test -p tack-render --test product_gpu --locked -- --include-ignored
cargo build --release --locked -p tack-app
```

Ten initial interaction tests, four app persistence tests, three new core batch/
history tests, all existing workspace/storage tests, seven Python/tool tests,
GPU smoke and all three explicit product GPU checks pass. Cargo-deny advisories,
bans, licenses, sources pass with existing duplicate warnings. After the added
reverse-button-order case, fmt/Clippy and all11 app interaction tests pass again;
Python/tool checks are repeated after the native harness changes.
Native benchmark/persistence/regression commands and successful reports are linked
above. No ignored GPU test is counted as executed unless explicitly run.

Configured Linux/Windows quality and dependency CI will be checked on the pushed
code commit. Results are pending here; no remote check is yet claimed passed.

## Independent verification

The independent read-only verifier did not implement changes and inspected
architecture, lifecycle, geometry/crop, command atomicity, Save, input precedence,
resource boundaries, cleanup and frozen artifact hashes. Material findings fixed:
(1) middle held before Alt+handle could resize instead of pan; middle priority
now holds at begin/update in either order, with direct tests;
(2) evicted repaired preview paths could break Save; stable CRC-checked handles
are pinned with a bound and unlink regression test;
(3) double-click center outside camera bounds could fail; focus clamps consistently.
Additional native diagnosis fixed repeated title updates. No unused dependencies,
orphan runtime experiment, duplicate durable transform or persisted UI state
remains. The final report/product/native artifacts and all ten product report hashes were
reviewed independently: no material finding remains. The requested native low/high
zoom evidence and remote CI were identified as final completion items and are
recorded here after execution.

## Remaining risks and stop

Full desktop freeze cause unknown; hardware/WM/driver causes are not excluded.
No Windows native pointer/DPI or tablet/stylus evaluation; no artist feel/equivalence
claim. Snapping, arrangement, groups/frames, annotation and camera rotation remain
unimplemented. Higher-LOD product refinement, EXIF/ICC/color management,50k scale,
Mission0.7 fairness/high-LOD contention, autosave/relink/import/preferences polish
remain open. Missing linked previews are last-known images, not source recovery.
Distant-jump visual supply limitation is retained. No Phase1D work begins.

The core manipulation path is coherent enough for human review and authoring the
next spatial-organization brief; outstanding system/interaction limits above stay
visible and are not treated as solved.

A — PASS
