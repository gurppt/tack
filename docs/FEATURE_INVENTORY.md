# Current feature inventory — Phase 2A8

Code audit of the local prototype on 2026-10-10, based on `Action::ALL`,
`product_bindings`, `ImageInput`, annotation/spatial input, local file workers,
storage/recovery and tests. This inventories shipped and reachable code, not a roadmap.
Historical Phase 1I evidence has 170 automated native assertions on US and French XKB layouts at
800×600 and 1024×768. Owner comfort/discovery review and real Windows desktop
validation remain pending; “implemented” does not imply human acceptance.

Every semantic catalog entry is listed below (104 actions). Shortcuts are the
**default logical, layout-aware keyboard bindings** exported by `context_ui`; the actual menu
reads the active user keymap. An em dash means no default press shortcut.
All actions can be inspected/remapped in Tack → Edit → Keymap. This exposure
does not make experimental tool modes fully operational. Version-2 imports may
explicitly bind physical positions; labels show `pos:Code(KeyZ)` for those.
Logical characters are base characters before Shift/AltGr text composition.
Punctuation defaults ([ ], Shift+[ ], Ctrl+Shift+.) may require remapping or menu
invocation on layouts without those base characters. Their listed labels describe
the configured binding, not universal keyboard availability. The native matrix
covers the targeted letter/named commands, not every punctuation default.
Version-1 advertised key names migrate to logical keys; unsupported legacy
positions fail safely without overwriting the profile. See the
[keyboard policy](design/keyboard_shortcuts.md).

`Board` means the committed result is saved in `.tack`; `profile` means local
preferences; `session` means transient state only. “Undo after commit” excludes
selection, tool choice, camera and text drafts. Menu paths are one submenu deep.

