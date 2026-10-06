# Phase 1I — native keyboard correctness and board arrangement

2026-10-06. **B — automated implementation/native gates complete; owner review
pending.** The reported keyboard Undo failure was reproduced on French AZERTY
and fixed. One-shot creation returns Pointer; Arrange in Grid and Snap Selection
to Grid are atomic, undoable ordinary transforms. No Phase 2 was started.

Implementation commit: `fd687d31789d9528d090eabe1aca0eee9ea1f066`.
Human-test executable: `./bin/tack`; current commit/profile/time/hash/dirty state
are in `./bin/BUILD.txt`. Verified release SHA256:
`56ffdaf83a6f4678e20127c887cf2b64788b7ce03cb87bbc2fe34a5facc4ab90`.

## Reproduction and correction

The previous released executable (`4d21e4ea…`) ran in a real native X11 window
on Linux 7.1.5, NVIDIA RTX 2060/Vulkan, using actual XKB US/FR layout changes.
Generated image import → pointer move → keyboard Undo → save comparison:
US restored the move; FR did not; menu Undo restored it in both layouts.
A diagnostic build of the unchanged old binding architecture then traced FR:
physical `Code(KeyW)`, logical `Character("z")`, CONTROL modifiers, old normalized
physical KeyW. No Undo action reached dispatch. The menu used the common owner
and history successfully. This is a native identity mismatch, not missing Undo.

Defaults now use the active layout's logical base key, for both commands and
tools. Ctrl+Z means the actual Z on AZERTY, not the US-Z position. The physical
key remains captured for hold/release correctness. Repeats do not repeat
one-shot commands; focus loss drains bounded input state. Modifier/layout changes
retain the captured physical release identity.
Menus, keymap labels and canvas share action resolution and the same owner/history.

Preferences version 2 supports explicit physical imports with honest
`pos:Code(KeyZ)` labels. Version 1 advertised letter/named bindings migrate to
logical bindings, including custom assignments. Unsupported legacy positions
are refused while preserving the original profile; custom files require repair
or explicit physical v2 export. Overlapping physical/logical modifier domains
are conservatively refused to avoid layout-dependent ambiguity. This deliberately
may reject combinations that would not collide on one particular layout.
See [keyboard policy](design/keyboard_shortcuts.md) for complete migration limits.

Ctrl+Z remains consumed while a note draft is active: there is no local text
Undo buffer. It cannot erase earlier document edits. Ctrl+Enter commits one note
operation, then document Undo works normally; Escape discards the draft.

## Tools and organization

Note/Text, Rectangle, Line and Arrow create once, then return Pointer. Frame
creation also returns Pointer. Note draft entry already resets the Pointer base
while the draft owns input. Freehand/Scribble stays active for repeated strokes;
Escape/V returns Pointer. Cancellation does not commit partial objects or add
history. Existing temporary overrides are cleared on modal reset/one-shot
completion; nested experimental Pan/Text commit/cancel/focus paths are tested.
Pan/RotateView tool modes remain experimental, without a second tool stack.

Multi-selection → right-click → Arrange has Grid and Snap first, then the
retained six align, two distribute and two pack operations. The flat 12-row child
menu fits native 800×600. No default shortcut was invented for the new actions;
both appear in the remappable semantic catalog (now 91 actions).

Grid collects independent units (whole image groups, rotated world AABBs), sorts
Y/X/stable ID, evaluates five bounded row/column candidates and places rows with
16 world-unit gaps. It preserves size/aspect, rotation, flip/crop and group
internals. For at least three units, it uses at least two rows and columns.
No persistent constraints: one transform batch, one Undo, ordinary save/reopen.
Sorting/indexed lookup plus bounded linear passes is O(n log n), temporary O(n).

