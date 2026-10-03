# Verified navigation mapping

Verified 2026-10-02 and rechecked 2026-10-03 for Phase 1A against the current
[official navigation handbook](https://www.pureref.com/handbook/navigation/) and
[PureRef 2.1 default shortcuts](https://www.pureref.com/handbook/shortcuts/all-shortcuts/).
This records interaction compatibility, not permission to copy proprietary code
or assets. These are the handbook defaults; users can rebind PureRef itself.

| Action | Verified PureRef mapping | Retained prototype / Phase 1A |
| --- | --- | --- |
| Pan canvas | Middle-button drag; Alt + left-button drag | Both implemented |
| Zoom canvas | Scroll wheel; Z + left-button drag | Wheel implemented; Z drag deferred |
| Move window | Right-button drag or title bar | Native decorated title bar only |
| Touch navigation | Pinch and two-finger pan | Deferred |
| Close | Ctrl+Q | Native window close only |

Zoom anchors to the cursor in physical screen pixels. No final Tack tool or
navigation shortcut scheme has been invented. Window/menu/tool compatibility
beyond this table is outside Mission 0.

Phase 1A routes retained pan/zoom through the semantic Action/Keymap boundary.
Contains(Alt) and Any-modifier middle/wheel preserve existing behavior; keyboard
bindings otherwise remain unassigned. R/Space temporary-tool tests are synthetic
architecture cases, not shipped PureRef defaults. No polished keymap editor,
rotate-view gesture or complete shortcut preset is implemented.
