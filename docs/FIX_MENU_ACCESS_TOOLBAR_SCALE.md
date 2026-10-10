# Fixed menu access and toolbar-only scale

Baseline: `250deb0`. Bare F10 opens the application menu independently of the
editable keymap. It cannot be reassigned to another action or to Hold/Release.
Existing F10 overrides are ignored without resetting other preferences. Additional
menu shortcuts remain editable. Close, recovery, connecting and reset confirmations
retain their protections; ordinary work panels commit drafts before yielding.

Native modifier state is tracked before panel routing, so Ctrl+F10 does not become
bare F10 and releasing Ctrl inside a modal restores the fixed shortcut.

The global UI Scale preference and submenu are removed. Native system DPI drives
UI geometry. The old JSON field remains readable but no longer overrides DPI.
Toolbar scale remains independently selectable at 1×, 2× and 3×.

Validation: full `tools/check.sh`, Linux and Windows GNU release builds; 43 existing
native UI checks and 10 targeted empty/legacy keymap, About/modifier and DPI checks
on owned Xvfb/software Vulkan. Independent review found no remaining blocker after
the modifier correction. Compact receipts: `benchmarks/phase2a7/menu-safety/`.
Physical Windows and artist acceptance remain pending.
