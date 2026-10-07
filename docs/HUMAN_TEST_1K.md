# Phase 1K owner validation

Launch `./bin/tack`. Check `./bin/BUILD.txt` for the Phase 1K checkpoint and hash.
Automated verification uses generated boards; the previously problematic real
board still needs owner testing before the next mission.

1. Open the original board with several large PNG/JPEG images.
2. Zoom strongly, wait for sharp detail, then dezoom slowly. Pay attention near
   the former intermediate-size quality hole; adequate resident HD should bridge
   the arrival of the smaller tier.
3. Cross thresholds repeatedly, reverse the wheel rapidly, pan slightly, stop,
   and check that every available visible image settles to appropriate detail.
4. Repeat with grouped/rotated images, Smooth and Nearest, and several images in
   view. Missing/changed sources still need their existing explicit relink flow.
5. At 800×600, open Preferences (Ctrl+,), decrease/increase Handle Size and Hit
   Radius with mouse -/+ and keyboard Left/Right. Close/reopen to check values.
6. Choose UI Scale 1× then 2×, and Very Dark / Neutral Gray / Light. Backgrounds
   should be flat; essential controls should remain reachable.
7. Open Keymap, search `Undo`, click the row then Change (or Enter), and capture
   a free shortcut. Check the new menu shortcut, then Unassign/Reset action.
8. Search `Pointer`, a category, and a current shortcut label. Escape should
   clear search first and close only when it is empty.
9. Deliberately try Ctrl+V for Undo: the panel should name Paste and refuse the
   conflict without losing Undo's previous binding. Cancel capture.
10. Export from Keymap, change an assignment, import the exported file, and
    confirm restoration. Cancel a global reset, then try explicit confirmation
    if desired. Revert experimental assignments before normal use.
11. Stop moving/clicking with panels open and closed; the application should
    settle without recurring redraw or codec activity. Normal successful paste
    should not print detailed clipboard negotiation.

Record any remaining visual LOD anomaly with the approximate zoom sequence,
window size, sampling and source availability. Optional developer receipts:
`./bin/tack open BOARD.tack --lod-debug --output /tmp/tack-lod.json` (close normally
to finish the receipt). This is opt-in diagnostics, not the normal idle mode.

Stop here for owner review. Collaboration, networking and timed media are not
part of Phase 1K.
