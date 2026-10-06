# Owner test — Phase 1I

Automated native checks pass; owner comfort/discovery review is still pending.
Run `./bin/tack`. `./bin/BUILD.txt` identifies the exact commit and release SHA.
The verified runtime SHA is
`56ffdaf83a6f4678e20127c887cf2b64788b7ce03cb87bbc2fe34a5facc4ab90`.

Use your normal French AZERTY layout, then repeat in QWERTY if convenient.

1. Import an image into a new board.
2. Move it.
3. Press the actual labelled Z with Ctrl: the move should undo.
4. Ctrl+Shift+Z should redo; Ctrl+Y is the alias.
5. Save with Ctrl+S.
6. Press T, click/drag, type, Ctrl+Enter: a note commits and Pointer returns.
7. Create a rectangle (R), line (L), arrow (A): each release returns Pointer.
8. Select 6–12 mixed portrait/landscape images.
9. Right-click a selected image → Arrange → Arrange in Grid: sizes stay unchanged.
10. Undo once: all prior positions return together.
11. Redo once: the whole grid returns.
12. Arrange → Snap Selection to Grid: independent unit top-lefts snap to the base lattice.
13. Undo once; try Escape during unfinished creation (nothing should commit).
14. Repeat at 800×600, checking the Arrange menu and Tack → Edit → Keymap.

Also try P: Freehand stays active for another stroke; Escape/V returns Pointer.
Ctrl+Z inside an active text draft is deliberately consumed and has no local
text Undo; Escape discards the draft. Snap can introduce overlap because it is
independent quantization. Preferences v1 migrate logical key names; unsupported
legacy positions preserve the original profile and require repair/export to v2.

Check punctuation shortcuts if you use them: bindings use the layout's base
character, not Shift/AltGr-composed text. [ ], Shift+[ ], Ctrl+Shift+. may need
remapping or menu invocation in AZERTY; this remains an ergonomic review item.
Record any remaining friction with resolution, layout, exact key/tool and steps.
Real Windows desktop review is separate from Windows CI. Do not interpret the
native Xvfb automation as a comfort or physical monitor latency assessment.
