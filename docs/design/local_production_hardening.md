# Local production contract — Phase 1F

Phase 1F extends the existing Document/DocumentEditor, ProductAssets, ImageSave,
physical input, canvas renderer and streaming storage. There is no alternate
editor, renderer, document singleton, media path or collaboration protocol.
Starting revision: `759fee0ac8ace8558556571498c84a6270b44760` (1E PASS).
The user authorized complete prototype ellipse removal, including the ignored
product brief. Remaining schema-3 kind numbers retain their existing meaning;
kind 5 is unsupported, with no migration/legacy reader. Owned development
fixtures containing kind 5 were deleted and current fixtures regenerated.

## Local windows and ownership

No arguments creates a fresh Untitled board. `new UNUSED_PATH.tack` creates a
named empty board; `open FILE.tack` or a bare `.tack` path opens an existing board.
New/Open/Recent launch independent native processes. Recent paths are bounded to
16 lossless platform descriptors, with no automatic reopening or thumbnails.

A BoardLease holds an OS lock on the stable `FILE.tack.tack-lock` sidecar for the
whole editable session. The canonical existing path is the ownership key;
ordinary symlink aliases resolve to it. Sidecars remain after close, are never
unlinked/recreated to steal a lock, and do not imply a live owner. Lock release is
explicit before File drop (including transient CLI publishers). Same-file
editing fails visibly; different documents have independent workers/recovery.
Hard-link aliases, adversarial non-cooperating writers and network filesystem
lock semantics are not a collaboration/CAS guarantee.

New/SaveAs/CLI-create require an absent target under that lease. Normal save and
repair compare the expected 80-byte header, length and modification time before
publication. An external replacement is refused. This is a cooperative lock plus
best-effort change detection, not an atomic OS compare-and-swap against arbitrary
writers. Unsupported authoritative files are never automatically rewritten.

Untitled uses 16 fixed private slots in the profile. Empty clean slots can be
retired/reused under ownership; first dirty state records the slot in Recent so
it can be recovered. Cleanup verifies the empty document identity and known
owned recovery entries. Existing symlinks, foreign files and unknown recovery
entries are retained. SaveAs/clean close explicitly retire an owned empty seed;
a crash seed with recovery is never inferred disposable from its empty base.

## Worker scheduling, originals and import

One on-demand local operation worker feeds a channel of capacity one. It handles
file pickers, clipboard, incremental imports, relink, preference I/O, recovery
choice and launches. Existing preview workers remain bounded to two with at
most 16 pending product jobs. No local worker exists until work is requested.
A second local request can queue once; cancellation drops the remaining import,
keeping admitted objects and their exact undo steps.

Native drop events coalesce for 50 ms only while arriving. Pickers are optional
fixed `zenity` or fixed PowerShell/Windows.Forms invocations, never document shell
commands. Import supports JPEG/PNG and emits each validated image separately;
it does not wait for whole-board preparation or hash full originals. Native
picker text is bounded to 64 KiB; batches to 4096 paths; existing decoder bounds
remain 64 MiB encoded/6000×4500 pixels. Native drop accepts platform-native paths.

Embedded import streams 128 KiB chunks into one private session spool. Original
ranges share one reader File; a separate writer handle avoids Windows seek/read
cursor interference. Quotas are 2 GiB copied per batch and 4 GiB per session;
failed/cancelled image copies roll back their appended range. The spool persists
until session close, allowing live and undo/redo original ranges to remain valid.
It is not compacted on every edit. Linked import retains the external descriptor.
Clipboard PNG is a single replaced private helper file, bounded to 64 MiB.
Clipboard text/reference output is bounded to 64 KiB before Rust allocation;
note insertion is validated against 16 KiB and controls. File URI decoding is
local-only, strict, percent-decoded and platform checked. Clipboard image support
is Linux; Windows text/reference uses the fixed helper. No converter framework.

Product asset results are keyed by asset/revision. Document-generation changes
prune removed/revised states; stale completions count their actual I/O but cannot
publish. A per-asset negative cache settles more than 256 missing references
without a clear/retry loop. Derived preview failure is disposable; original
failure is authoritative and blocks publication.

Relink selects one image but replaces its shared Source and all related asset
pixel dimensions atomically. Object IDs, layouts, crop and styles remain intact.
The revision high-water mark never recycles an issued revision across undo/redo
or SaveAs. Old preview completions cannot become new-revision content. Missing,
changed and foreign descriptors remain explicit. No file watcher is introduced.

Manual save snapshots committed authority and stable original handles on the
existing ImageSave worker. An acknowledgement marks clean only its captured
generation; edits arriving during normal Save remain dirty. Draft notes/frame
names commit on Save/close/local actions, retain across focus/DPI changes and
cancel only through explicit Escape. SaveAs blocks edits and local completions
until the publication/base-directory change is acknowledged. Relative links
moving to another directory become absolute with fresh revisions; foreign
relative descriptors refuse that operation. Rebased history is cleared where
old relative inverse commands would bind incorrectly.

