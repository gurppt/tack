# Changelog

## 0.1.0-test.1 — isolated testing and ready-to-send reports

Owner-authorized TEST prerelease on `testing`, not a stable release.

- Select Test in Update after installing Dev.3; subsequent Test updates download and install through Tack, with restart/save guards and existing artwork/data preserved.
- Includes the optional diagnostic launcher and double-click launch/export scripts, updated by the same updater.
- Export creates a ready-to-send ZIP/STORE without compression or PowerShell, with captured structural logs, summary and checksum manifest. No automatic upload.
- Carries the Windows native-dialog and bounded >64 MiB JPEG/PNG corrections from the bridge. Diagnostics stay off during normal launch.

Install path from an existing Dev.2 build: Update on Dev → Dev.3 → Channel Test → Test.1. Very old Dev.1 installs with update protocol 3 need one initial compatible installation; do not overwrite their artwork/profiles.

Known limits: large ≥1 GB inputs, giant progressive JPEG, GIF, native PNG tile promotion, incremental Embedded save and two-machine LAN routing remain gated. Linux/Wine/compiler tests do not establish physical Windows large-image/GPU acceptance. `.tack` container 1 and LAN/update major 4 stay unchanged.

## 0.1.0-dev.3 — updater bridge to isolated testing

Owner-authorized pre-alpha bridge release from the `testing` branch.

- Adds the manual Test update channel, separate from Dev and Stable; same-core prerelease tracks can be switched explicitly.
- Keeps this bridge archive compatible with the old dev.2 updater (no new diagnostic file names). After updating, select Test in Update to receive the test kit.
- Carries native Windows COM picker fixes and bounded JPEG/PNG admission above 64 MiB, precise import failures and copy progress.
- Preserves CRC/fsync, boards, profiles and editable artwork. No background updater or new runtime codec dependency.

Known limits: 1.33/1.478 GB inputs, giant progressive JPEG, GIF and incremental Embedded saves remain gated. Physical Windows large-image/GPU and cross-machine LAN acceptance are pending; Linux errno 113 is not solved by a version change. `.tack` remains container 1; LAN major 4 is unchanged.

This bridge excludes the optional diagnostic launcher/scripts for old-updater compatibility; the next Test archive includes the ZIP/STORE report exporter. Existing public dev.2 remains unchanged.

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
