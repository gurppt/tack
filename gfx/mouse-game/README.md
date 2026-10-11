# Authored Mulot game PNGs

Copied byte-for-byte from the artist's `work_icons/mulot_sprite` exports.
These are **individual numbered PNG frames**, not spritesheets. Numeric order
starts at 1. All original canvas padding and alpha values are retained; no
cropping, interpolation, filtering, generated replacement art or file watcher.

| Filename pattern | Count | Canvas | Frame duration |
| --- | ---: | --- | --- |
| animated_mulot_run{1..3}.png | 3 | 16×16 | 83.33 ms, loop |
| animated_mulot_idle{1..3}.png | 3 | 16×16 | 83.33 ms, loop |
| animated_mulot_attack{1..15}.png | 15 | 16×16 | 83.33 ms, once |
| animated_mulot_munch{1..2}.png | 2 | 16×16 | 83.33 ms, 3 seconds |
| explosion{1..12}.png | 12 | 16×32 | 83.33 ms, once |
| smoky_a{1..4}.png | 4 | 8×8 | 83.33 ms per puff frame |
| smoky_b{1..4}.png | 4 | 8×8 | 83.33 ms per puff frame |
| cheese_cursor.png | 1 | 16×16 | Native cursor, hotspot (7,7) |

Only the four source filenames `smokyb{1..4}.png` were normalized to
`smoky_b{1..4}.png`; their bytes are identical. Transparent final explosion and
smoke frames are intentional and remain part of playback.

Display at exactly 2× source pixels with a shared bottom-center anchor. The
16×32 explosion extends upward from the same ground anchor as the mouse.
Mulot faces left, so flip when travelling right; smoke is authored left-to-right,
so flip when travelling left. Preserve authored partial alpha/shadows.

Runtime path: executable sibling `gfx/mouse-game`, with repository fallback for
Cargo development builds. Builders seed missing PNGs only; portable updater
preserves existing artist replacements. At activation the existing local worker
loads 44 fixed PNGs (each capped at 64 KiB). At most 64 cached color masks per
frame, 24 smoke puffs and 64 existing paw effects. No additional GPU texture,
image codec, game framework or game worker is installed.
