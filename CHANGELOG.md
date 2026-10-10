# Changelog

## 0.1.0-dev.1 — unreleased integration

Phase 2A8 development; not yet an alpha candidate.

- Workspace SemVer and About build identity.
- Manual Dev/Stable update UI and separate bounded portable updater.
- Release candidate packaging/workflow, rollback and previous-package retention.
- Cached pixel cursors, annotation-lock ON/OFF artwork, image-group outlines.
- Flip for Rectangle, Line, Arrow and Scribble.

Known limitations: remaining compound Scribble and Frame-link implementation,
packaging/performance/human acceptance still pending. Collaboration protocol is
currently major 2; incompatible peers are refused. No finished timed media or
background update service. Application license/contact remain to be defined.

Phase 2A8 board editing: compound Scribbles with explicit finish, target-only vector
Eraser, painter-order Merge, flat translation-only Frame links and exact Undo.
Frame/group invariants, indexed drag children and shared scope/inverse budgets
are enforced. Compound/link records use schemas 6/7; LAN major 3 requires peers
to upgrade. Physical Windows/artist/two-computer LAN acceptance remains pending.
