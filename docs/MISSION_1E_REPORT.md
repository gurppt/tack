# Mission 1E — Annotation and reference utilities

Historical phase evidence. Phase 1F removes prototype oval annotations and the
smooth note atlas; current behavior is described in the 1F design/report.

Date: 2026-10-04. Entry: Phase 1D **A — PASS**, main/origin at
`3071e2377dfee6974033c7942635bc7fa8516012`. Required charters, phase reports,
backlog/audit and interaction/spatial/format documents were read; no conflict.

## Delivered behavior

Six durable annotation kinds share existing IDs, Transform, document order,
selection, commands/history, persistence and the common GPU pass: UTF-8 notes,
rectangles, ellipses, lines, end-headed arrows and bounded vector scribbles.
Creation, note editing, mixed selection/marquee/move/resize/rotate/delete and
semantic color/fill/width/size/alignment/opacity actions are implemented.
Each completed operation has one atomic undo entry; Escape/focus loss/navigation
cancel transient state. Groups remain image-only, with an explicit refusal for
mixed annotation membership. Frames coexist without acquiring contained images.

Source Open/Reveal/Copy use the actual linked descriptor, canonical regular
PNG/JPEG target and fixed native helpers on an on-demand worker. Embedded,
missing/foreign, executable and unsupported targets return safe errors. Relative
board symlinks resolve from the same lexical parent as ProductAssets. There is no
shell string, annotation-text execution, persistent worker or source read in
selection/rendering. Filesystem operations retain normal OS blocking behavior;
the ten-second timeout applies to the spawned helper. Linux Copy needs xclip or
wl-copy; absent helpers produce actionable errors. Tests used a temporary,
locally extracted xclip for actual clipboard verification, without host install.

[User shortcuts](../README.md), [object design](design/annotation_objects.md),
[font decision](design/text_rendering_decision.md) and
[schema 3](design/tack_file_format_v3.md) describe the final implementation.
No arbitrary vector graph, rich-text/drawing framework, per-object widget tree,
font service, new runtime package/version, brush engine or Phase 1F feature.

## Frozen evidence and method

Shipping binary SHA-256:
`b929230b4f1d7373cfb806e1144606f5f7959d654bb80793ab0cf80fdb63cdd9`.
Primary source ZIP SHA-256:
`43f0c19cea9e0dbdf5b5de14f03ae1dcd17c820a917d3eed1954bd3669101edb`.
Runtime Rust/WGSL/manifests/lock/assets were compared byte-for-byte against that
snapshot after measurement: unchanged. Later changes are harness assertions,
test quantization tolerance and documentation. Each tracked receipt records its
actual harness/board/report hashes; legacy/product harnesses make their own ZIPs.
Local full artifacts remain under `benchmark-results/phase1e-*-measured` and
`phase1e-native-*`; compact receipts are tracked in `benchmarks/phase1e-*.json`.
No user `gfx/` files or briefs are included in this work.

Linux 7.1.5, Rust 1.95.0, RTX 2060 6 GiB, NVIDIA 580.173.02, Vulkan, X11
`:0.0` for native/render benchmarks. Runs are serialized, no concurrent builds
or owned GPU tests during quantitative measurement. Shape/text/stroke and
interaction/spatial scenarios are 12 seconds. CPU statistics exclude the first
second for steady-state annotation work; GPU timestamps measure the canvas pass,
not uploads/acquire/presentation. Callback includes those native costs. Percentile
is sorted floor(p×(n−1)); maxima are retained. RSS is sampled /proc VmHWM.
Allocation counts, exact event-loop wakeups and monitor latency are not measured.
The image “80% useful” clock remains image coverage, never annotation substitutes.

Corpora are generated stress fixtures, not artist boards. Mixed has 1000 prepared
resident images, 100 annotations and one frame; only four image overviews are
requested in the measured viewport. Dense shapes are rect/line/arrow mixes.
5000 text boxes are metadata/query/layout/save/reopen evidence, with 1086 visible
boxes after culling; there is no all-5000-note simultaneous native GPU claim.

## Reproduction

Use a new output directory for each run. Build the pinned release binary first;
run GPU scenarios serially on an idle desktop. Example with the locally retained
1D prepared board (substitute your own prepared fixture where unavailable):

