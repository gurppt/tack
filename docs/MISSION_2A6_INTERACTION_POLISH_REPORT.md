# Phase 2A6 — interaction polish

Implementation and local automated validation PASS; physical artist acceptance
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
Windows UI acceptance remains pending. Linux/Windows CI result is recorded
below after publication of the implementation commit.

## Measured cost

Two serial paired runs on the same owned display/GPU and small local board,
warm/unspecified OS page cache; [raw comparison](../benchmarks/phase2a6/local-paired.json)
and [frame telemetry](../benchmarks/phase2a6/local-frame-telemetry.json) are retained.
This narrow sample does not establish startup or large-board performance gains.

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
is about 24 MiB; committed compact evidence is under 200 KiB. An owned Wine
prefix temporarily used the documented allowance of up to 2 GiB, then was stopped
and removed by the harness. The 10 GiB free-space reserve was retained. No user
board/profile/artwork or broad cache directory was cleaned.

The [human checklist](HUMAN_TEST_2A6_INTERACTION_POLISH.md) covers artist feel,
physical Windows and physical two-computer acceptance, which are still pending.
No Studio Server, WAN/auth, media, importer, new text tool or other 2B work was
started. Stop after 2A6.
