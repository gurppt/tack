# Changelog

## 0.1.0-dev.2 — portable development release

- Reduced toolbar catalog while keeping commands in menus/keymap; optional note alignment with explicit left/center/right/mixed icons.
- Visible creation of named camera bookmarks, distinct from local view shortcuts.
- Larger top-edge window strip, accessible toolbar buttons, and note Enter handling during IME composition.
- Multi-image Actual Size/grayscale/crop controls and explicit locked-selection behavior.
- Nonfatal image-render saturation: bounded, signaled presentation with the complete board still editable, saveable and recoverable.
- Lower supply cost for many visible instances of one asset; consecutive compatible GPU draws batch without reordering alpha or annotation interleaves.
- Portable Linux/Windows x64 archives include the pinned libjpeg-turbo SIMD decoder, authored graphics and the optional LAN server/updater.
- Update manifest identity aligned with the current LAN major 4; old major 3 peers require an upgrade.

Pre-alpha test release. Physical Windows and multi-machine LAN acceptance remain pending. See the UI Cleanup, 2A12B, 2A12C1 and 2A12C2 reports under `docs/` for exact measurements, workload limits and test coverage.

## 0.1.0-dev.1 — unpublished integration

Phase 2A8 foundation delivered on `dev`; physical alpha acceptance remains pending.

- Workspace SemVer, About logo/version/commit/channel, and Dev/Stable release policy.
- Manual update checking and a separate bounded portable updater; journal recovery,
  rollback and one retained previous package. No background update service.
- Linux/Windows release dry-run packaging with matching manifest/build identity.
- Cached pixel cursors, annotation-lock ON/OFF artwork and image-group outlines.
- Compound Scribble sessions with explicit finish, target-only vector Eraser,
  painter-order Merge and exact Undo/Redo/save/reopen.
- Flat indexed Frame links: move carries children; resize/delete/unlink retain
  child world positions. Shared scopes and inverse budgets remain enforced.
- Flip for images, Rectangle, Line, Arrow and Scribble.
- Author-provided pixel icons/cursors/About logo copied without regeneration;
  common toolbar/keymap symbols for alignment, arrangement, flip, sampling,
  stacking order, color, snapping, rotate view and unlink.
- Local-minimal capability admission gate and timed-media interface only.

Compound/link records use schemas 6/7; LAN major 3 requires incompatible peers to
upgrade. Legacy schemas 1–5 remain readable. No finished playback backend, accounts
or analytics. Application license/contact remain to be defined. Physical Windows,
artist feel, two-computer LAN, published controlled update and power-loss tests
remain human gates. See the [phase report](docs/MISSION_2A8_ALPHA_FOUNDATION_REPORT.md).