Snap independently rounds every unit's AABB top-left on both axes to the base
64 world-unit board lattice, including negative coordinates. This is independent
of DPI/zoom and not every adaptive visible dot (which may be 32 units at zoom 1).
Snap can reduce gaps or introduce overlap; it is quantization, not another
packing solver. Single-unit snapping is supported through a remapped action.
Tidy and equal-size normalization are deferred. See
[arrangement and restoration policy](design/board_arrangement.md).

## Native and deterministic verification

Current runtime: **170 native assertions**, all passing:

| Native client | US | French AZERTY |
| --- | ---: | ---: |
| 800×600 | 42 | 43 |
| 1024×768 | 42 | 43 |

Actual keyboard input verifies Undo/Redo/Ctrl+Y/Save/Select All, V/T/R/L/A/P,
grid/snapping toggles, Delete, F2, F10, repeat suppression, modifier release
order, focus loss, popup open/closed and active text drafts. Native keymap capture
refuses Ctrl+S conflicting with Save, preserves Undo, then remaps Undo to Ctrl+W
(FR physical Z/logical W) and proves the previous shortcut no longer fires.

Persisted state proves keyboard and menu Undo restore the same move. Six mixed
images plus a note exercise Grid/Snap, preserved dimensions, both-axis movement,
one exact Undo/Redo and actual native reopen followed by save authority equality.
Native traces retain actual physical/logical/modifier/resolution evidence.
[800 Arrange](../benchmarks/phase1i-ui/input-arrange-menu-0-800x600-fr.png),
[800 conflict refusal](../benchmarks/phase1i-ui/input-keymap-conflict-800x600-fr.png),
[800 remap](../benchmarks/phase1i-ui/input-keymap-remapped-800x600-fr.png).
These are automation/visual-fit observations, not owner comfort acceptance.

Nine new Rust owner/input/layout tests additionally cover non-US logical Z
versus physical Z negative cases; release aliases; nested temporary tools;
unsupported v1 preservation; 2/3/4/5/10/100 mixed bounds; rotation/groups;
determinism under reversed selection; no ordinary Grid overlap; exact atomic
Undo/Redo; save/reopen; negative/base-lattice/no-op Snap semantics.

Local `tools/check.sh`: formatting, all-target/all-feature locked check, Clippy
warnings denied, **153 Rust workspace/doc passes**, docs, **9 Python passes**,
fresh dependency/license/advisory checks all pass. Explicit NVIDIA Vulkan:
GPU smoke 1 pass, product GPU 7 passes (two also occur in workspace tests).
`git diff --check` passes; no Cargo manifest/lock/dependency change.

