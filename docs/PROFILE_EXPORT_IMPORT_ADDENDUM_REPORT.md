# Portable user profile and toolbar sets

Implemented alongside Phase 2A9 on 2026-10-11. Implementation commit: `161f615993ab85c34fa3ca503aeed93ef0314bda`.

`.tackprofile` is bounded, readable JSON with `format: "tackprofile"`, `version: 1`
and `preferences`. It carries theme, grid/snapping, default import/sampling,
editing/UI preferences, toolbar visibility/placement/scale, named sets and active
index, and the full keymap including Hold/Release. `.tack` stays a document;
`.tackey` stays keymap-only. No board/assets/artwork are bundled.

Export removes recent boards, last board folder, local camera views and the
keyset's filesystem path. Exact floating coordinates and the legacy pixel offset
reset to portable defaults; edge placement and normalized edge position remain.
No credentials or network session state exist in this format. Toolbar content is
mirrored in the active named set; layout and scale belong to the common toolbar.

Preferences offers Export Profile... / Import Profile...; the toolbar editor
also exposes both. Native pickers select files. Export appends `.tackprofile`
without overwriting an unconfirmed suffix-added existing target. Import parses
and validates off-thread before replacing the current window's preferences. A
portable safety copy is written to `before-import.tackprofile` in the profile
root before replacement. If validation or backup fails, settings stay unchanged.
Normal preference publication remains protected against concurrent windows.
The local profile root follows `TACK_PROFILE_DIR` when supplied, otherwise the
existing platform state directory: `~/.local/state/tack` (or `$XDG_STATE_HOME/tack`)
on Linux, `%LOCALAPPDATA%\tack` on Windows. The profile is `preferences.json`.

Toolbar sets are content variants, limited to 16 unique names and 32 entries per
set, with names bounded to 128 UTF-8 bytes. The editor provides previous/next,
New, Rename, Duplicate, Delete, action order and separators. At least one set
always remains. Ctrl+PageDown/PageUp cycle sets and report `Toolbar: NAME`.
Cycling preserves the active tool, drafts, position and scale. Toolbar settings
merge as one unit to prevent inconsistent active-set/content combinations across
windows.

Older preferences missing sets migrate to DEFAULT with their existing content.
An unmodified Default keymap gets the new set/coarse shortcuts where they do not
conflict; named/customized keymaps remain unchanged. Missing set/snapping fields
in portable imports have defaults; missing mandatory preferences/keymap fields or
an unsupported wrapper version produce a readable error. Unknown preference
fields are rejected rather than silently discarded. Files are limited to 256 KiB.
This is version 1 of a local format, without cloud/workspace management.

Validation: five portable-profile integration tests cover complete round-trip,
path stripping, `.tackey` coexistence, old/incomplete fields, CRUD/last-set guard,
shortcut dispatch/tool retention and concurrent-window protection. UI unit tests
cover editor feedback, profile actions and disabled last-set Delete. Native checks
at 800×600 and 1024×768 cover cycling/content persistence, real picker export,
replace import, and the backup. Idle checks remain at zero CPU ticks and zero E/S.
The 30 final native checks, 449-test Rust suite and 31 Python tests pass.
See [committed receipts](../benchmarks/phase2a9/README.md),
`benchmark-results/phase2a9/native-final/` and the
[main phase report](MISSION_2A9_ALPHA_UX_CORRECTIVE_REPORT.md).

Human acceptance remains: Windows native profile pickers, artist keysets and
set names with non-ASCII input/IME. Rename supports ordinary Unicode key text;
IME composition has not been specifically validated.
