# Human validation — phase 1G

Launch from the repository: `./bin/tack`. Read `./bin/BUILD.txt` first; it must
say `STATUS: CURRENT`. Rebuild with `bash tools/build-test-bin.sh` if needed.
Do not run the operating-system `/bin/tack`.

No human visual observation is recorded by the automated mission. The native
X11 receipts prove specific synthetic input/save/coverage/idle behavior; they
are not a Linux desktop/DPI/monitor observation or a Windows manual claim.

Use disposable copies of representative local boards and images. Record OS,
GPU, display backend, scale, build stamp, board kind and any problem for review.

- [ ] Create a board; import several small and large images with Ctrl+I.
- [ ] Drag local images into the window; paste a clipboard image with Ctrl+V.
- [ ] Create/edit a note (`T`, drag, type, Ctrl+Enter); test accents/fallback glyphs.
- [ ] Draw rectangle/line/arrow/freehand; confirm edges and chrome stay crisp.
- [ ] Create frames, select/group objects, move a group and undo/redo.
- [ ] Move a linked source away; relink explicitly, preserving crop/layout.
- [ ] Ctrl+S, Save As, reopen: same document, sampling and original authority.
- [ ] Close with dirty edits: exercise Save, Discard and Cancel separately.
- [ ] With a disposable board only, simulate interruption after autosave; Restore
      must remain dirty until Save, and Discard must keep the normal board.
- [ ] Open two separate boards; alternate editing/navigation/Save in each.
- [ ] Resize while image supply is active; move between displays/scales.
- [ ] Zoom in/out aggressively and reverse; pan during zoom, use mixed sampling.
- [ ] Jump between distant image clusters, zoom in until a sharper tier arrives,
      jump back: no old-image replacement, blank stall or UI corruption.
- [ ] Leave empty/small/large windows idle for several minutes; no continuing
      loading loop, flicker, title churn or unexpected disk activity.
- [ ] Reopen after sustained edits and navigation; verify saved state.

Generated fixtures for repeatable visual checks:

```bash
python3 tools/generate_supply_images.py benchmark-results/human-1g-images
./bin/tack supply-scale benchmark-results/human-1g-boards benchmark-results/human-1g-images
./bin/tack benchmark-results/human-1g-boards/sparse.tack
./bin/tack benchmark-results/human-1g-boards/dense-highlod.tack
```

`--potato` after the board path constrains developer resource budgets. It tests
pressure, not the timing of old hardware. `--dense-view` opens an intentionally
wide test view. `--present-immediate` is for presentation diagnostics only.

Windows: CI compiles/tests native helper code, paths and core/input/DPI/GPU
contracts. A real Windows clipboard/picker/monitor test is still required before
claiming Windows native visual/manual validation. No Windows machine was used
for human observation in this phase.
