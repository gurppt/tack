# Scribble and Frame links — Phase 2A8

Scribble is a single annotation with up to 256 independent strokes and 4096
points in total, stored in its local box. Ordinary pointer releases end a stroke;
Enter, a tool/action change, Save, Close or opening the menu ends the session.
There is no completion timeout. Escape discards the current session. Navigation
retains completed strokes; beginning a pan/zoom finishes the active stroke first.
A temporary Hold Scribble completes on release. The last valid preview remains
when a point/split budget is reached; finish the gesture/session to continue.

E selects the Scribble-specific Eraser. It edits one selected or hit Scribble,
never images, notes or another shape. Its swept circular footprint splits local
segments, including fast pointer movements. Geometry stays transient until release;
release produces one undo command, Escape cancels. Erasing every segment removes
the object; undo restores its Frame relation too. Disjoint shared edits rebase the
draft; edits touching the target cancel it.

Merge Scribbles is available for multiple selected Scribbles. It visits them in
board painter order, moves strokes into one local box, preserves per-stroke color,
width and opacity, and places the result at the topmost selected position.
Unselected objects keep their relative order. Merge refuses an intervening
unselected non-Frame object whose bounds overlap the selected union, because
one merged object cannot preserve that alternating painter order. The selected
Scribbles must share one Frame parent (or all be unlinked). Merge is one undoable
transaction. Color Cycle applies to every stroke; width/opacity edits retain
relative stroke differences. There is no Explode operation.

Ctrl+L / Link to Frame begins a transient Frame picker. With no selection it
reuses normal picking/marquee and image-group expansion, then enters Frame picking.
Otherwise it keeps the selected linkable units. Dotted lines are one logical UI
pixel, static 4-on/4-off, clipped to the visible screen. Each line uses one thin
quad in the existing overlay pass, with at most 256 lines; remaining capacity
selects a deterministic subset or skips optional decoration. Simplification
never changes the complete Link command or shuts down the app. A valid Frame
highlights on hover; initiating press and matching release commit. Invalid targets
show Forbidden after the attempt; Escape cancels without mutation. The prior tool
is retained. Confirmation is one 1.2-second deadline; shared confirmation only
appears after authoritative links are present. A slow shared acknowledgement can
outlast that feedback deadline without changing the command's outcome.

Ctrl+Shift+L / Unlink from Frame removes selected relations in one command and
preserves world positions. Select Linked Objects appears on a Frame context menu.
The default toolbar includes Link; existing personal layouts are preserved.
All these actions are remappable.

Relations are flat `child ObjectId → parent Frame ObjectId`; an indexed reverse
map resolves children once at the beginning of a Frame drag. Motion costs O(linked
children), with no board scan per motion and no idle work. Moving a Frame translates
its children; resizing/title/style do not transform them. Deleting a Frame leaves
children in place and clears links. Frames cannot be children. Every member of an
image group has the same parent; incompatible grouping/relinking is refused.
Ungroup retains member links. Duplicating children retains their parent; duplicating
a Frame together with selected children remaps those copies to the copied Frame.
A Frame copied alone does not acquire the original's children.

Shared operations use the same commands, authority and object conflict clock.
Frame transform scopes conservatively include children even for resize, so batches
cannot bypass a child's lease. Large generated inverses are refused before authority
publication if they cannot fit the 1 MiB network operation budget. Local history
keeps its existing separate bounds. Large shared drags can exceed the existing
256-target lease bound and are refused rather than silently truncated.

Schema 6 stores compound/per-stroke Scribbles; schema 7 additionally stores Frame
links. Legacy one-stroke annotations retain schema 3 when no newer data is needed;
schemas 1–5 remain readable. LAN protocol major is now **3**; major-2 peers must
upgrade. Highlights, dotted lines, drafts and confirmation feedback are not saved.
