# Phase 2A6 — interaction polish

Implementation `80f7014fefc6420491f5ee65891280ee384eb84e` and automated validation
PASS; physical artist acceptance
remains pending. Phase 2A5 authority and document/storage formats are retained.
Source base `0e92f45` includes 2A5 implementation `688afb9` plus icon preservation.
The prerequisite gate, repository status, binary/dependency baseline and local
idle were recorded in [baseline.json](../benchmarks/phase2a6/baseline.json).

## Implemented behavior

- Toolbar action cells are exactly 16×16 logical pixels with no outer cartouche,
  padding or edge margin. The existing draggable grip has its own 16 px cell.
  A persisted Separator layout item draws 1×12 (12×1 vertically), has no Action,
  shortcut or hover, and adjacent duplicates normalize away.
- Tools → Edit Toolbar replaces the View entry. Dedicated Keymap remains;
  duplicate Edit → Keymap is removed. The compact two-column editor uses the
  existing atlas, real mapped icons and text-only unmapped entries. Add selects
  the new Order row, minimally reveals it and acknowledges in green for 1.2 s.
  Remove selects the nearest survivor; Ctrl+Up/Down reorder. Arrows, Tab, Enter,
  Escape and mouse share selection/focus. Release cleanup works through modals.
- Context menus keep the toolbar present. The existing overlay is split around
  atlas drawing so covered pixels naturally occlude it without suppressing the
  uncovered bar. Nested Escape goes back, then closes the root.
- Local transform bounding outlines are removed, including multi-selection.
  Handles and supported rotation stem/handle remain. Frame rotation restrictions
  are unchanged. Foreign leases retain their separate red outer indication.
- Frame titles default to 1×. Unmarked old profiles at the former 2× default
  migrate once to 1×; old explicit 1×/3× and subsequent explicit 2× survive.
  Double-clicking the title dispatches the same RenameFrame action as F2.
  Enter commits, Escape cancels, click-away commits. Border/title hover is
  immediate; the interior is not a hover surface. Color cycle updates border
  and label background with deterministic readable light/dark text.
- Frame/Note edits and compatible local fields/search use the hard pixel block
  caret. Editing reserves its layout space so blinking cannot move wrapped
  titles or centered/right-aligned Note text. No UI glyph enters saved text.
  Blink deadlines stop on editing exit, focus loss, occlusion or replacement.
  Copy invite and toolbar Add reuse one expiring acknowledgement primitive.
- Executable-relative artist icons take precedence over repository defaults.
  Builds seed missing files only. Four runtime icons changed during this session,
  with different timestamps including changes after the build; they were kept,
  never restored to baseline. Source/runtime user artwork is not disposable.

## Automated evidence

The pinned normal quality gate passes formatting, check, Clippy with warnings
as errors, 362 Rust tests, documentation, 28 Python tests and cargo-deny.
Explicit NVIDIA RTX 2060 Vulkan tests pass: 1 smoke, 1 selection and 10 product
checks, including popup occlusion over the single atlas without reallocating it.
Targeted tests cover separators/profile migration, focus/capture, feedback
expiry, Frame hover/rename and stable Frame/Note geometry across caret blinking.

[Native UI receipt](../benchmarks/phase2a6/ui.json) records 38 passing checks across
800×600 at 1×/2× and 1024×768 on all five placements. The native editor, Tools
entry, Add/reorder/Remove, Frame F2/double-click/Enter/Escape, color captures and
settled idle are exercised. [Pixel measurements](../benchmarks/phase2a6/separator-pixels.json)
confirm 30 exact separator lines. Selected [captures](../benchmarks/phase2a6/ui/)
show the bar, context overlay, 2× editor, rename and light palette entry.

[Three-client receipt](../benchmarks/phase2a6/shared-three.json) repeats all 16
2A5 native checks on the current binary: independent gestures, same-object
lease denial/red indication, unrelated menu survival, exact-target deletion,
lease reclamation, Duplicate/Undo, local view slots and settled idle. Three
owned displays on one host are not a physical two-computer acceptance test.