```bash
cargo build --release --locked -p tack-app
python3 tools/run_annotations.py --prepared-board benchmark-results/phase1d-product-final/linked.tack --output benchmark-results/phase1e-reproduce
python3 tools/run_native_annotation_checks.py --binary benchmark-results/phase1e-reproduce/tack-app --output benchmark-results/phase1e-native-reproduce
```

Native authoring injects physical input: avoid interacting with its owned window
while it runs. For isolated idle, start an owned Xvfb display, then use
`DISPLAY=:91 python3 tools/run_idle.py --no-wm --edit-note --binary <frozen-binary>
--board <owned-annotation-board> --output <new-output-directory>`. Repeat without
`--edit-note`, with image-only/annotation boards and matched 1D binary. The tool
asserts the loaded title, grid and note editor before observing; close only the
owned display afterward. No Xvfb dependency is added to Tack. Receipt commands,
source archives and input hashes pin the remaining measured regressions.

## Annotation performance

Milliseconds, RSS MiB. CPU includes packing/overlays; callback tails remain.

| Scenario | CPU p99 / max | GPU p99 / max | Callback p99 / max | Peak RSS |
| --- | --- | --- | --- | --- |
| shapes-1000 | 0.840 / 0.905 | 0.050 / 0.056 | 16.426 / 31.719 | 321.109 |
| shapes-5000 | 3.050 / 3.743 | 0.098 / 0.110 | 17.450 / 22.004 | 325.168 |
| shapes-10000 | 6.865 / 8.445 | 0.186 / 0.702 | 19.015 / 31.399 | 331.469 |
| text-100 | 0.462 / 0.507 | 0.063 / 0.091 | 15.913 / 32.482 | 321.246 |
| text-1000 | 1.617 / 1.741 | 0.109 / 0.141 | 13.806 / 18.267 | 325.477 |
| scribble-1 | 0.224 / 0.264 | 0.036 / 0.051 | 14.905 / 69.042 | 320.148 |
| scribble-2 | 0.470 / 0.521 | 0.060 / 0.071 | 16.703 / 31.799 | 320.762 |
| scribble-100 | 1.272 / 1.595 | 0.105 / 0.107 | 12.936 / 16.767 | 322.461 |
| mixed | 0.718 / 0.994 | 0.073 / 0.085 | 16.572 / 31.535 | 323.363 |

CPU-only metadata trials: 64 queries/builds each; save/open single measurements,
including exact round-trip invariant. Counts are document objects of that kind.

| Kind/count | Query p99 | Build p99 | Save / reopen | File bytes | Primitives / glyphs |
| --- | --- | --- | --- | --- | --- |
| shapes/1000 | 0.058 | 0.191 | 8.665 / 0.569 | 128412 | 1333 / 0 |
| shapes/5000 | 0.320 | 1.153 | 8.462 / 2.915 | 641756 | 6666 / 0 |
| shapes/10000 | 0.811 | 2.298 | 12.841 / 5.579 | 1283412 | 13333 / 0 |
| text/100 | 0.004 | 0.082 | 7.262 / 0.057 | 14090 | 1390 / 1390 |
| text/1000 | 0.049 | 0.881 | 3.883 / 0.440 | 140990 | 14890 / 14890 |
| text/5000 | 0.482 | 2.071 | 11.359 / 2.301 | 708990 | 16298 / 16298 |
| scribble/1 | 0.000 | 0.000 | 7.417 / 0.018 | 275 | 3 / 0 |
| scribble/100 | 0.004 | 0.326 | 3.500 / 0.104 | 132800 | 7500 / 0 |
| scribble/2 | 0.000 | 0.084 | 3.273 / 0.029 | 32322 | 1998 / 0 |

All scenario source-byte counts and omitted-object counts are zero. RDP at
normalized tolerance .002 takes 1000 sine points to 76: p99 .303 ms, max .303 ms.
Raw-stroke case draws two unsimplified 1000-point strokes (1998 segments);
repeated case draws 100 simplified 76-point strokes (7500 segments).

Additional ellipse probes exercise the expensive closest-edge shader, beyond
the rect/line/arrow corpus, including full-viewport stroke/fill with opacity .5:

| Ellipse probe | CPU p99 | GPU p99 / max | Callback p99 / max | RSS MiB |
| --- | --- | --- | --- | --- |
| ellipse-1000 | 0.861 | 0.246 / 0.260 | 17.000 / 30.032 | 320.461 |
| ellipse-10000 | 5.236 | 0.386 / 0.464 | 18.419 / 31.648 | 331.008 |
| ellipse-full | 0.224 | 0.376 / 0.385 | 17.140 / 17.775 | 320.062 |

## Image and spatial regressions

Prepared image-only and mixed boards: every scenario passed bounds/invariants,
source bytes zero, ten committed+undone operations (or ten cancelled previews).
Input includes begin/commit/undo. RSS is approximately 321–323 MiB.

| Operation | Image input / CPU p99 | Mixed input / CPU p99 | Image / mixed callback p99 |
| --- | --- | --- | --- |
| drag | 0.202 / 0.476 | 0.256 / 0.911 | 17.276 / 16.198 |
| resize | 0.010 / 0.501 | 0.009 / 0.772 | 17.249 / 16.646 |
| rotate | 0.168 / 0.590 | 0.236 / 0.755 | 17.171 / 16.888 |
| crop | 0.009 / 0.704 | 0.008 / 0.667 | 16.771 / 16.471 |
| multi10 | 0.237 / 0.558 | 0.239 / 0.928 | 16.930 / 16.960 |
| multi100 | 0.278 / 0.571 | 0.313 / 0.969 | 16.382 / 17.045 |
| cancel | 0.226 / 0.507 | 0.239 / 0.755 | 17.018 / 16.795 |

Spatial native regression, zero source bytes throughout; CPU-only layouts and
flat groups (10/100/1000 layouts, groups 10/100) also retain exact invariants.

| Scenario | Input p99 | Snap query p99 | CPU p99 | GPU p99 | Callback max |
| --- | --- | --- | --- | --- | --- |
| snap-1000 | 0.156 | 0.091 | 0.438 | 0.043 | 31.938 |
| snap-5000 | 0.780 | 0.470 | 1.109 | 0.041 | 24.542 |
| snap-10000 | 1.643 | 0.935 | 2.229 | 0.044 | 31.620 |
| grid-hidden | 0.001 | — | 0.393 | 0.041 | 33.437 |
| grid-visible | 0.000 | — | 0.423 | 0.070 | 33.262 |
| frames-10 | 0.000 | — | 0.212 | 0.025 | 31.993 |
| frames-100 | 0.000 | — | 0.259 | 0.026 | 31.747 |
| frames-1000 | 0.000 | — | 0.509 | 0.033 | 31.815 |

Prepared product persistence suite passed create 1000 linked images (36.473 s),
prepared/warm/tour/missing/repaired reopen and embedded reopen after deletion of
its owned external file. Prepared/warm image-80%-useful were .687/.719 s; ordinary
views reuse four overviews, source bytes zero. Tour reuses all 1000. Repair-three
is .411 s, regenerates exactly three and reuses 997. No source errors; missing
originals retain saved previews. Timed reopen wall durations include the explicit
benchmark observation window, not pure loading latency. See product receipt.

Legacy navigation renderer (fresh display caches, generated 1000-object/32-source
6000×4500 JPEG corpus, existing native decoder) also passes bounded queue/inflight/
residency/upload gates. Coverage and presentation limits remain visible:

| Scenario | CPU p99 / max | GPU p99 | Callback p99 / max | RSS MiB | Content / requested coverage |
| --- | --- | --- | --- | --- | --- |
| pan-normal | 3.921 / 8.087 | 0.645 | 30.381 / 32.427 | 549.824 | 0.811 / 0.227 |
| pan-fast | 2.240 / 2.740 | 0.229 | 27.916 / 30.839 | 466.430 | 0.773 / 0.000 |
| zoom | 1.566 / 6.327 | 1.334 | 22.898 / 30.239 | 491.340 | 0.159 / 0.151 |
| pressure | 2.320 / 6.147 | 0.376 | 19.853 / 30.370 | 446.555 | 0.952 / 0.704 |
| pan | 0.400 / 0.679 | 0.063 | 25.537 / 30.889 | 341.125 | 0.000 / 0.000 |

