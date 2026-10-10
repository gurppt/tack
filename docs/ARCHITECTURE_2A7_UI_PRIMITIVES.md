# Phase 2A7: small reusable UI primitives

The canvas renderer and semantic document editor remain the only rendering and
mutation paths. This phase adds no GUI toolkit, dependency, permanent worker,
service or network endpoint.

`ui_scroll::Scrollbar` stores bounded row/viewport/thumb geometry and one captured
drag. Wheel events move a small integral number of rows; motion alone never
scrolls. Keyboard focus requests minimal reveal. Keymap, Preferences, both
Toolbar columns and context lists share this primitive. Thumb geometry is painted
after row backgrounds; it has no inertial animation or recurring deadline.

`modal_shell::ModalShell` provides integer frame geometry, clipping, translation
and outside-dismiss policy. Short dialogs center on every draw, including resize.
Work panels anchor at the top-left. About uses the same shell with its own compact
body. Unsaved/recovery/connecting dialogs require explicit action; ordinary
information, errors and camera capture can dismiss outside.

Keymap rows share one selected row and Action / Shortcut / Behavior column focus
for mouse and keyboard input. Normal, Hold and Release are mutually exclusive.
`shortcut_capture::Capture` validates a candidate before changing live bindings,
shows displaced actions and requires Enter/Confirm. Escape discards the candidate.
Bookmark capture uses the same path, including pointer bindings and dynamic slots
0–63. Losing focus dismisses bookmark capture. Explicit Right Mouse bindings
precede the default context popup when no popup already owns input.

Keyset bindings and identity are one unit for profile merge/adoption. A completed
export acknowledges its snapshot without erasing newer bindings; newer edits
retain the dirty marker. General Preferences export excludes keyset identity and
bindings. Default has no writable file; Save becomes Save As. Disk work remains
on the existing bounded local worker.

Temporary pointer tools receive their initiating press after Hold activation and
their end before restoration. Focus cancellation restores the base. Opening a
Note draft releases captured input and restores the previous base of a Hold;
ordinary one-shot Note creation finishes to Pointer. Shift+corner scales a Note,
while normal corners and all side handles change its box without text scaling.

Contextual adjustment and aspect reset emit existing bounded semantic command
batches. They share local history and the LAN serialization/authority boundary.
Fill cycles OFF → 100 → 75 → 50 → 25 → OFF; Color changes stroke and fill hue,
retaining alpha. Fill context is selected explicitly by Fill and cleared by other
style actions/new gestures. Image/Frame adjustments report a no-op. Text-size
buttons snap to 16-pixel bitmap steps within 16–256; existing imported/scaled sizes
remain intact until explicitly changed. Note paper uses one flat primitive and at
most six palette-derived ruled lines, independent of text length; no texture or
additional document objects.

About portrait and the supplied 73×33 transparent logo load only on demand. Two
bounded textures share the existing image pipeline, nearest sampler and canvas
pass. Both textures are released on close. Runtime prefers the owner's executable-
relative logo; packages include an exact fallback copy. Builds still install only
missing toolbar icons, preserving editable runtime artwork.

Transient feedback uses the existing cached status strip, also temporarily visible
when its preference is hidden. It reserves toolbar space while shown and clears
on the next action. It adds no acknowledgement timer. The existing caret deadline
is active only during text editing.
