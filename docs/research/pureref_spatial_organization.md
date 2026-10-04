# Spatial organization research — 2026-10-04

Verified against the official PureRef 2.1 handbook, accessed on this date.
These are documented behaviors, not measurements of proprietary implementation.

| Topic | Verified PureRef behavior | Tack decision |
| --- | --- | --- |
| Grid | G toggles grid visibility and snapping together; lines/dots/none; grid snapping can remain enabled with no visible grid. | Dots only, visibility independent of snapping; G / Shift+G. |
| Neighbor snapping | Shift+Space+left drag snaps to neighboring edges. | Explicit toggle; edges and centers; hold X to bypass snapping during a gesture. |
| Alignment | Ctrl+arrows aligns left/right/top/bottom. | Same edge shortcuts; Ctrl+Shift+Left/Up for horizontal/vertical centers. World AABBs. |
| Distribution | Ctrl+Alt+Shift+Up/Down lays items out in a horizontal/vertical row. | Equal nonnegative gaps, extending right/down when needed; separate row/column packing actions. |
| Packing | Ctrl+P attempts to pack items into the window. | Deterministic row/column packing with 16 world-unit gaps, not an optimal packer. |
| Groups | Ctrl+G / Ctrl+Shift+G; auto-fitting background; locked groups select as a unit; double-click enters a locked group. | Flat image membership only; clicking/marquee expands to the full group. Ungroup to edit individual members. No backdrop ownership or deep hierarchy. |
| Named zones | Handbook documents group backgrounds, parented notes and hierarchy. | Independent named rectangles, with no ownership of contained images. No separately documented PureRef frame object verified. |
| Temporary disable | No universal temporary grid/neighbor snap-disable shortcut established by the consulted pages. | X is a deliberate Tack binding, not a claimed PureRef default. |

Sources: [Canvas](https://www.pureref.com/handbook/canvas/),
[Images](https://www.pureref.com/handbook/images/),
[Arrange and align](https://www.pureref.com/handbook/images/organize/),
[Features and groups](https://www.pureref.com/handbook/features/).
No proprietary code, icons, fonts or assets were copied.