Loaded embedded originals needed by live/undo/redo state retain their stable
range handles when a relink/save publishes a container that omits them. This
preserves relink → Save → Undo → Save authority. **These handles can pin whole
older container inodes and disk allocation until history eviction/subsequent
publication/close. The 4 GiB spool quota does not bound that retained-container
cost.** History retains at most 200 entries/32 MiB metadata, but original bytes
are not counted by that metadata budget. There is no zero-copy-memory versus
zero-retained-disk claim or eager copying/compaction subsystem.

## Separate recovery and failure guarantees

Recovery never publishes over the normal board. Each owned hidden directory
`.<board>.tack-recovery` contains a 24-byte owner marker (magic/document UUID),
up to two complete snapshots and a 232-byte `state.meta` manifest. The manifest
records generation, active slot, normal base stamp and snapshot stamp.

After five seconds without a new edit, dirty work may snapshot; sustained edits
have a 30-second ceiling. There is one storage worker, no per-edit fsync and no
idle autosave timer when clean or after the same generation is captured. A
failure does not retry forever; new edits/manual action can rearm work.

Publication writes/fsyncs/atomically replaces the inactive snapshot, then
atomically writes/fsyncs/replaces the manifest and syncs the directory on Unix.
A crash/fault before the manifest changes preserves the previous valid recovery.
The normal publisher also streams CRC-verified payloads, fsyncs its temporary,
checks the lease stamp, replaces the destination and syncs the directory on
Unix. A post-replacement directory-sync error can mean published-but-not-confirmed;
errors are retained visibly rather than claiming the old file necessarily won.
Windows build/CI validation does not prove power-loss durability or native DPI.

Startup accepts only owned, structurally valid recovery whose base still matches
the normal file. Restore validates original CRCs on a worker, presents dirty
recovered state and requires explicit normal Save. Discard removes only known
owned snapshot/control files; unknown entries are retained. Corrupt recovery
shows an error with the normal file intact and returns to the recovery choice.
Conservative stale recovery retention is at most two snapshots, plus validated
staging cleanup; unknown files are not deleted. Up to one normal-save temporary
and an inactive recovery snapshot can temporarily duplicate substantial payloads.
Recovery is a whole-document snapshot, not an incremental journal. Low disk space
can prevent both save and recovery. Only a completed snapshot can recover edits.

## Compact preferences, errors and idle costs

Preferences/keymap/Recent share versioned readable JSON capped at 256 KiB,
256 binding records and 16 recent paths. Unknown schema/fields/actions/controls
or conflicts reject the profile/import. Invalid/future profiles remain unchanged
while in-memory defaults permit opening the app. Settings writes and export use
the same stable lock; fingerprint conflicts refuse overwriting another window's
settings, while recent-only updates merge disk settings and preserve foreign
recent descriptors. Preferences never reset board grid just because an unrelated
setting changed. `TACK_PROFILE_DIR` selects an explicit absolute test/profile root.

F10 actions, Preferences, Recent, Keymap, close/recovery and error panels use the
existing pixel overlay and canonical 81 actions. Search accepts up to 128 bytes;
keymaps permit zero/multiple keyboard/pointer/wheel bindings, conflict detection,
press/release and reset action/category/all. No UI framework, font discovery,
mandatory dialog service or resident preference daemon. The panel is allocated
only while visible and glyph output is bounded.

Errors are local and visible in the panel/title without a timer/modal queue.
An unsuccessful initial Open has an explicit noneditable error-only mode:
document shortcuts, drops, local mutations and recovery scheduling are disabled,
partial board/lease state is released, and Enter/Escape/Dismiss closes that
window. A Save error in an opened document instead keeps the dirty editor usable.
CLI stderr and bounded per-error local messages (2048 scalars) are inspectable;
RUST_LOG controls existing optional verbosity. Tack creates no persistent log,
telemetry, analytics, network client or logging thread. User-directed stderr
capture is owned by the invoking shell; Tack does not rotate arbitrary external
log destinations. Profile files, helper output and caches have explicit bounds.

Native title updates are guarded by string equality. Idle uses ControlFlow::Wait;
deadlines exist only for real work/drop/recovery. No caret animation, polling
file watcher, local heartbeat, glyph preload or network wakeup. Shared bitmap
Spleen/Unifont lookup, no-AA raster policy and licenses are in
[ui_font_decision.md](ui_font_decision.md). Numerical fixed/per-instance costs,
startup, I/O, native observations, verification and platform limitations belong
to [MISSION_1F_REPORT.md](../MISSION_1F_REPORT.md).
