# Phase 2A6 native artist acceptance

Automated isolated Linux GPU checks supplement this checklist. Physical Windows
and two-computer acceptance remain pending until an artist records a result.
Use the current `bin/tack` or `bin/tack-windows-x86_64/tack.exe`.

- At 800×600 (1× and 2×) and 1024×768, try Top/Bottom/Left/Right/Floating.
  Check exact 16 px cells, no cartouche/margin, crisp icons and a usable grip.
- Open canvas/object context menus repeatedly, navigate arrows/submenus/Enter,
  dismiss with Escape. The toolbar stays present; a menu may cover part of it.
- Tools → Edit Toolbar: navigate both columns and buttons entirely by keyboard.
  Add a mapped tool, check its real icon, selected row, minimal scroll and brief
  green feedback. Add/reorder/remove Separator with Ctrl+Up/Down. Reset, change
  placement, close/reopen and verify persisted order. Separators are 1×12 px.
- Use mouse and keyboard interchangeably. Release after a toolbar action opens
  a modal. Capture a shortcut while moving the pointer across other rows: the
  intended action must retain capture. Export/import Keymap still works.
- Select image, rotated image, Rectangle, Frame and multiple objects. Resize,
  rotate where supported and duplicate/undo. Only handles/rotation stem remain;
  no rectangular transform outline masks the object border.
- Frame titles default to 1×. Double-click its title and use F2: same rename,
  hard block caret, Enter commit, Escape cancel, click away commit. Save/reopen.
- Hover Frame border/title and then its interior. Only border/title highlight.
  Cycle all colors, check border and label match and light/dark text is readable.
- Edit a Note and simple local text fields; verify caret and existing Note
  Enter/Shift+Enter behavior. Exit editing, wait two seconds, check quiet idle.
- Repeat with two computers: unrelated edits leave menu/gesture/view local,
  foreign manipulation has a red outline, exact-target deletion refuses safely,
  Frame title/color save through the server, Save to Local and Close Board work.

Record correctness failures, subjective friction and computer/OS/build separately.
Do not treat automated single-host clients as physical artist acceptance.
