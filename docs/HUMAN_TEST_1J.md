# Phase 1J owner check

Technical native tests run on an owned X11 display; aesthetic approval and your
ordinary desktop clipboard producer are still yours to assess. Run the current
`./bin/tack`; `bin/BUILD.txt` identifies the tested executable. Use a disposable
board and normal user profile. Record the display backend, helper, exact error
if any, and aesthetic comments separately from correctness.

1. Copy a screenshot using your usual application, then Ctrl+V. One embedded
   image should appear and survive saving/reopening without a temporary file.
2. Copy one PNG, one JPEG, then several images in your file manager and paste.
   All valid images should be admitted. A bad/remote entry should give a single
   rejection summary while keeping valid local images.
3. Copy plain text, including `/review notes`, then Ctrl+V on canvas: one note.
   Paste into an active text editor too; confirm and undo through normal history.
4. Right-click canvas, image, multi-selection, note, frame and annotation. The
   top Tack section stays identical; the lower section matches the target.
   File/Edit/View/Tools/Preferences/Keymap remain reachable. F10 exposes the same
   application commands. Check Escape, outside dismissal and screen edges.
5. Preferences → UI scale: select 2×, then 1×, then Auto. Reopen the application
   and check persistence. Repeat at 800×600; both parent and child menu should
   remain usable at 2×. The permanent top-left Tack button is gone.
6. Preferences → Background: Neutral Gray → Very Dark → Light. Inspect text,
   shortcuts, disabled/selected states, editor, notes, guides and grid. Assess
   whether the cyan/magenta/yellow hierarchy is restrained and readable.
7. Pan/zoom: the subtle gradient stays in screen space. Change themes, save and
   inspect: no board geometry or schema change. Reopen to check profile settings.
8. Repeat at 800×600/1× and 2×, 1024×768 and a larger window. Leave the board
   settled, with menu closed/open; no visual animation or clipboard activity.

On Linux, install `xclip` for X11 or `wl-clipboard` for Wayland if the explicit
helper message appears. `xsel` supports text only. This session made `xclip`
available in `~/.local/bin`; ensure that directory is on your launcher's PATH.
The isolated Flameshot/private-D-Bus launch failed during automation, while
actual owned-window screenshot + X11 clipboard paste passed. Please check your
ordinary screenshot application on your normal desktop. No Windows or Wayland
clipboard runtime pass is claimed by this Linux X11 evidence.

Owner result: **pending**. Record comfort/discovery, gradient subtlety and accent
preferences here or in the next brief before the next major branch.