`pan` is the distant-jump stress: zero content/requested coverage remains a supply
failure, not a useful-navigation success. Pan-fast requested LOD remains zero;
zoom content coverage is .159. Native callback tails up to 69 ms in the short
stroke run and 30–33 ms in many other runs remain; GPU pass time alone does not
prove monitor smoothness. Existing high-LOD/fairness work remains deferred.

## Anti-bloat, font and idle

No new Rust package/version, runtime text engine or idle application thread/timer.
The renderer adds a direct reference to the already-used image crate. The renamed
OFL Tack Note Mono subset is 416,042 PNG bytes + 2560 slot bytes, one copy each;
1197 glyphs, existing 1D Unifont bitmap fallback for other scalars. No runtime
font I/O or cache growth. Pinned source and OFL notices are retained; the build
TTF is not embedded. Generator reproduced exact atlas/slot hashes.

Matched `strip --strip-debug`: 1D 19,919,264 B; 1E 20,591,112 B; +671,848 B
(.641 MiB, 3.37%). Fully stripped: +633,352 B. Object remains 160 B, Command 176;
Document +16 B and ImageInput +80 B. Unused annotation GPU/atlas allocations are
zero; CPU scene/editor/pipeline are lazy. CPU primitive cap 32768×176 = 5.5 MiB,
instance staging and GPU buffer each cap 4 MiB; separately bounded order/ranges.
Image upload telemetry excludes annotation instances: at most 4 MiB additional
instances/frame plus 1.781 MiB atlas once, not a global 16 MiB upload claim.

100 notes: layout p99 .151 ms, GPU instances .25 MiB; 1000 notes: .719 ms, 2 MiB.
Both retain the same 1,867,776 B R8 GPU atlas. First CPU frame 18.293/14.256 ms
includes atlas decode/upload/pipeline setup; decoding alone was not isolated.
No multi-megabyte second complete Unicode font, leftover RLE payload or glyph
service is shipped. Fixed glyph cache is constant after first use.

Final paired idle observations use an owned isolated Xvfb `:91`, no WM, same
RTX2060/Vulkan adapter. The isolated display prevents physical desktop input;
GL/llvmpipe initialization threads also appear in both binaries, so these are
paired process/RSS observations, not desktop presentation benchmarks. Each row
waits for loaded title, asserts grid/editor state, settles, then observes 10 s
with no application measurement timer. Startup/shutdown redraws are excluded.

| Binary/state | Grid | CPU % one core | RSS MiB | Idle redraw / submit | New threads |
| --- | --- | --- | --- | --- | --- |
| baseline | hidden | 0.399 | 333.996 | 0 / 0 | 0 |
| baseline | visible | 0.399 | 334.270 | 0 / 0 | 0 |
| unused | hidden | 0.399 | 334.398 | 0 / 0 | 0 |
| unused | visible | 0.499 | 334.617 | 0 / 0 | 0 |
| annotations | hidden | 0.499 | 337.172 | 0 / 0 | 0 |
| annotations | visible | 0.399 | 336.969 | 0 / 0 | 0 |
| edit | hidden | 0.399 | 337.293 | 0 / 0 | 0 |
| edit | visible | 0.499 | 337.145 | 0 / 0 | 0 |

All eight rows: zero IO deltas, source bytes zero, no new/departed threads,
41 total threads in both baselines/candidates; overview workers remain asleep.
Application main-thread idle ticks/context switches are zero. Driver/support
threads account for the observed 4–5 total CPU ticks, not a new caret wakeup.
Unused matched RSS delta is +.402/.348 MiB (single paired trials, allocator/driver
noise not excluded); used annotations about +2.7–3.3 MiB. Editing does not grow
RSS during observation and introduces no redraw/timer churn. Event-loop waits
and /proc switches are not exact wakeup counts. Socket counts are native X11/
driver IPC descriptors, not a network traffic meter; Tack has no network client.

Earlier live-desktop idle trials were invalidated by user interaction (explicitly
confirmed by the user), producing redraws and unsaved in-memory edits. Board
hashes were unchanged. They are retained, excluded and replaced; no app defect
or machine-freeze causal attribution follows. Preliminary isolated trials sent
keys before async initialization; final harness asserts ready/grid/editor state.
No new freeze was observed; the earlier 1C incident remains root-cause unresolved.

## Native, persistence and safety checks