| Feature / semantic action | Status | Object type | Invocation / default shortcut | Other UI | Context menu | Undoable? | Persisted? | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Add camera bookmark (`AddCameraBookmark`) | implemented | local camera slots | B | Tack → View → Add bookmark | shared Tack menu | No | Local profile | Centered shortcut capture, Enter confirms collision reassignment, Escape cancels; at most 64 local views, works on shared boards without a shared revision. |
| Camera bookmarks (`CameraBookmarks`) | implemented | camera / bookmark metadata | — | Tack → View → Camera bookmarks | shared Tack menu | jump No; rename/delete Yes | Board metadata, camera session | Enter/click jumps; F2/visible Rename and Delete buttons. Shared snapshots allow jump, not editing. |
| Duplicate selection (`DuplicateSelection`) | implemented | images, notes, all supported annotations, frames | Ctrl+D | Tack → Edit | shared Tack menu | Yes, one transaction | Board | Fresh object/group IDs, same image assets/sources/originals; one adaptive grid-step offset. Shared metadata-only through authority, with conflict/lease checks. |
| Image information (`SourceInfo`) | implemented | one selected image | — | Image → Source | Image information | No | temporary panel | Existing metadata and observed status only; no filesystem access, codec probing or decode requests. |
| Join shared board (`JoinSharedBoard`) | implemented | connection descriptor | — | Tack → File | shared Tack menu | No | another native window | Primary Paste invite; canonical tack://IP:PORT/ID or compatible numeric IP:PORT + Board ID; IPv6 brackets; explicit cancellable launch reuses CLI join. Current local board retained. |
| Copy Invite (`CopySharedBoardAddress`) | implemented | joined board | — | Tack → File | shared Tack menu | No | clipboard | Canonical URI; off-thread existing native clipboard helper. Disabled for local boards. |
| Share Board (`ShareBoard`) | implemented | local/shared incarnation | — | Tack → File / toolbar | shared Tack menu | No | independent shared file + companion | Start Sharing owns existing server process, local original remains independent; same-profile Put Online reuses Board ID. |
| Stop Sharing (`StopSharing`) | implemented | owned hosted board | — | Tack → File / online panel | shared Tack menu | No | checked offline snapshot | Enabled only for owned host; checkpoint/reap outside UI, offline read-only. |
| Toggle Toolbar (`ToggleToolbar`) | implemented | chrome | — | Tack → View / keymap | shared Tack menu | No | profile | One toolbar, six placements, hidden has no icon draw. |
| Edit Toolbar (`EditToolbar`) | implemented | chrome | — | Tack → View / keymap | shared Tack menu | No | profile | Tools → Edit Toolbar; two independent wheel/thumb lists; Add/Remove, reorder, Reset, 1×/2×/3×; proportional edge anchor, ≤32 semantic IDs. |
| Toggle Status Bar (`ToggleStatusBar`) | implemented | chrome | — | Tack → View / keymap | shared Tack menu | No | profile | One cached hover/shortcut/state line; fixed shared indicator also visible without strip. |
| Pointer tool (`SelectTool(Pointer)`) | implemented | selection | V | Tack → Tools | No | No | session | Direct canvas selection/manipulation. |
| Pan tool (`SelectTool(Pan)`) | implemented | camera | — | toolbar / keymap | No | No | session | Left drag uses existing camera pan, no document mutation. |
| Rotate view tool (`SelectTool(RotateView)`) | experimental | tool state | — | keymap catalog only | No | No | session | Generic Interaction state exists; native pointer pipeline does not implement a complete dedicated pan/rotate-view tool. |
| Temporary pointer tool (`TemporaryTool(Pointer)`) | experimental | tool state | — | keymap hold binding only | No | No | session | Generic held-tool restoration tested; no default binding or menu; complete native tool combinations not validated. |
| Temporary pan tool (`TemporaryTool(Pan)`) | implemented | tool state | — | keymap catalog only | No | No | session | Mouse or keyboard Hold begins temporary Pan, uses the initiating pointer press, and restores the previous tool on release/focus loss. |
| Temporary rotate view tool (`TemporaryTool(RotateView)`) | experimental | tool state | — | keymap catalog only | No | No | session | Generic Interaction state exists; native pointer pipeline does not implement a complete dedicated pan/rotate-view tool. |
| Undo (`Undo`) | implemented | document | Ctrl+Z | Tack → Edit | all object menus | history operation | Board result; history session | Text drafts have no local undo buffer; Ctrl+Z is consumed while editing. |
| Redo (`Redo`) | implemented | document | Ctrl+Shift+Z | Tack → Edit | all object menus | history operation | Board result; history session | Ctrl+Y is an additional Redo alias. |
| Pan view (`PanView`) | implemented | camera | Middle drag / Alt+Shift+left drag | canvas gesture / handles | No | No | session | Alt+left pans away from handles. |
| Zoom view (`ZoomView`) | implemented | camera | wheel | canvas gesture / handles | No | No | session | Cursor-centred zoom; no document command. |
| Select and manipulate image (`ImagePointer`) | implemented | all object kinds | left click / drag; resize handles | canvas gesture / handles | No | Yes after drag | Board result | Click selects; empty drag is marquee; move/resize share gesture history. |
| Toggle image selection (`ToggleSelection`) | implemented | selection | Shift+left click / marquee | canvas gesture / handles | No | Yes if manipulating | Board result; selection session | Group members selected as an image group. |
| Rotate image (`RotateImage`) | implemented | images / annotations | Ctrl+left drag; Ctrl+Shift constrains | canvas gesture / handles | No | Yes after drag | Board | Rotation handle is another route; frames remain axis aligned. |
| Scale image (`ScaleImage`) | implemented | selection | Ctrl+Alt+left drag | canvas gesture / handles | No | Yes after drag | Board | Uniform scaling; frames keep their core limits. |
| Adjust image opacity (`AdjustOpacity`) | implemented | images | Ctrl+Alt+Shift+left drag | canvas gesture / handles | No | Yes after drag | Board | Continuous alpha preview is a single transaction. |
| Pan or resize handle about center (`CenterPointer`) | implemented | camera / selection | Alt+left drag | canvas gesture / handles | No | Yes for centred handle resize | Board result; camera session | Pans away from a handle; resize around centre on a handle. |
| Cancel interaction (`CancelInteraction`) | implemented | interaction | Esc (any modifiers) | canvas / draft | No | No | session | Cancels preview/draft and resets Pointer; while a popup is active Escape dismisses that popup. |
| Crop gizmo (`CropMode`) | implemented | one image | Ctrl+Shift+Alt+C | crop handles | image | Yes after crop drag | Board crop; mode session | Toggles crop gizmo; the crop drag, not the mode toggle, is undoable. |
| Select all (`SelectAll`) | implemented | all object kinds | Ctrl+A | Tack → Edit | canvas | No | session | Selection only; no document mutation. |
| Delete selection (`DeleteSelection`) | implemented | all object kinds | Del | Tack → Edit | all object menus | Yes | Board | Atomic batch; removes corresponding group membership. |
| Flip horizontal (`FlipHorizontal`) | implemented | images / Rectangle / Line / Arrow / Scribble | Shift+Alt+H | keyboard / keymap | image | Yes | Board | Works through the shared dispatcher; Frame, Note and Text are excluded. |
| Flip vertical (`FlipVertical`) | implemented | images / Rectangle / Line / Arrow / Scribble | Shift+Alt+V | keyboard / keymap | image | Yes | Board | Works through the shared dispatcher; Frame, Note and Text are excluded. |
| Bring forward (`Order(Forward)`) | implemented | all object kinds | — | keyboard / keymap | all object menus → Order | Yes | Board | New UI adapter over existing SetZOrder; selected relative order retained. Frame chrome stacks above image content. |
| Bring to front (`Order(Front)`) | implemented | all object kinds | — | keyboard / keymap | all object menus → Order | Yes | Board | New UI adapter over existing SetZOrder; selected relative order retained. Frame chrome stacks above image content. |
| Send backward (`Order(Backward)`) | implemented | all object kinds | — | keyboard / keymap | all object menus → Order | Yes | Board | New UI adapter over existing SetZOrder; selected relative order retained. Frame chrome stacks above image content. |
| Send to back (`Order(Back)`) | implemented | all object kinds | — | keyboard / keymap | all object menus → Order | Yes | Board | New UI adapter over existing SetZOrder; selected relative order retained. Frame chrome stacks above image content. |
| Opacity 100% (`Opacity(Full)`) | implemented | images | — | keyboard / keymap | image → Opacity | Yes | Board | New UI preset adapter over existing SetOpacity; no second history. |
| Opacity 75% (`Opacity(ThreeQuarters)`) | implemented | images | — | keyboard / keymap | image → Opacity | Yes | Board | New UI preset adapter over existing SetOpacity; no second history. |
| Opacity 50% (`Opacity(Half)`) | implemented | images | — | keyboard / keymap | image → Opacity | Yes | Board | New UI preset adapter over existing SetOpacity; no second history. |
| Opacity 25% (`Opacity(Quarter)`) | implemented | images | — | keyboard / keymap | image → Opacity | Yes | Board | New UI preset adapter over existing SetOpacity; no second history. |
| Default sampling (`Filtering(Default)`) | implemented | images | — | keyboard / keymap | image → Sampling | Yes | Board | Default follows current default sampling; Smooth and Nearest remain independent of representation LOD. |
| Smooth sampling (`Filtering(Smooth)`) | implemented | images | — | keyboard / keymap | image → Sampling | Yes | Board | Default follows current default sampling; Smooth and Nearest remain independent of representation LOD. |
| Nearest sampling (`Filtering(Nearest)`) | implemented | images | — | keyboard / keymap | image → Sampling | Yes | Board | Default follows current default sampling; Smooth and Nearest remain independent of representation LOD. |
| Cycle image sampling (`CycleFiltering`) | implemented | images | Alt+T | keyboard / keymap | Sampling choices instead of cycle | Yes | Board | Default → Smooth → Nearest; same existing filtering command. |
| Save (`Save`) | implemented | board | Ctrl+S | Tack → File | No | No | file | Atomic asynchronous save; untitled goes through Save As. |
| Toggle dotted grid (`ToggleGrid`) | implemented | view | G | Tack → View / Preferences default | No | No | session | Session toggle; persistent initial value is changed in Preferences. |
| Toggle snapping (`ToggleSnapping`) | implemented | view | Shift+G | Tack → View | No | No | session | Bounded axis/grid snapping and guides; no document edit by itself. |
| Temporarily disable snapping (`SnapDisable`) | implemented | snapping | hold X | canvas gesture / handles | No | No | session | Temporarily suppresses current snapping. |
| Arrange in Grid (`Layout(Grid)`) | implemented | independent selection units | — | keymap catalog | multiple → Arrange (first row) | Yes, one batch | Board | Repositions AABB units into deterministic rows/columns, 16 world-unit gaps, preserves size/rotation/group internals. At least two units. |
| Snap Selection to Grid (`Layout(SnapToGrid)`) | implemented | independent selection units | — | keymap catalog | multiple → Arrange (second row) | Yes, one batch | Board | Each unit AABB top-left snaps independently to the base 64 world-unit lattice; single unit allowed via remap. No overlap prevention or adaptive-dot spacing. |
| Align left (`Layout(Left)`) | implemented | independent selection units | Ctrl+Left | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Align horizontal center (`Layout(HorizontalCenter)`) | implemented | independent selection units | Ctrl+Shift+Left | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Align right (`Layout(Right)`) | implemented | independent selection units | Ctrl+Right | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Align top (`Layout(Top)`) | implemented | independent selection units | Ctrl+Up | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Align vertical center (`Layout(VerticalCenter)`) | implemented | independent selection units | Ctrl+Shift+Up | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Align bottom (`Layout(Bottom)`) | implemented | independent selection units | Ctrl+Down | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Distribute horizontally (`Layout(DistributeHorizontal)`) | implemented | independent selection units | Ctrl+Shift+Alt+Up | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Distribute vertically (`Layout(DistributeVertical)`) | implemented | independent selection units | Ctrl+Shift+Alt+Down | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Pack horizontally (`Layout(PackHorizontal)`) | implemented | independent selection units | Ctrl+P | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Pack vertically (`Layout(PackVertical)`) | implemented | independent selection units | Ctrl+Shift+P | keyboard / keymap | multiple → Arrange | Yes | Board | Image groups act as units; frames and annotations participate. Ctrl+Shift+D/V also alias the two distribute actions. |
| Group selection (`GroupSelection`) | implemented | image groups | Ctrl+G | keyboard / keymap | image / multiple | Yes | Board | Groups are images only; mixed selections cannot group. Ungroup removes touched image groups. |
| Ungroup selection (`UngroupSelection`) | implemented | image groups | Ctrl+Shift+G | keyboard / keymap | image / multiple | Yes | Board | Groups are images only; mixed selections cannot group. Ungroup removes touched image groups. |
| Create frame (`CreateFrame`) | implemented | frame | Ctrl+Shift+F | Tack → Tools | canvas | Yes | Board | Returns Pointer after creation. Selection bounds +32 or 60% viewport; frame is a labelled region; optional flat translation-only children are managed by Link/Unlink. |
| Rename frame (`RenameFrame`) | implemented | frame / note | F2 | inline text editor | frame → Rename / note → Edit | Yes after commit | Board | F2 and double click edit existing text; frame Enter / note Ctrl+Enter commit; Escape discards. |
| Focus selected frame (`FocusFrame`) | implemented | frame / camera | Space | Tack → View | frame → Focus | No | session | Camera/selection navigation; unavailable if no applicable frame. |
| Focus next frame (`NextFrame`) | implemented | frame / camera | PgDn | Tack → View | No | No | session | Camera/selection navigation; unavailable if no applicable frame. |
| Focus previous frame (`PreviousFrame`) | implemented | frame / camera | PgUp | Tack → View | No | No | session | Camera/selection navigation; unavailable if no applicable frame. |
| Text (`SelectTool(Text)`) | implemented | note | T | Tack → Tools | canvas → New note | No | session | Click/drag, release, then type and Ctrl+Enter. One-shot: Pointer base restored on draft entry and after commit/cancel; no local text Undo. |
| Rectangle (`SelectTool(Rectangle)`) | implemented | annotation | R | Tack → Tools | No | No | session | Click/drag with selected tool; release commits once and returns Pointer. Escape/focus cancellation adds no history. |
| Line (`SelectTool(Line)`) | implemented | annotation | L | Tack → Tools | No | No | session | Click/drag with selected tool; release commits once and returns Pointer. Escape/focus cancellation adds no history. |
| Arrow (`SelectTool(Arrow)`) | implemented | annotation | A | Tack → Tools | No | No | session | Click/drag with selected tool; release commits once and returns Pointer. Escape/focus cancellation adds no history. |
| Scribble (`SelectTool(Scribble)`) | implemented | annotation | P | Tack → Tools | No | No | session | Click/drag with selected tool; release finishes a stroke; Enter/tool change/save/menu finishes the compound object. No idle timeout. Escape cancels; navigation retains the draft. |
| Text (`TemporaryTool(Text)`) | experimental | tool state | — | keymap hold binding only | No | No | session | Generic held-tool restoration tested; no default binding or menu; complete native tool combinations not validated. |
| Rectangle (`TemporaryTool(Rectangle)`) | experimental | tool state | — | keymap hold binding only | No | No | session | Generic held-tool restoration tested; no default binding or menu; complete native tool combinations not validated. |
| Line (`TemporaryTool(Line)`) | experimental | tool state | — | keymap hold binding only | No | No | session | Generic held-tool restoration tested; no default binding or menu; complete native tool combinations not validated. |
| Arrow (`TemporaryTool(Arrow)`) | experimental | tool state | — | keymap hold binding only | No | No | session | Generic held-tool restoration tested; no default binding or menu; complete native tool combinations not validated. |
| Scribble (`TemporaryTool(Scribble)`) | experimental | tool state | — | keymap hold binding only | No | No | session | Generic held-tool restoration tested; no default binding or menu; complete native tool combinations not validated. |
| Cycle annotation color (`AnnotationStyle(Color)`) | implemented | notes / annotations | C | keyboard / keymap | note or annotation → Text / Style | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Toggle annotation fill (`AnnotationStyle(Fill)`) | implemented | rectangle | F | keyboard / keymap | annotation → Style | Yes when applied to committed objects | Board result; creation default session | Fill disabled for line/arrow/freehand; those render strokes only. |
| Increase stroke width (`AnnotationStyle(Wider)`) | implemented | notes / annotations | ] | keyboard / keymap | note or annotation → Text / Style | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Decrease stroke width (`AnnotationStyle(Narrower)`) | implemented | notes / annotations | [ | keyboard / keymap | note or annotation → Text / Style | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Increase note size (`AnnotationStyle(LargerText)`) | implemented | notes | Ctrl+Shift+. | keyboard / keymap | note → Text | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Decrease note size (`AnnotationStyle(SmallerText)`) | implemented | notes | Ctrl+Shift+, | keyboard / keymap | note → Text | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Increase annotation opacity (`AnnotationStyle(OpacityUp)`) | implemented | notes / annotations | Shift+] | keyboard / keymap | note or annotation → Text / Style | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Decrease annotation opacity (`AnnotationStyle(OpacityDown)`) | implemented | notes / annotations | Shift+[ | keyboard / keymap | note or annotation → Text / Style | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Cycle note alignment (`AnnotationStyle(AlignText)`) | implemented | notes | Ctrl+Shift+E | keyboard / keymap | note → Text | Yes when applied to committed objects | Board result; creation default session | Bounded palette/style cycles; defaults also affect later creation. |
| Open linked image source (`OpenSource`) | implemented | one native linked image | Ctrl+Shift+O | platform helper | image → Source | No | session | Explicit local OS helper; unavailable for embedded/foreign paths. Native path passed as a single argument. |
| Reveal linked image source (`RevealSource`) | implemented | one native linked image | Ctrl+Alt+O | platform helper | image → Source | No | session | Explicit local OS helper; unavailable for embedded/foreign paths. Native path passed as a single argument. |
| Copy linked source path (`CopySourcePath`) | implemented | one native linked image | Ctrl+Shift+C | platform helper | image → Source | No | session | Explicit local OS helper; unavailable for embedded/foreign paths. Native path passed as a single argument. |
| Save original as (`SaveOriginalAs`) | implemented | one linked or embedded image | — | keymap catalog | image → Source | No | new encoded file | Native picker; worker copies/extracts original bytes with a 128 KiB buffer, cancellation and atomic publication; no transform or re-encoding. |
| Local menu (`ApplicationMenu`) | implemented | window | F10 or right-click | shared Tack block | every context root | No | session | Shared File/Edit/View/Tools/Preferences/Keymap block; no permanent Tack button or menu bar. |
| New board (`NewBoard`) | implemented | board | Ctrl+N | Tack → File | No | No | new file/session | Starts a separate local window. |
| Open board (`OpenBoard`) | implemented | board | Ctrl+O | Tack → File | No | No | file | Native picker; reuses only the initial clean empty Untitled window; meaningful or dirty boards open separately. Last successful board folder is remembered. |
| Import images (`ImportImages`) | implemented | images | Ctrl+I | Tack → File | canvas | Yes, per admitted image | Board | PNG/JPEG; native picker, bounded asynchronous import. |
| Save As (`SaveAs`) | implemented | board | Ctrl+Shift+S | Tack → File | No | No | file | Native picker; forces a case-insensitive `.tack` suffix, appending after arbitrary dotted names; new document identity; preserves original authority. |
| Paste (`Paste`) | implemented | clipboard | Ctrl+V | Tack → Edit | canvas | Yes after admission/commit | Board | PNG, plain text or file URI list; note draft paste does not commit immediately. |
| Relink selected source (`RelinkSource`) | implemented | image source | Ctrl+Shift+R | native picker | image | Yes | Board | Can replace embedded, missing or changed source; shared-source revision semantics preserved. |
| Preferences (`Preferences`) | implemented | profile | Ctrl+, | Tack root | No | No | profile | Compact temporary panel. |
| Edit keymap (`KeymapEditor`) | implemented | keymap | — | Tack → Edit / Preferences | No | No | profile | Search, remap, unassign and reset; no permanently visible control. |
| Recent boards (`RecentBoards`) | implemented | board paths | — | Tack → File | No | No | profile | Bounded 16 paths; selecting opens another window. |
| Import keymap (`ImportKeymap`) | implemented | profile | — | Preferences / Keymap | No | No | profile | `.tackey` exposed first; compatible legacy JSON accepted; unsupported/corrupt files rejected. |
| Export keymap (`ExportKeymap`) | implemented | profile | — | Preferences / Keymap | No | No | profile | Exports preferences and keymap together as `.tackey`; refuses an existing destination introduced by suffix adjustment. |

Phase 1J adds 27 automated native clipboard/menu/scale/theme checks, actual
Dolphin single/multiple-image copying and owned-window screenshot paste on
Linux X11/NVIDIA Vulkan. Owner aesthetics and real Windows/Wayland clipboard
runtime remain pending. See [Phase 1J](MISSION_1J_REPORT.md).

## Features outside the semantic action catalog

| Feature | Status | Type / invocation / shortcut | UI and context exposure | Undoable? | Persisted? | Limits / notes |
| --- | --- | --- | --- | --- | --- | --- |
| Resize corners/edges | implemented | selection; left drag handles, Shift constrain, Alt centred | canvas handles / platform cursors | one release = one Undo | Board | Pixel geometry shared with hit testing; no per-motion command. Note corners resize the wrapping box; Shift corners uniformly scale box/font/style. |
| Rotate handle | implemented | images / annotations; left drag offset handle | canvas handle / crosshair | one release = one Undo | Board | Axis-aligned frames have no rotation handle. |
| Marquee selection | implemented | empty-canvas left drag; Shift adds | outline | No | session | Includes annotations and groups under current hit rules. Every visible selected member gets its own derived outline, up to the existing 10,000-visible-member limit. |
| Double-click text | implemented | existing frame or note; double left click | inline editor; Edit/Rename context equivalent | after commit | Board | Bounded plain text, no rich text / local text history. |
| Note input / IME | partial | text editor, typing / IME / paste; Ctrl+A, Backspace, Delete; Ctrl+Enter; Escape | inline draft | document Undo after commit | Board | Bounded 16 KiB; whole-text replacement/deletion model; no arbitrary rich editor/caret navigation/local Ctrl+Z buffer. |
| Clipboard PNG/JPEG | implemented | Ctrl+V, canvas Paste, Tack Edit Paste | native clipboard worker | admitted image | Board | PNG/JPEG payload is embedded regardless of default import mode; 64 MiB encoded cap. X11 xclip or Wayland wl-paste; actionable missing-helper errors. |
| Clipboard text | implemented | Ctrl+V, canvas Paste | creates plain note, or paste into active draft | after admission/commit | Board | Creates one undoable note; active editor paste retains document undo focus rules. 16 KiB note cap; slash-leading prose stays text. |
| Clipboard file URI list | implemented | Ctrl+V, canvas Paste | bounded local import | per admitted image | Board | Local URI lists/gnome-copied-files take priority over PNG/JPEG and text. Valid entries survive remote/invalid entries with one rejection summary; 64 KiB / 4096 references. Existing absolute image-path text remains supported; no remote downloads. |
| Drag/drop image files | implemented | OS drop into canvas | native window | per admitted image | Board | Bounded batch/debounce; Escape cancels remaining admission. |
| Linked / embedded import | implemented | Preferences → Import; picker/drop; CLI create | temporary preferences | import undo | Board and profile default | Original authority and shared sources preserved. |
| Default sampling | implemented | Preferences → Image sampling | temporary preferences | No for default; image override Yes | profile / image override Board | Renderer samples Default independently from stored representation. |
| Native DPI / toolbar scale / handle size / hit radius | implemented | Preferences / Edit Toolbar | handles/radius bounded -/+; toolbar 1×/2×/3× | No | profile | Manual global UI Scale removed. UI follows system DPI; legacy profile scale is ignored. Toolbar scale stays independent. |
| Background theme | implemented | Preferences → Background | three direct choices | No | profile only | Very Dark / Neutral Gray / Light, flat fill in existing grid pass. Bitmap UI and immutable cyan/magenta/yellow palette; no board/schema changes. Old profiles default Neutral Gray; unknown future themes rejected without overwriting profile. |
| Keymap search/capture | implemented | Tack → Edit → Keymap; type, Enter, F6; Delete; F5 / Shift+F5 / Ctrl+F5 | temporary panel | No | profile | Press/release/hold/wheel, atomic capture/conflict refusal with requested shortcut and owning action; mouse Change/Unassign/Reset; confirmed category/all resets. Search also matches active shortcut labels; Escape clears search before closing. Menu labels follow actual configured press bindings. Nested Escape returns to Preferences; direct entry closes after search/capture cancellation. |
| About Tack | implemented | right-click Tack / F10 | temporary flat bitmap modal | No | `gfx/about.toml` and artwork, compiled into package | Owner text editable; Cargo version checked; compact artwork decoded only on opening, texture released on dismissal; readable links, 800×600 at 1×/2×. |
| Close confirmation | implemented | OS close button / window manager | Save / Discard / Cancel panel | No | normal Save if selected | No new Quit action; closing stays in the existing native lifecycle. |
| Recovery restore/discard | implemented | open board with newer valid recovery | bounded modal; choice required before import | restored document starts dirty | recovery/normal file authority | Normal save remains distinct; document history itself is not serialized. |
| Read-only board lock conflict | implemented | second writer to same board | errors/status; Save As | No | lock file / new identity if Save As | Separate board windows work independently; no collaboration. |
| Failed import/relink/save status | implemented | native operations | title/errors/temporary panel | successful admission only | committed Board only | Drafts remain on failed save; bounded worker/cancellation model retained. |
| Image supply / missing-source fallback | implemented | open/pan/zoom/relink | canvas imagery/status | source edits Yes; display supply No | original/preview Board; details disposable | 1G bounded projected asynchronous 128/512/2048 supply; see 1G report for limits. |
| Save and reopen | implemented | File menu / Ctrl+S / Ctrl+Shift+S / Ctrl+O / recent list | shared right-click Tack block / F10 + native pickers | Save itself No | .tack | Commands serialize committed geometry/crop/style/order/groups; selection/tools/history are transient. |
| Context resolution / dismissal | implemented | right click object/blank; Esc, outside click, activation | shared Tack block + six specific context kinds | No; command Yes where listed | session | Preserves selected multi-selection/group; clicks outside dismiss without starting a drag. No timers/animations. |
| Menu keyboard navigation | implemented | Up/Down, Left/Right, Enter, Escape, wheel | popup only | No; command Yes | session | One child submenu, no timed hover; disabled rows remain visible. Headings skipped by keyboard; 800×600 2× bounded adjacent columns keep parent accessible. |
| Contextual cursor feedback | implemented | hover/move/handle/crop/text | native platform cursor | No | session | No cursor asset cache or animation. |
| Multiple local windows | implemented | New/Open create another window | File menu | per document | separate .tack | Resource budgets are per instance; no shared-board synchronization. |
| Developer CLI and benchmark modes | experimental | explicit create/inspect/supply/preparation/benchmark commands and flags | CLI only, absent from authoring menus | depends on CLI operation | generated files/receipts | Explicit diagnostics; not a new authoring feature or a legacy-hardware performance guarantee. |

## Deliberately absent operations

Duplicate, add-selection-to-frame membership, ellipse, rich text, layer panel,
collaboration and online services are not implemented authoring features. There
is no context item pretending to perform them. Ordering/opacity and the two new layout actions adapt existing core commands.
There is one document history and no persistent arrangement engine. Tidy and
equal-size normalization are deferred. See [arrangement policy](design/board_arrangement.md).

Mixed selections expose only supported grouping/arrangement/order/delete paths.
Image-specific sampling/opacity menus are intentionally limited to the single
image menu; the corresponding keyboard/custom actions retain their existing
multi-image semantics. Canvas New note selects the existing Text tool and labels
its click/drag workflow; it does not silently insert a note at an invented size.

Frame order controls frame chrome/hit order amongst frames. Frame borders remain
an overlay above image/annotation content, as before; ordering does not turn
frames into filled layers or change frame membership.

The repository's broader “128 MB / Pentium III” direction is a UI design target.
This inventory makes no claim that current wgpu/native process RSS satisfies it.

One-shot completion clears the existing temporary-tool stack. Modal entry
additionally resets held-input state. Temporary Pan/RotateView remain experimental; there is no
second restoration stack. Nested Pan/Text commit/cancel/focus paths have
owner tests, but complete native experimental combinations are not claimed.

## Phase 1L display supply

Large static noninterlaced PNG sources derive overviews by bounded scanlines.
NORMAL/LARGE/HUGE_TILED classify checked decoded risk, with a 32 MiB monolithic
pixel-working allowance and separate capped parser/row scratch. 16-bit precision
and codec capability affect routing. Large baseline JPEG now uses bounded
scanlines and default requested tiles; dimensions exceeding the ordinary route
select that bounded path. Huge progressive/multiscan JPEG is explicitly refused.

Visible PNG 256-pixel mip tiles remain an **opt-in prototype** (`open BOARD --huge-tiles`).
JPEG tiles are automatic and include a one-pixel Smooth gutter. Their
CPU/GPU/disk residency uses existing budgets;
source/revision identities and cancellation reject obsolete results. Sequential
PNG rescans, nearest-source mip sampling and more visible sources than admitted
tile slots remain explicit limitations. No complete full-resolution frame or
pyramid is constructed. Neighboring JPEG demand coalesces into at most fifteen
tiles and 4 MiB of output per regional job, with a separately checked crop
footprint. Uncached JPEG regions still scan sequentially. A disposable raw tile
cache in the local profile reuses visited JPEG/PNG detail after reopening;
its quota is taken from existing disk budgets (64 MiB default, 4 MiB potato).
Only image workers access that cache; corruption, deletion or a conflicting
lease falls back to the source. A real 50,000 × 50,000 baseline JPEG passes
overview, local detail, pan, opposite-corner, exact return and restart checks.
Potato keeps one worker, four admitted keys, 8 MiB CPU and 16 MiB GPU payload
budgets; this is a modern-host budget test, not a legacy-CPU runtime claim.
Ordinary LOD selection keeps the finest valid
resident, avoids native upscaling, and requests native small Nearest sources
within admission budgets. Opt-in diagnostics and long-churn tests cover actual
resident dimensions and source revisions. See the
[2A1 report](MISSION_2A1_CORE_IMAGE_REPORT.md),
[2A2 report](MISSION_2A2_HUGE_RASTER_STREAMING_REPORT.md) and decoder design note.

Preferences Theme choices stay inside their submenu for live preview;
Escape or Back returns to Preferences. Last successful Open/Save As folder uses
a bounded native path descriptor and worker-only fallback checks.

Implementation, measured limits and validation: [Phase 1L report](MISSION_1L_REPORT.md), [huge-image design](design/huge_images.md), [owner checklist](HUMAN_TEST_1L.md).

Phase 2A3 adds static SVG-derived package/window/About identity without a runtime SVG library. Desktop Publish and shared recents are deferred: current publish consumes a committed file; recent-file records represent native paths. No new startup LAN activity or permanent worker is introduced. See [the 2A3 report](MISSION_2A3_DAILY_USE_POLISH_REPORT.md).

Phase 2A4 scope: editable hard-alpha PNG16 icons and primitive native toolbar/status
are documented in [pixel_toolbar.md](design/pixel_toolbar.md); desktop identity
and server ownership in [shared_desktop_lifecycle.md](design/shared_desktop_lifecycle.md).
Physical two-computer LAN acceptance remains separate from same-host automation.

Phase 2A5 adds scoped object conflicts and bounded five-second active-transform
leases, foreign red pixel outlines, unrelated-menu/gesture preservation, explicit
Save to Local, current-window Share and Close Board. Shared state stays in the
bottom strip; disconnect opens a centered panel and invites are readable/copyable.
These features keep the authoritative server; there is no CRDT or offline merge.

Interaction corrections include 15° rotation snap and temporary Shift snap,
outward Rectangle/Frame selection outlines, three-pixel creation strokes separate
from edited widths, clearer arrowheads, Note Enter/Shift+Enter, annotation selection
filter (F8), shared metadata-only Duplicate, Frame palette colors/wrapped 2× titles,
and semantic z-order defaults. The centered docked toolbar has an explicit saved
offset, remembered visibility placement and two-column editor. Keymap supports
shortcut double-click capture, reassignment that unbinds a collision, Normal/Hold
tool mode, separate keymap/preferences exports and local B → 0–9 camera slots.
Existing custom keymaps retain their bindings; Reset adopts changed defaults.

Frame palette colors require schema 5 only when nondefault colors exist. LAN peers
used protocol major 2 in that phase; Phase 2A8 now requires major 3. Test evidence and remaining physical acceptance are
recorded in [the 2A5 report](MISSION_2A5_CONCURRENT_EDITING_INTERACTION_REPORT.md).
Quadratic curves and Plain Text remain deferred.

Phase 2A7 adds Reset Aspect Ratio (crop-aware, area/center/rotation preserved,
one undo), contextual Numpad +/− adjustments, Rectangle Fill opacity cycling,
palette-aware ruled Notes, and named keyset Save / Save As / Load. Short dialogs
center and ordinary ones dismiss outside; unsaved-data confirmations require an
explicit choice. Keymap, Preferences and Edit Toolbar remain top-left work panels.
Plain Text remains deferred; Note normal/side resize changes wrapping geometry,
Shift+corner scales text and paper together.

Deferred drawing semantics: a minimal quadratic curve uses endpoint A, endpoint
B, one movable control point and stroke/color, followed by confirmation. A later
curved arrow uses the endpoint tangent. No multisegment path, node editor, fills
or boolean geometry. Future Plain Text follows Note geometry: normal corners
and sides change box/wrap only; Shift+corner scales box and text together.


## Phase 2A8 current additions

| Action/capability | Current behavior | Default | Persistence / authority |
| --- | --- | --- | --- |
| Check for Updates | Manual bounded GitHub published-Release check; no polling | Dev channel | Profile; separate non-resident verified portable updater |
| Link to Frame | Existing selection or normal acquisition; pixel dots, target hover and bounded confirmation | Ctrl+L; default toolbar | Flat links, schema 7; local Undo/shared authority |
| Unlink from Frame | Removes selected unit/group relations, retains exact positions | Ctrl+Shift+L | One semantic command |
| Select Linked Objects | Selection only, respects annotation selection lock | Frame context menu | Transient |
| Finish Scribble | Several independent strokes become one local object | Enter | Compound/per-stroke schema 6; protocol major 3 |
| Scribble Eraser | One target, swept segment splits; release commits once | E; Scribble context | SetAnnotation/remove with Undo/shared authority |
| Merge Scribbles | Painter-order strokes and per-stroke styles; common Frame parent required | Multiple-Scribble context | One batch, originals restored by Undo |
| Cursor ownership | 17 cached hard-alpha PNG16 cursors; UI pointer restores canvas tool | Native | Seed only missing personal files |
| Annotation-lock icon | One action, distinct ON/OFF artwork | F8 | Existing action/profile rules |
| Image/group highlight | Per-visible-image outlines, one transformation handle frame | Native | Derived and bounded |
| Local-minimal | Same local document/renderer/storage; network/updater admission and UI omitted | `--no-default-features` | Protocol remains linked: documented architecture exception |
| Timed-media seam | Codec-independent bounded API only; no live player/backend | No codec | No resident worker/process |

The older phase sections above record evolution. [2A8 semantics](SCRIBBLE_FRAME_LINKS.md)
and [alpha checklist](HUMAN_TEST_2A8_ALPHA.md) describe the current compound/link behavior.
Release artifacts identify Cargo version, exact source commit/channel and protocol;
`dev` is integration and `main` holds promoted snapshots. Testers do not need git.
