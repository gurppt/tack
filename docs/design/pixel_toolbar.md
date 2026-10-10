# Primitive toolbar and editable icons

One optional toolbar projects semantic Action IDs through the same dispatcher
as menus/keybindings. Defaults: Pointer, Pan, Text, Rectangle, Line, Arrow,
Scribble, Frame, Duplicate, Undo, Redo, Share. Pan uses existing camera pan;
tool choice and camera remain local even on shared boards.

View → Toggle Toolbar / Toggle Status Bar and Tools → Edit Toolbar are remappable
semantic actions. The temporary two-column editor lists Available Actions and Toolbar Order,
with Add/Remove, Up/Down and Ctrl+Up/Down, cycles Top/Left/Right/Bottom/Floating/Hidden and resets the defaults.
The grip moves floating or docked placement within the canvas. Docked edges
center by default; `edge + offset` persists an explicit user offset. Preferences persist at most
32 action IDs or Separator items and bounded placement coordinates. Unknown/duplicate/held-only
Action IDs and adjacent separators normalize away. Toggle from Hidden restores the last visible placement. F9 is the new default.
Preference saves merge independent setting/recent-board changes across windows;
conflicting writes to the same setting are reported without a modal.

Geometry is a fixed 32-button array, integer-snapped at scales 1–4, wrapping
and clamping to the client area. Resizing relayouts hit rectangles. Active canvas
gestures retain their releases when crossing chrome. There is no docking tree,
widget framework or external panel window. Action cells are exactly 16×16 logical
pixels, with no outer outline/padding. The functional grip occupies one 16×16
cell. Separators occupy one pixel and draw a centered 1×12 line (rotated on
vertical bars). Context menus draw over the toolbar without suppressing it;
normal occlusion applies. Modal panels replace the bar icon quads with their
own previews from the same atlas.

## Artist files

Edit `gfx/icons/<name>.png` in GrafX2/Aseprite and restart Tack. No Rust rebuild
is needed. The development build reads the repository icon directory even from
another working directory. Packaged copies fall back to `gfx/icons/` beside the
executable when the development directory is absent. `TACK_ICON_DIR` explicitly
overrides the directory for testing/portable iteration.

Icons beside the executable (`bin/gfx/icons/` for the local Linux build,
`bin/tack-windows-x86_64/gfx/icons/` for the Windows package) are editable user
artwork. Build and packaging scripts install only missing PNGs and preserve
every existing file, including custom names. Rebuilding never resets this folder.

Format: exactly **16×16 RGBA PNG**, encoded file ≤16 KiB, alpha 0 or 255, integer
pixel artwork. Files: pointer, pan, text, rectangle, line, arrow, scribble,
frame, duplicate, undo, redo, share, join, save, grid, placeholder. Other semantic
actions use the deterministic placeholder. IDs remain authoritative regardless
of icon artwork. Edit Toolbar previews actual mapped icons; unmapped actions
and separators have text only. Other menu/keymap rows have no icons.

The existing bounded PNG decoder rejects malformed/oversized/wrong-dimension or
soft-alpha icons, logs a concise diagnostic and substitutes a stable placeholder.
`tools/prepare_toolbar_icons.py` creates original starter symbols only when files
are missing; it never replaces artist edits. It is not a runtime dependency.
No copyrighted reference bitmap was reused.

## Cost and status

Sixteen icons load once before the event loop into a 128×32 RGBA atlas:
16,384 bytes. GPU resources total **20,224 bytes** including the fixed 32-quad
vertex buffer. Decoded startup CPU pixels are released after upload. Hidden
toolbar retains these tiny resources but submits zero icon quads. Nearest
sampling gives exact integer pixel expansion; no SVG, antialiasing or watcher.

Static draw geometry uses stack arrays. Hover/state text is cached and reads
the current keymap; preferences invalidate it. The optional bottom strip shows
one action/shortcut or a stable saved/shared state. Connection blocks are fixed
green/yellow/red rectangles; Connecting/Reconnecting are yellow. Shared boards keep the strip and indicator visible even when the local strip
preference is hidden. Ordinary local boards have no indicator.
Add selects the new Order row, minimally scrolls to it and shows a green
1200 ms acknowledgement. Ctrl+Up/Down reorder, Left/Right change columns,
Tab visits controls, Enter activates, Escape closes. Remove selects the nearest
survivor. Copy invite uses the same deadline primitive. No fades, hover timers
or recurring idle redraws remain after feedback settles. Text fields blink a
pixel block caret only while editing and focused; deadlines stop on exit.