[Windows receipt](../benchmarks/phase2a6/windows.json) proves the cross-built
Windows CLI creates/reopens a JPEG board under Wine, and the packaged scalar,
SSE2 and automatic-SIMD decoder paths execute with identical output. Physical
Windows UI acceptance remains pending. The exact implementation passes [Quality CI 38043789172](https://github.com/gurppt/tack/actions/runs/38043789172):
Linux, Windows and dependencies all succeed. The [CI receipt](../benchmarks/phase2a6/ci.json)
records job/step conclusions. Linux also executes software Vulkan and LOD
convergence/churn; Windows runs the pinned-toolchain workspace gate.

## Measured cost

Two serial paired runs before the reboot, on the same owned NVIDIA display/GPU
and small local board,
warm/unspecified OS page cache; [raw comparison](../benchmarks/phase2a6/local-paired.json)
and [frame telemetry](../benchmarks/phase2a6/local-frame-telemetry.json) are retained.
These runs measured validation candidate `4a28e5ea`; the final binary
`13129839` was rebuilt after adding the last regression tests. This narrow
sample does not establish startup or large-board performance gains.

| Metric | 2A5 baseline | 2A6 |
| --- | ---: | ---: |
| External Cargo dependencies | 278 | 278 |
| Stripped Linux executable | 19,947,776 B | 19,968,512 B (+20,736; 0.104%) |
| Local idle threads | 41 | 41 |
| Mean idle RSS | 334.930 MiB | 334.584 MiB |
| Accounted scene GPU peak | 104,960 B | 104,960 B |
| Recurring redraw / app I/O / IP sockets | 0 / 0 / 0 | 0 / 0 / 0 |
| Mean native startup | 455.91 ms | 479.71 ms (+23.80 ms) |

Startup was slightly slower in these two runs; callback medians overlap
(21–23 ms; only three startup frames per run). No stronger performance claim
is made. There is no new worker, network behavior or permanent polling timer.
The UI workflow also settles to zero caret deadline, redraw and I/O after edit.
[Build receipt](../benchmarks/phase2a6/builds.json) records hashes and Windows metadata.

## Review, resources and acceptance

[Independent source review](reviews/phase2a6_interaction.md) passes after fixing
capture retargeting, swallowed releases, blink-induced reflow and form navigation.
Early failed harness attempts remain separate: comparing pixels physically
covered by the menu, and expecting a stricter gray than the Frame palette uses.
Neither attempt is counted as passing evidence.

Shared target reused; one baseline executable retained. Generated phase evidence
is under 32 MiB; committed compact evidence is under 300 KiB. An owned Wine
prefix temporarily used the documented allowance of up to 2 GiB, then was stopped
and removed by the harness. The 10 GiB free-space reserve was retained. No user
board/profile/artwork or broad cache directory was cleaned.

The [human checklist](HUMAN_TEST_2A6_INTERACTION_POLISH.md) covers artist feel,
physical Windows and physical two-computer acceptance, which are still pending.
No Studio Server, WAN/auth, media, importer, new text tool or other 2B work was
started. Stop after 2A6.

## System incident and recovery

The owner reported a whole-system crash and reboot at 12:11:29 CEST on October
10. See the [incident record](INCIDENT_2026_10_10_SYSTEM_CRASH.md). The persisted
EFI kernel trace is root-only and awaits a readable owner copy. Accessible logs
do not determine the cause. Pre-existing unreadable sectors on another disk are
recorded without attributing this crash to them or to Tack/GPU.

Final native UI (38 checks), separator pixels and Windows execution receipts
survived and match the delivered executable hashes. The final hardware
three-client receipt and two reports became empty during the reboot; this
attempt is excluded from complete evidence despite its surviving 16 check rows.
The earlier valid NVIDIA receipt is retained as candidate evidence.
[Post-reboot concurrent checks](../benchmarks/phase2a6/shared-post-reboot.json)
repeat all 16 checks on the delivered binary with explicitly selected llvmpipe
Vulkan. No simultaneous hardware-GPU clients were restarted while cause
investigation awaits the kernel trace. Final paired software measurements are
recorded separately from the pre-reboot hardware comparison.

[Final software-Vulkan paired runs](../benchmarks/phase2a6/local-paired-post-reboot.json)
and [frame telemetry](../benchmarks/phase2a6/local-frames-post-reboot.json)
use the delivered hash on both repeats. Results:

| Final software comparison | 2A5 | 2A6 |
| --- | ---: | ---: |
| Idle threads | 39.00 | 39.00 |
| Mean RSS | 107.11 MiB | 108.65 MiB |
| Mean native startup | 85.06 ms | 61.11 ms |
| Recurring redraw / I/O / IP sockets | 0 / 0 / 0 | 0 / 0 / 0 |

The 1.55 MiB software-driver RSS difference is a small measured variation,
not a permanent worker/dependency increase. Hardware and software timings/RSS
are distinct environments and must not be compared across those tables.
