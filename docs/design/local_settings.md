# Temporary Preferences and Keymap panels

Phase 1K uses the existing bitmap text/flat-rectangle overlay. LocalUi owns a
bounded set of immediate button hit regions only while its temporary panel is
open. Closed panels allocate nothing and create no worker, timer or index.
Preferences, Scale and Background use compact heights; Keymap reserves room for
its search and actions at 800×600 in both 1× and 2×.

Handle Size uses 3..21 px and Hit Radius 5..32 px. Mouse -/+ and keyboard
Left/Right change by one, disable endpoints and never wrap. UI Scale and themes
retain direct discrete choices. Current values and checked states are explicit;
selection/focus uses a border/marker as well as cyan. Magenta marks editable
controls, yellow shortcuts/conflicts, gray disabled controls. All three canvas
themes now use flat fills; the former second color and interpolation state are
removed from both Rust and WGSL.

Keymap search is visible at the top and scans the canonical 92-action catalog
at event/draw time, case-insensitively matching label, category, action identity
and the canonical active binding label. It has no retained per-action widgets
or background index. Escape clears a nonempty search before closing. Ctrl+F,
click and ordinary typing reach search. Rows select without starting capture;
Enter or Change starts capture. Tab/Shift+Tab reaches visible button controls.

Change applies a candidate map transactionally. A conflict names the requested
shortcut and existing action, refuses the change and leaves the active profile
untouched. Escape/Cancel exits capture. Logical-key normalization preserves
AZERTY semantics; existing imported physical controls and profile migration keep
the same validator. Compact mouse/wheel labels share the menu formatter, and
selected-action details include the trigger semantics.

Unassign and Reset action are visible. Reset action also uses a candidate map,
so a conflict restoring a default cannot partially mutate other shortcuts.
Global and category reset show a distinct confirmation; neither a first click
nor the shortcut alone performs it. Cancel preserves the map.

Import/Export buttons call the existing native picker and local worker. Import
validates through the canonical preferences reader before replacing bindings;
export writes the existing editable profile JSON, with no new file format.
The Keymap panel remains/reopens for the operation result. Profile changes use
the existing guarded asynchronous persistence path, and menu labels always read
the active keymap.

Developer measurements are finite: `keymap_filter_cost` scans all 92 actions,
and `run_settings_checks.py` verifies native mouse/keyboard persistence,
conflict/capture/reset, canonical import/export and open/closed idle across three
themes and two scales on an explicitly owned X11 display. No harness watchdog
or measurement code becomes a production service.
