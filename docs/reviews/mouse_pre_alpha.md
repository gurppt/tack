# Independent review — Mouse and Link pre-alpha polish

2026-10-11; read-only review by the existing `menu_safety_review` worker.
Implementation checkpoint: `24440e1af2013c05c71be3d1b75af3a8f361615c`.

Final verdict: no remaining concrete blocker. Review covered semantic tool
routing, temporary tool restoration, bounded effects and dotted overlays,
one-step ordinary image admission, shared identity uniqueness, Save As and
board replacement, cancellation and idle behavior.

Issues found and corrected: consume new Mouse pointer beginnings but still
release/cancel a prior gesture; finish an active Scribble before changing tool;
retain random identity bits for independent concurrent first image imports;
defer admission across Save As and reject late results for a replaced board;
use thin line triangles rather than a full diagonal bounding-box raster area.
Link Escape now clears pending acknowledgement/flash/timer state. A final
review also approves `64f09f2`: per-frame Link diagnostics are confined to
the existing optional `--output` path and its 7,200-frame bound; the harness
measures active geometry separately from settled cancellation.

A final suspected Close Board cancellation defect was withdrawn: the existing
`UiResult::Dismiss` already clears `close_board`. Nine native lifecycle checks
confirm rebased Save As followed by one-step Undo, clean board replacement
without a leaked image/deadline, and dirty Close Board cancellation followed by
one successful Mouse activation. These checks use owned fixtures and an
accelerated deadline, not artist boards.

Physical Windows use, aesthetic acceptance of gait and a real uninterrupted
ten-minute activation remain owner checks.
