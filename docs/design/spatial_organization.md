# Phase 1D — spatial organization

## Execution plan

1. Extend existing core commands with flat groups and named frame objects; test reference validation, rollback and bounded history.
2. Extend storage only for those records, retaining schema 1 output for image-only ungrouped documents and reading old files unchanged.
3. Add allocation-free deterministic snapping and atomic world-axis layout actions; integrate frame/group selection with the existing gesture controller.
4. Add procedural GPU dots and crisp bounded pixel overlays; expose semantic shortcuts and a small frame-name editor.
5. Exercise native X11, persistence, idle, 1k/5k/10k snapping, layout/groups/frames, and existing product/renderer regressions on a frozen binary.
6. Run independent architecture/performance/parser verification, fix findings, complete quality gates and Linux/Windows CI, report and commit/push. Stop before 1E.

## Model and persistence

Frames reuse ObjectId, DocumentObject and Transform. They are axis-aligned,
unflipped rectangles with UTF-8 names bounded to 256 bytes, nonempty and without
control characters. Spatial containment never implies membership or ownership.
Flat groups use GroupId and sorted unique image ObjectIds; at least two members,
one group per object, no frames/groups as members, no nesting. An inverse lookup
is allocated only for actual members. Groups have no transforms: gestures edit
members in one atomic Batch. Deleting a group selection explicitly ungroups then
deletes its images in one Batch; direct removal of a grouped object is rejected.
Ungroup preserves geometry and order. Alignment treats a selected group as a unit.

Schema 2 adds the implemented frame kind and group records. Schema 1 remains
readable and is written when no spatial records exist. Unknown versions, kinds,
fields and invalid references are refused before editable state is published.
Snap settings, guides, selection, name-edit previews and camera are transient.

## Geometry and interactions

Grid spacing is 64 world units times a power of two chosen to keep dots at least
24 logical pixels apart. Origin remains world (0,0); camera-relative modulo is
computed in f64, avoiding large-coordinate f32 precision loss. Dot pixels are
integer-aligned; DPI controls size/spacing. Hidden grid issues no draw or upload,
and creates no grid resource until first use.

Snap tolerance is 6 logical pixels (DPI/zoom converted to world units). Candidate
scan uses resident metadata, excludes selected objects, and compares selection
AABB edges/centers with object/frame AABB edges/centers. Small deterministic
ties prefer objects then stable ObjectId/anchor order; grid is a fallback.
An existing axis latch remains stable within 1.5 times the threshold; exact ties
retain the first stable candidate. Uniform corners also enforce the final 6-pixel
bound below. Each update starts from gesture-start geometry, never the previous snapped
preview. Guides have at most two axes, vanish on bypass/cancel/commit. Move and
axis-aligned single-object resize snap. Uniform corner snapping admits only targets
whose final corner displacement is at most 6 logical pixels after projection,
including extreme aspect ratios; it keeps one winning axis and its guide.
Rotated resizing retains its geometric
constraint and is documented separately. X is semantic held SnapDisable.

Middle/Alt pan takes precedence; handles precede hit selection; images precede
frame border/title hit regions; frame interiors do not steal image or marquee
selection. Group member hits/marquee expand to complete groups. Marquee selects a frame only
when the entire frame is enclosed; interior marquees select images. A frame moves or
resizes by the same preview/commit controller; rotation/crop/image effects do not
apply to frames. Escape/focus loss cancels previews exactly. Frame creation uses
selection bounds plus padding, or a centered viewport rectangle when empty.
F2 edits only a selected frame name; Enter commits once, Escape cancels. Frame
navigation follows existing back-to-front object order, independent of grouping.

Alignment uses the initial selection outer AABB as its reference for unit
edges/centers; the resulting AABB may shrink.
Distribution sorts units by the selected axis, then stable ID, and equalizes
nonnegative gaps. It preserves the outer span when the widths fit; otherwise it
uses zero gaps and expands right/down from the leading edge to avoid overlap.
Row/column pack uses the same order, a 16 world-unit gap and shared leading edge.
All actions validate geometry before one Batch; no z-order or source changes.

## Resources and visual direction

No new dependency, worker, media I/O, polling timer or spatial index was added.
Simple scans must be measured at 1k/5k/10k before considering an index. Object
metadata remains source-independent. Group maps are empty/unallocated when unused.
Frame strings exist only for actual frames; history accounts for owned payloads.
UI primitives use square geometry, integer physical-pixel alignment and tiny
functional palettes; media sampling remains unchanged. Labels use Tack Label Bitmap, converted from pinned OFL-1.1 GNU Unifont 18.0.01
BMP and upper-plane glyphs. Fixed 37-byte records are binary-searched in a
4,256,961-byte read-only table, with no decoder, font allocator, texture or I/O.
Each visible glyph is one quad (six expanded vertices carrying packed rows),
sampled by integer bitmap bits in the existing overlay shader. At most 512
visible glyphs and 2048 total overlay quads are built. Selected decorations and
two snap guides retain priority; crowded frame rendering is deterministically
capped. Initial buffers remain 128 quads and grow only for actual active labels.
The expanded vertex raises initial CPU/GPU overlay capacity by 33,792 bytes each;
expanded quad metadata adds 5,120 bytes of initial CPU capacity. Document gains
48 bytes of empty map/counter state; per-image DocumentObject/Command sizes stay
160/176 bytes. The font is one shared static array to avoid duplicate const promotion.
This fixed increase and the font's executable storage are explicit compromises
for readable Unicode without a new rendering dependency. Unknown codepoints use
U+FFFD; complex-script shaping/bidi is limited in compact labels. The selected
and editing full UTF-8 name is also shown by the native window title. Scalar-safe
editing, byte bounds and IME Commit preserve document text independently of
compact label truncation. `--font-license` embeds copyright/OFL for distribution.
Overlay memory grows only when actual frame labels exceed the existing budget,
with a hard bound and deterministic truncation. No per-object widget hierarchy.

Measurements and any deviations will be recorded in MISSION_1D_REPORT.md.

Native WM can intercept Ctrl+Alt+Shift arrows; Ctrl+Shift+D/V are additional
distribution bindings. They dispatch exactly the same semantic actions.