Remote exact-implementation Quality receipt: [Quality 37429790856](https://github.com/gurppt/tack/actions/runs/37429790856),
**success** for `fd687d31789d9528d090eabe1aca0eee9ea1f066`: Linux, Windows
and dependencies all successful.
Windows desktop input/GPU testing remains separate from the Windows CI gate.

## Arrangement cost and performance comparison

Ryzen 7 2700X, release build. Seven serial, uninstrumented CPU samples per count,
separate glibc memusage observations of the identical fixture baseline and plan.
Fixtures alternate 120×80 / 60×140 lightweight image units. Medians:

| Units | Plan ms | Atomic apply ms | First visibility rebuild/query ms | History bytes | Extra plan heap peak bytes |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | 0.0193 | 0.0039 | 0.0006 | 1,936 | 1,465 |
| 100 | 0.0947 | 0.0387 | 0.0066 | 17,776 | 31,065 |
| 1,000 | 1.2551 | 0.4403 | 0.0923 | 176,176 | 306,361 |
| 10,000 | 10.6348 | 2.2502 | 0.5074 | 1,760,176 | 2,785,081 |

Cumulative extra requested plan bytes: 3,384 / 40,984 / 388,280 / 4,373,432.
Allocator deltas include small report-format differences and fixture peak
interaction; they are not exact Rust live-object accounting. Instrumented times
are excluded from CPU medians. The visibility column is CPU query cost, not
whole GPU redraw or user-observed latency. Whole native frame costs are retained
in the native reports; that functional matrix uses opt-in tracing and is excluded
from performance acceptance. A separate trace-disabled native 10,000-rectangle
run verifies changed state and exact Undo/Redo. Its final saved-arrangement frame
at 800×600 measures query 0.798 ms, annotation build 2.859 ms, CPU 5.110 ms,
callback 24.478 ms including presentation 19.338 ms; last five GPU passes
0.0184–0.0200 ms. Only four annotation primitives are on this viewport after
arrangement: this is resulting redraw at the current camera, not full-board
10,000-visible-object GPU cost or isolated command latency.
No arrangement engine/memory remains after command planning beyond ordinary
transformed document and retained history state; standard visibility memo remains.

Regression comparison: two alternated matched prior/current pairs × three cases,
12 serial 9-second native runs, actual 800×600, same generated linked-source boards,
warm OS cache, input tracing disabled. Medians across two runs per binary:

| Board | RSS MiB prior → current | First useful ms prior → current | Callback p99 ms prior → current |
| --- | --- | --- | --- |
| 5k | 301.014 → 301.137 | 407.077 → 412.323 | 29.796 → 29.411 |
| 50k | 327.393 → 326.291 | 423.707 → 416.966 | 29.979 → 28.161 |
| Sparse | 277.672 → 277.555 | 380.980 → 370.690 | 28.844 → 28.832 |

No material regression is visible in these matched samples. Two repetitions are
not a universal performance guarantee; presentation tails remain platform sensitive.
No compiler, second Tack GPU owner or CPU benchmark ran alongside accepted timings.

Settled idle: matched empty board, prior/current/menu-open/menu-closed, 5 seconds
each after settlement. All show zero main-thread CPU ticks, zero decoder ticks,
zero idle redraw/submission and zero process I/O delta. RSS prior 234.645 MiB;
current 234.262, menu 234.418, closed 234.703 MiB, flat within each interval.
Whole process still consumes 2–3 ticks/interval and driver-related wakeups
(~499 switches on one `tack`-named thread plus NVIDIA threads). Do not claim
zero OS wakeups for the whole GPU process. No new timer/worker/network/I/O path.

Fully stripped release: **17,804,776 → 17,908,392 bytes**, +103,616 (+0.582%).
Unstripped release: 114,874,416 → 115,282,304 bytes. **276 locked packages,
zero new dependencies.** Caches, workers, document limits and renderer budgets
are unchanged; diagnostic tracing is explicitly opt-in.

## Evidence, independent review and next step

[Machine-readable receipt](../benchmarks/phase1i.json) records source-file hashes,
native assertions/captures, layout samples/allocator, prior/current frame
statistics, idle threads/I/O, binary budget and exact CI. Raw generated evidence
is locally retained under ignored `benchmark-results/phase1i/`. Accepted subsets
are explicitly listed; exploratory failures/earlier binaries are not acceptance
measurements. Source snapshot SHA256:
`2803c4177c91562bb663839b4ff525cf46c31860f6e492a91a26d03f2790acef`
(191 tracked source/config/tools/assets/CI files; docs/receipts/user gfx excluded).

[Independent review](VERIFICATION_1I.md) audits non-US falsification, owner/history,
tools, complexity, 800×600, idle/performance and budget. Its two concrete findings
were corrected: aliased physical release could end another held key; and the
initial native reopen assertion compared disk state rather than the reopened
owner. Both now have strengthened tests/evidence.

The owner should run [the short checklist](HUMAN_TEST_1I.md) on the current binary
before another major branch. No owner hands-on review of the new arrangement is
fabricated. No local text Undo, real Windows desktop gate still pending, dense
overlap GPU cost still real, final global max-performance pass still pending,
128 MB/Pentium III still aspirational, retained container/history disk cost and
platform presentation tails still known. No collaboration, cloud, media or final
global optimization was started. Stop here for human review as required by 1I.
