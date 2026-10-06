# Layout-aware shortcuts — Phase 1I

Command and tool defaults are logical keyboard bindings. Ctrl+Z means Z on the
active keyboard layout, including French AZERTY; it does not mean the US-Z
physical position. Pointer V, Note T, Rectangle R, Line L, Arrow A and Scribble P
follow the same policy. Named keys (Delete, arrows, F2, F10, Space) remain named.
Ctrl+Shift+Z and Ctrl+Y both redo. Existing modifiers are unchanged.

On native Linux/Windows, winit's `key_without_modifiers()` supplies the active
layout's base key, independent of Shift/Caps/Ctrl transformation. Text composition
continues through the separate native text/IME path. Here “logical character”
means the layout's unmodified base character, not composed text produced by
Shift/AltGr. Punctuation defaults such as [ ], Shift+[ ], Ctrl+Shift+. can require
remapping or menu invocation on layouts without those base characters, including
French layouts. Labels describe the configured binding; they do not guarantee
that every keyboard layout exposes that base character. The native matrix proves
the targeted letter/named shortcuts, not all punctuation. This remaining
punctuation ergonomics question is reserved for owner review. Keyboard events retain both
physical identity and fixed-size logical identity; no per-event heap queue,
timer, translation table per board or background worker is added. Resolution
scans the existing bounded keymap (256 bindings), with at most 32 held inputs.

Press captures the physical identity for release. Logical/layout/modifier changes
during a hold cannot orphan its release. A second physical key aliasing the same
logical key cannot release the first key's held action (Enter/NumpadEnter is
covered). Repeat events do not repeat one-shot commands. Focus loss drains held
state and cancels unfinished creation through the existing owner.

Menus and canvas resolve the active keymap into the same `Action` and
`ImageInput::dispatch` document history. The label formatter is shared with the
keymap panel. Logical chords have familiar labels (`Ctrl+Z`); deliberately
physical version-2 imports are shown honestly (`Ctrl+pos:Code(KeyZ)`). A
physical binding applies only when its configured modifiers match. Conflicts
between physical and logical keyboard domains with overlapping modifier masks
are conservatively refused, including different triggers: a layout can otherwise
make their identities overlap. This can reject combinations that would happen
not to collide on today's layout; it avoids layout-dependent shadow dispatch.

Preferences use version 2 and accept version 1. Version-1 US key names migrate
to the advertised logical letter, including customized letter/named assignments;
this changes old position-based muscle memory intentionally to preserve the
user-facing label. The conversion is in memory until normal explicit preference
publication. Letter/digit/punctuation, navigation, function and Space mappings
are supported. Legacy numpad/media/unsupported international positions are not
silently guessed: loading fails, the original file stays intact and the existing
protected-profile/default-keymap behavior applies. Such a custom file must be
repaired/exported with an explicit version-2 physical binding. Unsupported
IntlYen preservation is tested. No French-specific remap is embedded.

While a note draft is active, Ctrl+Z is consumed: there is no local text Undo
buffer and document Undo must not erase earlier board edits. Ctrl+A selects the
draft's text; Ctrl+Enter commits; Escape cancels. After commit, document Undo
undoes the single note operation. File-name modals also consume keyboard input.

Opt-in `TACK_TRACE_INPUT=1` logs actual physical/logical identity, modifiers,
normalized event, resolved action and owner history counters for diagnosis.
It is disabled in normal runs and all accepted performance observations.
