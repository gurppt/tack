# Editable pixel artwork

The defaults in `gfx/icons/` and `gfx/cursors/` include the author's October 10,
2026 update from `bin/gfx/icons/work_icons/`. PNG files were copied byte for byte;
no redrawing, recoloring or regeneration was performed. About uses the supplied
75 × 35 logo in `gfx/logo_tack_about.png`.

Toolbar, keymap previews and action cells share the same symbol table. The 37
symbols occupy a single nearest-filtered 128 × 80 RGBA atlas loaded once at
startup. The extra atlas row costs 8 KiB; the draw bound remains 32 visible quads.
Each symbol must be 16 × 16, at most 16 KiB, with alpha 0 or 255. Invalid/missing
files use an in-memory placeholder and never rewrite artwork.

| Added file stem | Action |
| --- | --- |
| `align_left`, `align_right`, `align_top`, `align_bottom` | Corresponding selection alignment |
| `arrange_in_grid` | Arrange selection in grid |
| `distribute_horizontal`, `distribute_vertical` | Corresponding distribution |
| `flip_horizontal`, `flip_vertical` | Corresponding flip |
| `bring_forward`, `send_backward` | One step forward/backward in stacking order |
| `cycle_color` | Cycle annotation color |
| `cycle_image_sampling` | Cycle image filtering |
| `rotate_view_tool` | Rotate view tool, including temporary tool |
| `toggle_snap` | Toggle snapping |
| `unlink` | Unlink from Frame |

Existing named symbols retain their indices. Temporary tools use the same
symbols as persistent tool selection. No unrelated action receives an invented
symbol.

## Cursor files and hotspots

Cursor PNGs are stored under `gfx/cursors/`, cached once, and assigned by the
existing canvas/UI ownership rules. All coordinates below are in the 16 × 16
source image.

| Author file stem | Installed stem | Hotspot |
| --- | --- | --- |
| `arrow_cursor` | `cursor_pointer` | 1, 1 |
| `hand_open_cursor` | `cursor_hand_open` | 7, 7 |
| `hand_cursor_closed` | `cursor_hand_closed` | 7, 7 |
| `crosshair_cursor` | `cursor_crosshair` | 7, 7 |
| `draw_cursor` | `cursor_draw` | 1, 14 |
| `eraser` | `cursor_eraser` | 4, 11 |
| `text_cursor` | `cursor_text` | 7, 7 |
| `move_cursor` | `cursor_move` | 7, 7 |
| `resize_nwse_cursor`, `resize_nesw_cursor` | `cursor_resize_nwse`, `cursor_resize_nesw` | 7, 7 |
| `resize_ns_cursor`, `resize_ew_cursor` | `cursor_resize_ns`, `cursor_resize_ew` | 7, 7 |
| `rotate_cursor` | `cursor_rotate` | 7, 7 |

Crop, linking-state and forbidden cursors retain the existing defaults. Alternate
single-direction resize drawings and Aseprite originals
remain in the author's working directory; there is no matching runtime action.

## Preservation

Normal builds and existing-directory Windows packaging install missing defaults
only. They preserve runtime PNG edits under `bin/**/gfx/`. A fresh portable release
includes the tracked source defaults. The explicit author-requested import is a
separate one-time copy, not a new overwrite step in build scripts. Restart Tack to
load edited icons or cursors; there is no file watcher or idle polling.

## Optional Mouse (Mulot)

The nine author-approved `work_icons` PNGs are copied byte for byte into tracked
defaults: `mulot_icone` and the six `pawL/R_{n,45,w}` plus `poo_easter` under
`gfx/icons/`, and `minimulot` under `gfx/cursors/` (hotspot 7,7). The semantic
Mouse/Temporary Mouse actions share atlas slot 36; no default toolbar or keymap
entry is added. The atlas remains 128 × 80. Paws use startup-cached binary masks,
with mirrors of the authored orientations; original files are never generated
or rewritten. Import hashes: [artwork receipt](../benchmarks/mouse-polish/artwork-import.json).
