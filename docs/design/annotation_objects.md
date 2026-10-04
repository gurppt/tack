# Phase 1E annotation objects

Implemented 2026-10-04. The six annotation kinds extend the resident Document,
commands/history, selection/input, storage and common GPU canvas. There is no
second document, widget tree, vector framework or annotation worker service.

## Durable model and authority

`ObjectKind::Annotation(Box<Annotation>)` holds Text, Rect, Ellipse, Line, Arrow
or Scribble. IDs and document order are shared with images/frames. Object stays
160 bytes, Command 176 bytes; Document becomes 208 (+16). Annotation itself is
72 heap bytes plus retained text/point capacity. ImageInput becomes 832 (+80),
with lazy boxed creation/editor state and empty vectors while unused.

Rect/ellipse/text use the centered rotated Transform box. Line/arrow endpoints
and scribble points are normalized inside it, so manipulation never clones the
point array for every pointer event. Stroke width is in world units, 0.1..256;
colors are RGBA8; opacity is finite 0..1. Expanded stroke/arrow bounds must remain
inside existing world limits. Text is at most 16,384 UTF-8 bytes, allows newline
and tab, has font size 4..256 world units and left/center/right alignment. Points
are finite normalized pairs, 2..4096 per stroke. Schemas 1/2 remain readable;
[explicit schema 3](tack_file_format_v3.md) is written only with annotations.

One completed creation, note edit, style action or manipulation is one atomic
history entry. Text/point capacities count against the existing 32 MiB history
budget. An oversized inverse is applied back immediately, before generation,
dirty state, redo or history publication. A regression deleting 520 maximum-size
strokes verifies refusal preserves both the document and an existing redo entry.

## Interaction and geometry

T/R/O/L/A/P select note/rectangle/ellipse/line/arrow/scribble; V returns to pointer.
Creation reuses the existing semantic temporary-tool ownership. Navigation,
Escape, focus loss and DPI changes discard pending annotation edits/previews.
Selection/marquee, mixed movement, resize, rotation, opacity and delete use the
existing interaction adapter. Frames retain their border/label precedence;
flat groups deliberately remain image-only with an explicit mixed-group error.
Image crop, filtering and flips retain their image-specific semantics.

Shape hits use transformed local geometry, capsules/triangles and hollow edges,
with six logical pixels of pick tolerance. Ellipse edge distance uses a bounded
closest-point solve (48 CPU bisections, 32 shader bisections, closed major-axis
case), including thick, very eccentric ellipses. Ellipse marquee uses a 64-segment
contour approximation, an explicit precision limit for extreme eccentricity.

Scribble captures at most 4096 points; further movement replaces the latest
endpoint and exposes a coarse-tail warning. Iterative deterministic RDP runs once
on completion; cancel commits nothing. The measured 1000-point sine stroke at
normalized tolerance .002 keeps 76 points. No pressure/brush engine is added.

Note editing appends scalar-safe keyboard/IME text, Backspace removes one scalar,
Enter inserts newline, Ctrl+A selects the whole value for replacement,
Ctrl+Enter commits and Escape cancels. Double-click/F2 edits a selected note.
There is no caret timer, navigation editor, rich text, clipboard paste service,
bidi or complex shaping. Fixed scalar wrapping and vertical clipping keep the
box deterministic; increasing font size can clip text until the box is resized.
Text support and its measured costs are in [the font decision](text_rendering_decision.md).

C cycles eight colors, F toggles fill, brackets adjust width, Shift+brackets
adjust opacity; Ctrl+Shift+comma/period adjust note size, Ctrl+Shift+E alignment.
These semantic actions, existing bitmap tool labels and selection handles form
the compact style path. Chrome remains pixel-native; content is antialiased.

## Common renderer and bounds

The CPU scene and GPU annotation pipeline allocate lazily. Cull before layout or
segment generation; build at most 32,768 primitives. Transient creation/edit data
receives budget first and draws last; selected durable objects receive priority,
then common document order is reconstructed across images and annotations.
An object that exceeds the budget is omitted whole, counted and reported in the
title. No durable data is lost. Tests saturate the scene while preserving previews.

CPU primitives are 176 bytes (maximum 5.5 MiB), reusable GPU instances 128 bytes,
initial capacity 128, doubling to at most 32,768. CPU instance staging and GPU
instances each cap at 4 MiB; order/range storage has separate bounded costs.
The existing image upload counter measures image uploads only: annotations add
at most 4 MiB instance upload per rendered frame and a one-time 1,867,776-byte
R8 atlas upload when the first primary note glyph is visible. Unused annotation
GPU buffers/atlas are zero. There is no renderer filesystem/source access.

Rect/ellipse compose stroke and fill before applying global opacity once.
Adjacent scribble capsules and arrow head/shaft use joined-alpha coverage to
avoid doubled opacity at their shared joints. Nonadjacent self-crossing scribble
segments retain ordinary overdraw and may look darker: this is a simple bounded
polyline, not an offscreen whole-object compositing engine. Pixel readback tests
check ordered draw, smooth coverage, global alpha, joints and ellipse geometry;
8-bit compositing comparisons allow one least-significant channel unit.

## Source actions

Ctrl+Shift+O Open, Ctrl+Alt+O Reveal, Ctrl+Shift+C Copy operate on one selected
linked image's resident source descriptor. Embedded/foreign/missing sources give
explicit errors. Relative paths use the lexical absolute board parent, matching
ProductAssets even when the board filename is a symlink, then the actual target
is canonicalized. Linked paths may legitimately be outside the board directory;
no annotation text becomes a path or command. Only regular PNG/JPEG targets with
matching extension/signature are admitted, Unix executable mode is refused.

One explicit action starts one named worker; no source reads or process launch
occur on the event/render thread. Fixed `gio`/Explorer, `xclip`/`wl-copy`/Windows
`clip` executables receive distinct absolute arguments; never a shell command.
Copy is bounded to 4096 bytes and reports unsupported/missing helpers. Helper
execution is polled only during the action and killed/waited after ten seconds;
filesystem canonicalization/open follows normal OS blocking semantics. A hung
filesystem is not claimed to be bounded by that helper deadline. Only one source
action is active. Completion/errors use the existing event proxy, no idle worker.

## Verification and retained limits

See [Mission 1E](../MISSION_1E_REPORT.md) and tracked Phase 1E receipts for native
X11/DPI2 authoring, schema mutation tests, exact history/save/reopen, shape/text/
stroke scale, eccentric/full-screen ellipses, prior image/spatial/product suites,
idle observations and fixed binary costs. Windows native feel/tablets remain
unmeasured; Linux and Windows compile/tests are checked in configured CI.