Linux X11 automated physical mouse/key input and inspected owned captures:
31 annotation checks at DPI1, 26 at DPI2. Creation/edit/cancel/multiline, all six
kinds, bounded scribble, styles, move/resize/rotate exact undo, frame coexistence,
mixed marquee/group refusal, zoom, delete undo and exact schema3 save/reopen.
Open/Reveal/Copy actually invoked native helpers on owned PNG/parent fixtures;
clipboard matches the canonical descriptor, missing source gives a safe error.
DPI2 omits source helper repetition. Native image regression has 17 assertions,
spatial 25, low/high zoom eight, all observed true. This is automated native
exercise plus visual inspection, not a human artist/tablet/stylus feel claim.

Core tests cover finite/bounded geometry/style/text, deterministic simplification,
atomic mixed styling/text undo and oversized inverse rollback. App tests cover
creation/edit/cancel/pan precedence, bounded capture, hollow/arrow/thick-eccentric
ellipse hits, mixed transforms and opacity preview, saturation keeping transients,
source safety (embedded/missing/foreign/magic/executable/symlink/hostile filename),
and relative board-symlink resolution. Text UTF-8 round-trips exactly; native
keyboard checks are plain text, IME/scalar paths are implementation/unit evidence.
Storage tests round-trip six kinds with frames and old schemas; CRC-repaired
future schema4, kind99, oversized length, NaN, invalid style/alignment/text/points
are refused without modifying the input file. Existing global metadata/object/
history budgets remain. Schema3 is only emitted when annotations are present.

Explicit GPU readback passed six product tests plus smoke: common order, lazy
bounds, note atlas, antialias coverage, combined global stroke/fill alpha,
adjacent capsule/arrow joints and eccentric ellipse. The joined-alpha assertion
allows one 8-bit channel quantization unit, not a widened geometric threshold.

Local final quality: fmt, all-target/all-feature check, Clippy -D warnings,
workspace tests, docs, seven Python tests and cargo-deny. An initial final command
reached deny with cargo-deny missing from PATH; the complete command was rerun
with the already-installed /tmp tool path. Logs and hashed receipts retain both.
Configured [Quality CI](https://github.com/gurppt/tack/actions/runs/37194870799)
completed successfully at code commit
`f2ccccc793b825b7c07f4cf968330cf9ace00711`: Linux Rust/Python/software-Vulkan GPU,
Windows Rust, and dependency advisory/license/bans/source jobs all succeeded.
[CI receipt](../benchmarks/phase1e-ci.json) pins the exact SHA and workflow.
The final followup commit contains only documentation/receipts; runtime is
unchanged. No native Windows claim follows CI.

## Independent verification

The existing independent `phase1e_verifier` did not implement/build/measure the
mission. Read-only reviews found six code defects, fixed and re-reviewed: oversized
inverse-history rollback, annotation opacity preview, transient priority at
primitive saturation, lexical relative-source resolution for symlinked boards,
ellipse true edge distance and joined/global opacity coverage. An annotation
query benchmark initially used the image-only query path; its evidence was
corrected to query actual annotation bounds and rerun.
Affected core/app/parser/GPU/native and quantitative tests were run on the final
shipping implementation. Preview priority at a full primitive budget is tested.
Final evidence/report review checked 122 shipping files against the snapshot
and verified binary/log hashes and paired costs. Its disposition is PASS local;
the CI condition is now satisfied, with no material finding remaining. The
verification receipt records its scope and retained limits.

## Remaining limits and stop gate

Plain scalar/monospace notes only: no bidi/complex shaping, caret navigation,
rich text or paste editor; finite atlas resolution at extreme zoom, box clipping
and Unicode bitmap fallback remain visible. Ellipse marquee is a 64-segment
approximation. Nonadjacent scribble self-crossings can darken through overdraw;
adjacent joints and shape fill/stroke opacity are corrected. Primitive overflow
omits whole objects with a visible warning, keeps durable authority unchanged.

Windows native DPI/source helper behavior, artist/stylus feel, allocation counts,
exact idle wakeups, 50k scaling, distant-jump supply/high-LOD fairness and native
presentation tails remain open. Existing autosave/relink/import/preferences,
EXIF/ICC/color management and final maximum-performance work stay deferred.

This local standalone slice is ready for human review and a future **Phase 1F —
Local production hardening** brief. Stop at 1E; 1F is not started.

A — PASS
