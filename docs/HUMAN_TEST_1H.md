# Human/native validation — Mission 1H

**Pending human review.** Native automated checks now pass at 800×600 and
1024×768, including 1×/2× captures and open/closed idle; see
[the updated report](MISSION_1G_REPORT.md). Automation does not complete this
uncoached user checklist. No user observation is claimed. Test the current `./bin/tack` and record `./bin/BUILD.txt`, OS, GPU,
window backend, physical display and scale. Use disposable images/boards.

Give a new tester only the task list first, without the shortcut reference or
source code. Observe which controls they discover without coaching. Record
failures and whether any documentation/hint was needed.

1. Launch the empty board; import or drop several images.
2. Select, move, resize and rotate one image using its handles.
3. Right-click it; discover crop, flips, sampling and opacity. Apply a command.
4. Undo with Ctrl+Z; redo with Ctrl+Shift+Z. Repeat after a long drag: one Undo
   should restore the complete gesture, not a single motion.
5. Multi-select; right-click; group/ungroup and align supported selections.
6. Right-click blank canvas; discover import, paste, note and frame creation.
7. Create/edit a note and a frame; verify their specific menus and cancellation.
8. Discover Save, Save As, Open and Preferences through the tiny Tack entry.
9. Save and reopen; compare crop, sampling, opacity, geometry, object order,
   note text, frame names and grouping.
10. Dismiss menus with Escape, outside click and activation; check that the
    dismissal click does not start a canvas drag. Open at all four corners;
    verify submenus, disabled entries, real shortcut labels and keyboard access.

Repeat in native **800×600** and **1024×768** windows, UI 1× and 2×. Check actual
text readability, crisp glyphs/chrome, handle reachability, canvas dominance and
all transient Preferences/Keymap/Close/Recovery/Error panels. Exercise resizing
with a popup open and monitor/DPI changes. 4×/8× stress checks may truncate text;
they are not comfort/readability passes. Native file pickers are OS dialogs;
check them too. Test Windows on a real Windows desktop separately.

For the reviewer only: V pointer, T note (click/drag then type, Ctrl+Enter), R
rectangle, L line, A arrow, P freehand; F2 Edit/Rename. Note drafts have **no
local text Undo**; Escape discards, Ctrl+Enter commits, then document Undo applies.
New Frame uses current selection bounds or the viewport. Groups contain images
only. Frames are spatial regions, not membership containers. OS close remains
the existing Quit route with Save/Discard/Cancel.

## Native CPU/RSS/idle reproduction

On an **owned isolated X11 display**, open a generated empty board and run the
existing untimed idle harness. This extension now passes native open/closed/canvas/application idle probes. Supply must settle before an idle result is
interpreted. It records process/task CPU ticks, switches, RSS, file I/O and late
GPU submissions with no application benchmark timer.

```bash
./bin/tack new benchmark-results/human-1h-empty.tack
DISPLAY=:94 python3 tools/run_idle.py --binary ./bin/tack \
  --board benchmark-results/human-1h-empty.tack --no-wm --seconds 5 \
  --context-menu canvas --output benchmark-results/human-1h-idle-canvas
```

Use separate output directories for `--context-menu application`, `closed`, and
`none`. The harness refuses menu probes on `:0`; it must never drive the user's
normal desktop. Record native open/closed menu screenshots alongside receipts.
Do not mark acceptance PASS from the offscreen example or harness source alone.
