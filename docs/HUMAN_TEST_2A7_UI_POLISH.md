# Phase 2A7 artist / physical acceptance

Automated evidence is separate from these unchecked acceptance items. Use
`bin/tack`; Windows uses the complete `bin/tack-windows-x86_64/` folder. Back up
personal profiles before choosing Keymap Reset; existing keymaps keep their old
bindings until explicitly reset/remapped.

- [ ] At 800×600 and 1024×768, Keymap's three columns, current keyset name and `*`
      are readable. Up/Down and Left/Right move focus; Enter edits its cell.
- [ ] Wheel and draggable thumbs feel predictable in Keymap, Preferences and
      both Toolbar editor lists. Pointer resting at an edge does not scroll.
- [ ] Normal/Release/Hold behave as labeled. Illegal modes are refused clearly.
      Capture a collision: Enter replaces the named actions, Escape changes nothing.
- [ ] Default Save opens Save As; save a named `.tackey`, edit, Save, then Load.
      Name/dirty state follow the file. Concurrent window edits produce a clear
      conflict instead of combining the wrong keyset identity and bindings.
- [ ] Bind Pan to Middle Mouse Hold. Its initiating drag pans immediately and
      release restores the previous tool. Repeat with a drawing tool and keyboard
      Hold; Alt-Tab and open/close menus without leaving a stuck gesture.
- [ ] Place the toolbar at 25%, center and 75% on each edge; resize the window.
      Try independent 1×/2×/3× toolbar sizes; menus/status remain unchanged.
- [ ] Stretch an image wide/tall, rotate and crop it. Reset Aspect Ratio preserves
      center, area, rotation, flips and crop; Undo/Redo and save/reopen are exact.
- [ ] Complete a Note with Enter: colored paper remains. Shift+Enter inserts a
      newline. Numpad +/− adjusts readable text size; Color Cycle updates paper.
- [ ] Normal Note corner/side resize changes wrapping only; Shift+corner scales
      text and paper. Shift+side still changes only that box axis. One drag is one Undo.
- [ ] Rectangle F cycles OFF/100/75/50/25/OFF; Color Cycle changes both hues.
      Numpad +/− changes fill opacity after Fill, otherwise stroke width. Status
      identifies the target. Future creation defaults remain unchanged.
- [ ] B opens a centered camera capture. Try a function key, modified key, keypad
      and mouse button, including a collision. Enter confirms, Escape/outside
      cancels, loss of focus closes. Recall changes no shared revision/history.
- [ ] About shows the exact supplied logo at the top of its info column, only
      the version beneath, portrait touching its inner frame edges and author/label
      hierarchy. Resize while open; outside click closes ordinary short dialogs.
- [ ] Cancel Save As, Open, Import, keyset Load/Save As and Original export. No
      error modal or document replacement. Unsaved-data confirmation cannot be
      bypassed by clicking outside.
- [ ] Status feedback appears in Tack's bottom strip; normal title contains only
      Tack, filename and dirty marker. Return to settled idle without recurring redraw.
- [ ] Repeat editing on two physical LAN computers and real Windows desktop:
      independent edits, same-object leases, Undo, reconnect and Save to Local.

Whole-system crash cause remains undetermined until the persisted kernel trace
is available. These UI checks do not authorize repeated simultaneous hardware-GPU
stress. Plain Text/curves and Studio Server remain deferred; stop after 2A7.
