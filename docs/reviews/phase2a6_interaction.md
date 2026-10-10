# Phase 2A6 independent interaction review

Date: 2026-10-10. Review base: `0e92f45` (2A5 implementation: `688afb9`).
Result: **Source review PASS after corrective findings were resolved.**

This reviewer read the normative 2A6 brief, contributing rules, architecture,
2A5 report, affected source and targeted tests. The review did not compile or
execute Tack, manipulate the owner's desktop/profile, or alter icon files.
Native/GPU, performance, CI and human acceptance belong to the integrator's
separate evidence; this is not a second execution of those checks.

## Resolved findings

- **Shortcut capture retargeting:** new mouse hover navigation initially changed
  the selected Keymap action during capture. Hover now leaves capture selection
  and focus untouched; `pointer_hover_cannot_retarget_a_shortcut_capture` covers
  this event path.
- **Stuck toolbar press:** a toolbar action opening a modal could swallow its
  button release before clearing the pressed state. Release/focus cleanup now
  precedes modal and active-interaction early returns.
- **Blink-induced text movement:** appending/removing the block caret initially
  changed wrapped Frame title height and centered/right-aligned Note positions.
  Editing reserves caret layout space; visibility changes only the caret's
  rendering. Frame hit geometry uses the same current draft and reserved space.
- **Simple form keyboard routing:** Bookmark/Join/Server edit handling consumed
  navigation keys before ordinary panel focus handling. Arrows/Tab now continue
  to that handler; Enter honors Cancel. Those fields and Keymap search reuse the
  block-caret primitive, with focus/visibility-dependent deadlines.

## Reviewed integration

Toolbar actions retain full 16-pixel cells; edge placement has no enclosing
cartouche or exterior margin. The existing movable bar has an explicit 16-pixel
grip cell. A separator is a distinct layout item, stored through the bounded
profile string list, with no Action/shortcut. Adjacent separator normalization
does not discard separated occurrences. Vertical bars transpose its 1-by-12
visual geometry. The editor uses the same Action-to-icon mapping and atlas,
bounded icon slots, separate list scroll positions, and ordinary button focus.

Context menus no longer suppress the bar. The renderer splits the existing
overlay around icon drawing so a popup occludes only the covered portion of
the icons. The new GPU readback test checks both this order and unchanged atlas
allocation. Modal panels may still replace toolbar presentation. Menu placement
and local UI remain independent of document mutation.

Local selection no longer emits per-member or aggregate gizmo rectangles.
Resize handles and supported rotation geometry remain; foreign manipulation
lease indication is still supplied separately. Frame/Rectangle document color
is therefore readable without selected-object chrome masking it. Existing Frame
rotation restrictions are retained, not expanded in this polish pass.

Frame label drawing/hits share wrapped integer geometry. Double-click dispatches
the same RenameFrame action as F2. Color treatment uses a deterministic contrast
choice over the existing palette. The old unmarked 2x title default migrates to
1x once; subsequent explicit integer overrides survive. Document/storage and
shared authority paths are unchanged.

Feedback expires once. Caret deadlines exist only in active, focused, drawable
editing states and disappear on completion, modal replacement, focus loss or
occlusion. Copy acknowledgement invalidates cached status on expiry. No new
Cargo dependency, worker, network behavior or polling loop appears in the diff.

Runtime artwork is protected by the prior missing-defaults-only packaging
policy. Icon discovery now prefers an existing executable-relative icon folder
over development defaults; this review made no changes to source or runtime
PNG files.

## Acceptance boundaries

Source review does not prove pixel appearance, settled redraw counts, performance
parity, real Windows behavior or two-computer artist comfort. The integrator must
retain final native evidence at 800x600 (1x/2x) and 1024x768, the normal quality
gate, GPU smoke/readback, paired idle measurements and Linux/Windows CI. Physical
two-computer 2A5 acceptance remains pending and is not converted to PASS here.

No Studio Server or other deferred architectural phase was reviewed or started.
