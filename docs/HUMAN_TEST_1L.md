# Phase 1L — owner checklist

Use the verified `./bin/tack`; check `bin/BUILD.txt` for its checkpoint and SHA.
The final automated evidence is in [MISSION_1L_REPORT.md](MISSION_1L_REPORT.md).
Try an 800×600 window. These checks capture usability and visual comfort; they
do not replace the automated safety/format/cache tests.

Subjective review is deferred in the local director queue `docs/HUMAN_REVIEW_PENDING.md`
and does not block independent work after technical validation. Corruption,
unsafe allocation and major regressions still block. The director audits the
closeout and activates the next brief; this checklist does not start Phase 2A.

## Files, sources and fresh Open

1. Launch Tack fresh. Choose File → Open and open a saved board. The initial
   empty window should become that board, with no spare empty window. Its title
   and content must agree.
2. Launch fresh again; cancel Open, then try an invalid board. The same fresh
   document must remain usable. Create a Note or import an image, then Open
   another board: your existing work must survive in its original window.
3. Save As `foo`, then `foo.bar`: verify `foo.tack` and `foo.bar.tack` are created.
   An existing `.tack` or `.TACK` suffix should not be appended twice. Reopen the
   saved board and confirm its content.
4. Open/Save As successfully in another board folder. The next board picker
   should start there. Cancel it and reopen: the useful remembered directory
   should persist. A removed remembered folder should fall back cleanly.
5. In Keymap, export to `studio.tackey`, change a spare binding, then import the
   saved file and verify restoration. Compatible legacy JSON still imports.
   Invalid imports must leave the current bindings intact; restore experiments.
6. Select a linked image, choose Source → Save Original As, and save elsewhere.
   Repeat with an embedded image. Crop/rotate first if desired: the saved bytes
   must remain the original, not the displayed crop. On Linux compare:

   ```sh
   sha256sum /path/to/original.png /path/to/exported.png
   ```

   Cancel export and try a missing linked source: expect cancellation/error
   without losing the board or creating a partial export.

## Selection, Preferences and annotations

1. Marquee more than 20 images; every visible selected image should have a
   contour. Add another marquee with Shift, then move/resize/rotate immediately.
   Repeat with grouped/rotated images and Notes; deselection clears contours.
2. Open Preferences → Theme. Choose several themes: preview applies immediately
   and stays in Theme. Esc returns to Preferences; a second Esc closes it.
3. Open Keymap through Preferences. With search/capture cleared, Esc returns to
   Preferences. Open Keymap directly: Esc closes directly. During search/capture,
   Esc cancels that state first. Repeat open/back/close and check keyboard/mouse
   focus, essential controls and menu fit at 800×600, UI Scale 1× and 2×.
4. Create a multi-line Note. A normal corner drag changes its box and wrapping,
   leaving text size unchanged. Undo once restores the whole drag. Shift plus a
   corner drag scales the whole Note, including text and style. Undo/redo once,
   edit text size afterward, save/reopen: values should remain stable.
5. Draw a default straight arrow and judge shaft/head legibility at normal zoom.
   Curved arrows and endpoint attachments are deferred; do not expect them here.

## Huge PNG and production-bank feel

The fixtures below are generated procedural data. Huge tile detail is experimental
and must be enabled explicitly. Static noninterlaced PNG is supported; large
interlaced/APNG sources and oversized JPEGs have codec admission limits. Failure
must be explicit and leave the document usable. This is not a 128 MiB hardware
validation. See [the design note](design/huge_images.md) for bounds and limitations.

Compare ordinary opening with opt-in tiles:

```sh
./bin/tack open test_file/phase1l_generated/boards/single_huge.tack --window-size 800x600
./bin/tack open test_file/phase1l_generated/boards/single_huge.tack --window-size 800x600 --huge-tiles
```

1. Wait for the overview. Deep zoom, pan rapidly to a distant area, return to the
   first area, and zoom out sharply. Repeat the reverse sequence, close/reopen,
   and check coverage and detail convergence. Input should remain responsive
   while work is running. Sequential tile scans may take seconds; nearest mip
   sampling can alias and Smooth can expose tile edges.
2. Rotate, flip, crop and change opacity on a copy of the huge object. Check
   boundaries through zoom changes, then save/reopen. Derived tiles must not
   affect the original source bytes or document authority.
3. Open `several_huge_png.tack` for separated sources, then
   `overlapped_huge_png.tack` for three overlapping huge PNGs. Compare overview
   and deep zoom; opaque overlaps intentionally hide lower images visually.
4. Open `huge_plus_ordinary.tack`, then `mixed_huge_visible.tack`. Check that
   ordinary references become usable while huge detail prepares. Repeat with
   `--huge-tiles --potato`; reduced detail is expected, missing coverage is not.
5. Open `mixed_prod_1000.tack`, `stress_1000_paths.tack`, and
   `stress_250_unique.tack`. Pan/zoom aggressively, select many objects, move
   them, then leave the app idle. The hardlink and distinct-source fixtures are
   different workloads; test both. Save edits outside the generated directory
   if you want to keep them.
6. After work settles and menus close, leave the window untouched. There should
   be no recurring Tack redraw/decode activity. A quiet placeholder from a
   rejected source is a failure result, not successful detail convergence.

All named boards are under `test_file/phase1l_generated/boards/`. Do not regenerate
the existing corpus just for this checklist. Cleanup instructions are in the
design note and remove only the sentinel-owned generated subtree.

For a useful report, record the board/source, window size, sampling mode,
default/potato and tile flag, exact zoom/pan sequence, and a screenshot of the
problem. On Windows also check native pickers, hidden extensions and focus;
Linux automation or Windows CI compilation is not a Windows desktop acceptance.
