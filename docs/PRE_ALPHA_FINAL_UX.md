# Final pre-alpha UX build

2026-10-11 — source `36a97bbefe00c67ef7234500d7e40e9ce0884bd3`, version 0.1.0-dev.1.

- Keymap displays authored tool/action icons beside their labels, using the
  existing toolbar atlas and only visible rows. Actions without an authored
  icon keep their text. No artwork is regenerated or replaced.
- Behavior displays `onClick` instead of `Normal`. This is a label change:
  Press/Hold/Release behavior and saved keysets remain compatible.
- Link-to-Frame accepts its visible title tab on press and release, including
  a press in the Frame followed by release on the tab. Picking shares the
  displayed title geometry at the current zoom, DPI and title size. Unavailable
  Frames remain refused; the matching-Frame rule and Undo are preserved.
- Only the Mulot cursor is doubled: nearest 32×32, hotspot 14,14. The 16×16 PNG
  is unchanged; other cursors and paw sprites retain their existing size.

`tools/check.sh`: 432 Rust / 31 Python tests pass; nine explicit software-Vulkan
GPU checks pass. Native checks: 21 Mouse/UI assertions (including exact authored
Keymap pixels and exact 2× cursor readback), and six Link assertions, including
all 32 objects linked by release on the title tab, exact Undo/Redo and no redraw
for two seconds after Escape. Geometry tests additionally cover title scaling,
DPI/zoom combinations, blocked Frames and one-step Undo.

Deliverables: `bin/tack` and `bin/tack-windows-x86_64.zip`, with the pinned native
SIMD JPEG decoder. Physical Windows desktop acceptance remains pending.
Exact artifact hashes and artwork preservation are recorded in
[the verification receipt](../benchmarks/mouse-polish/final-ux.json).
Raw screenshots/checks remain in `benchmark-results/mouse-polish/final-ux-*`.
