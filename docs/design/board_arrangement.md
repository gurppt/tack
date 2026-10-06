# One-shot board arrangement and tool completion — Phase 1I

`Layout(Grid)` and `Layout(SnapToGrid)` use selected independent layout units.
An image group is one unit, including all its members; other images, annotations
and frames are individual units. Rotated objects contribute their world AABB.
The command translates units, preserving image size/aspect, rotation, flips,
crop and internal group geometry. It is a normal `Command::Batch` of transforms:
one history entry, exact Undo/Redo, ordinary save/reopen, no serialized constraint
or persistent arrangement worker. A no-op does not add history.

Grid arrangement sorts unit top-left coordinates by Y, then X, then stable unit
ID. The union's aspect is clamped to 2/3…1.75; count and summed widths/heights
estimate a column count. Five bounded neighboring candidate counts are scored
using bounding-area waste plus 0.5 times logarithmic aspect error, with stable
tie breaking toward fewer columns. For three or more units, candidate bounds
ensure at least two columns and two rows. Two units may occupy one row.

Rows start at the original selection's top-left. Items are top aligned within
each row; the row height is its largest AABB height. Horizontal and vertical
minimum gaps are exactly 16 world units. Portrait and landscape sizes remain
natural, so the space below shorter items is intentionally uneven. Five linear
measurements plus sorting and indexed group/document lookup give O(n log n)
time and O(n) temporary memory. No pairwise solver or equal-size normalization.

Snap Selection rounds each unit's AABB top-left independently on both axes to
`GRID_BASE = 64` world units, including negative coordinates. It uses the base
board lattice, independent of camera zoom or UI DPI, rather than every
adaptively displayed dot (at zoom 1, dots may be 32 units apart). Members of a
group share the same translation. Snapping can introduce overlap or reduce a
previous gap: it is independent quantization, not a second arrangement solver.
A single unit can invoke it through a remapped action; the existing single-image
menu is unchanged. Multi-selection exposes both actions first in Arrange,
followed by the retained six align, two distribute and two pack commands.
The flat 12-row child menu fits at 800×600 without a permanent panel.

Note/Text, Rectangle, Line and Arrow are one-shot tools. Successful completion
returns Pointer and does not create a separate history operation. Frame creation
also returns Pointer. Note draft entry already restores the Pointer base while
the text modal owns input; commit/cancel retains Pointer. Freehand/Scribble
remains active for repeated strokes; Escape or V restores Pointer.

Escape cancels uncommitted creation without history. Focus loss drains held
inputs and cancels unfinished one-shot creation. Completed/cancelled one-shot
creation clears the existing temporary-tool overrides. Modal reset additionally
clears held-input state; discarded release tokens cannot leave orphan overrides.
Nested experimental Pan/Text on a Pan base is covered for commit/cancel/focus.
Temporary Pan/RotateView remain experimental, not a new dedicated gesture system.

Tidy, equal-size mode, persistent auto-layout and Phase 2 are deferred. Numeric
costs and native evidence are recorded in [the phase report](../MISSION_1I_REPORT.md).
