# Tack snapshot format v1

Implemented in Phase 1B (2026-10-03). Container version 1 and document schema
version 1 are independent of app/package version 0.0.1. This is an explicit
binary codec, not a memory dump or Rust/Serde layout. All integers are
little-endian; signed integers use two's complement; floating point is IEEE754
binary64. IDs are exactly 16 bytes encoding a nonzero u128 little-endian.
CRC32 is the standard IEEE CRC used by crc32fast, an accidental-corruption
check, never authentication.

## Envelope

A single uncompressed file contains, in order: 80-byte header, authoritative
metadata, derived overview directory, then addressed payload ranges. No stored
filename is extracted. Payloads may be accessed independently through positional
reads of the same opened file generation. Unix `read_at` and Windows
[`seek_read`](https://doc.rust-lang.org/std/os/windows/fs/trait.FileExt.html#tymethod.seek_read)
receive an explicit offset per read; no code consults the OS shared cursor
(Windows may update it as a side effect).

| Header offset | Size | Meaning |
| --- | ---: | --- |
| 0 | 8 | ASCII `TACKSN01` |
| 8 | 4 | Container version, currently 1 |
| 12 | 4 | Document schema version, currently 1 |
| 16 | 8 | Authoritative metadata byte length |
| 24 | 8 | Derived directory byte length |
| 32 | 8 | Complete file byte length, must equal actual length |
| 40 | 4 | Authoritative metadata CRC32 |
| 44 | 4 | Derived directory CRC32 |
| 48 | 4 | Source count |
| 52 | 4 | Asset count |
| 56 | 4 | Object count |
| 60 | 4 | Embedded-original directory count |
| 64 | 4 | Overview directory count |
| 68 | 12 | Required capabilities/reserved, all zero in v1 |

Authoritative metadata contains document ID, source records, asset records,
object records, explicit back-to-front object-ID order, then original directory
records. Counts come from the header. Sources/assets use deterministic ID order;
objects are encoded in their explicit object order. Parsing consumes the exact
metadata length; trailing authoritative fields are unsupported, never discarded.
There is no compression or payload scan to find metadata/overviews.

## Authoritative records

Each source starts with u32 record byte length. Its payload is:

1. u16 record version = 1, SourceId, u64 nonzero source revision.
2. u8 fingerprint presence (0/1). If present: u64 file size, i64 modification
   seconds since Unix epoch, u32 nanoseconds in `[0, 1_000_000_000)`.
3. u8 source kind: 0 embedded, 1 Unix linked, 2 Windows linked.
4. For linked sources only: u8 absolute flag (0/1), u32 path byte length,
   then lossless native bytes.

The fingerprint is an observation of cheap size/mtime, not a content hash or
freshness proof. An embedded fingerprint records import provenance; the original
range remains the authoritative bytes. Embedded sources carry no extraction path.

Assets are 42 bytes: u16 version = 1, AssetId, SourceId, u32 pixel width,
u32 pixel height. Dimensions must be positive. Object records are 119 bytes:
u16 version = 1, u16 kind = 1 (image), ObjectId, AssetId, five f64 values
(center x/y, world width/height, rotation radians clockwise in y-down space),
two u8 flip flags, four f64 crop values (x/y/width/height), f64 opacity, u8
filtering (0 Default, 1 Smooth, 2 Nearest). The following explicit ordering
sequence contains exactly object-count ObjectIds and must agree with physical
object record order. No duplicate/dangling IDs are accepted within their tables.

Domain validation applies on reconstruction through commands: finite transform,
positive world size, bounded rotated AABB, normalized positive crop fully inside
`[0,1]`, finite opacity in `[0,1]`, known filtering/kind and valid references.
There are no absent/defaulted fields in schema v1. No camera, undo history,
selection, tool holds, decoded pixels, worker state or GPU handles are persisted.

## Path fidelity and source changes

Unix paths preserve native bytes, including non-UTF8 names. Windows paths preserve
UTF-16 code units as little-endian bytes, including unpaired surrogates. NUL,
empty path, odd Windows byte length and paths exceeding 4096 bytes are rejected.
Local native paths must agree with the absolute flag. Foreign descriptors retain
bytes and platform metadata and are not converted into local text. Unsupported
Windows drive/root-relative forms also remain preserved but unresolved; ordinary
relative paths resolve against the parent of the opened `.tack`. Local path
resolution is worker I/O, not document/parser/GPU work. Referenced linked files
may be outside the board directory; the format does not treat them as extraction
names or provide a filesystem sandbox. No watcher or network resolver is used.

The CLI creates canonical absolute links and refuses existing `create` targets
(including dangling symlinks), before preparation and again before publication.
It does not silently replace a different/newer document with a new identity. `repair` preserves descriptors and
refuses output in a different canonical parent when any relative link exists,
including foreign relative links. Save APIs lack the original location context;
a future Save As composition must explicitly rebase/relink or enforce this rule.
Moving a linked file/board does not rewrite its source silently. `SetSource`
changes a descriptor/fingerprint with a fresh increasing revision, is undoable,
and retains object/asset identities. Editor high-water revisions survive undo
and divergence, so a new binding cannot recycle the previous cache token.
A later UI can expose relink through this command; Phase 1B has no relink dialog.
Windows/Linux operational interoperability and Windows filesystem atomicity
require Windows execution evidence; byte preservation alone does not prove them.

## Directories and disposable overviews

Every directory entry is 64 bytes:

| Offset | Size | Meaning |
| --- | ---: | --- |
| 0 | 2 | Record version = 1 |
| 2 | 2 | Kind: 1 original, 2 derived |
| 4 | 16 | SourceId for original, AssetId for overview |
| 20 | 8 | Source revision |
| 28 | 2 | Representation role |
| 30 | 2 | Encoding |
| 32 | 4 | Generator/version token |
| 36 | 4 | Representation width |
| 40 | 4 | Representation height |
| 44 | 8 | Absolute file offset |
| 52 | 8 | Payload byte length |
| 60 | 4 | Payload CRC32 |

Original role/encoding/generator/dimensions are zero. Each embedded source has
exactly one nonempty original entry with matching revision. Original ranges
must lie after the directories, inside the file, and not overlap each other.
No full original is buffered during save; a 128 KiB copy buffer computes CRC.
Stored CRC is verified when copying existing payloads into a replacement.
Metadata-only open performs no original CRC scan. Ranged/partial image decoding
also does **not** verify a whole original's CRC: a valid equal-length substituted
image can decode before a later save rejects corruption. This is an explicit
lazy-integrity limitation; checksums are not a security boundary. Future background
verification can operate separately from document-first opening.

Overview role = 1; encoding = 1 (PNG); generator = 1 (existing native JPEG
thumbnail adapter) or 2 (bounded streaming adapter). The current generator
emits longest edge 128; the directory permits dimensions 1..512 and payload
≤1 MiB. PNG 128 is a representation policy, not image-object semantics.
An overview binds AssetId to source revision; stat mismatch is separately
retained as Changed/Missing/Foreign/Unavailable state. A validated persisted
preview may render as last-known data despite unavailable/changed external bytes.
No source hash/decode is required for that preview. A changed source without a
valid preview requires explicit revision/relink before regeneration.

Bad authoritative CRC/records refuse opening. Bad derived-directory CRC discards
that directory only. Within a valid directory, unsupported version/kind/encoding/
generator, stale binding, duplicate, bad dimension/range or overlap entries are
discarded individually. Payload CRC/decode validation is deferred until demand;
only the damaged/missing representation regenerates if a usable source exists.
Overview ranges cannot overlap accepted originals or each other. Discarding
unknown derived formats is safe because they are reproducible, not user work.

## Reader and runtime bounds

V1 implementation accepts file lengths 80 bytes..8 TiB; authority ≤64 MiB;
each count ≤100,000; each source record ≤8192 bytes; derived directory exactly
64 × overview-count (≤6.4 MB). Checked additions validate all ranges before
payload allocation. Tables are bounded before reconstruction. Original/preview
overlap validation sorts intervals, O(n log n), avoiding quadratic pair checks.
Malformed files produce structured errors, never unchecked indexing/panic.

Workers accept JPEG/PNG source dimensions at most 6000×4500. Encoded parser reads
are limited to 64 MiB (plus sniff/sentinel); streaming embedded originals can be
larger as stored data, but regeneration is limited by this decoder policy.
PNG metadata/decode uses a 192 MiB allocation hint; overview decode uses 4 MiB and
512×512 limits. Native JPEG uses the existing 64 MiB input and scaled-output
limits. These bounds do not promise exact RSS: codec scratch and driver resources
are separate. Two workers admit at most 16 total jobs/results; CPU RGBA cache
64 MiB, worker repair SSD cache 512 MiB; GPU retains the existing upload/queue
budgets. File/stat/decode/cache maintenance stays off native frame callbacks.
Generated repair files use stable asset/revision/generator names and are reused
after CPU eviction. A shared worker mutex guards disk quota maintenance/writes.
The directory is process-owned; simultaneous writers sharing it are unsupported.

## Saving, interruption and compatibility

Save writes a complete snapshot to an exclusively created random sibling temp.
Only its owner cleans it on recoverable errors. It streams payloads, checks
existing CRC/input length and size/mtime changes, then writes header/directories,
flushes with `sync_all`, and renames over the target without first removing it.
On Unix the parent directory is synced after publication. A failure there is
`PublishedButNotDirectorySynced`: the new valid file has already been published.
Earlier failures/interruption leave the old valid target. SIGKILL may leave an
orphan sibling temp: never auto-promoted, never mistaken for a current document.
Manual removal after identifying the abandoned owned temp is safe. No broad
startup scan/deletion of arbitrary temporary files is performed.

New Unix board/temp files use mode0600; owned work directories700 and repair
PNG600. Replacement preserves an existing regular target's permissions after
payload copying; nonregular/symlink targets are refused. Permission/ACL/security
label preservation across Windows/other filesystems is not claimed. Changing
file permissions/contents concurrently or editing the opened inode in place
is outside the single-writer snapshot guarantee.

Four actual process-kill stages and injected I/O failures test process consistency
on Linux. fsync calls strengthen durability; no simulated power failure, arbitrary
filesystem atomicity or Windows durability has been demonstrated. Snapshot save
requires space for the complete replacement and copies all embedded originals;
large-board incremental save/autosave cost remains future measured work.

Unknown container/schema versions, nonzero required capabilities, authoritative
record versions/kinds/enum values/extra fields refuse editable open. No opaque
future-authority preservation or migration exists yet; refusal prevents silent
loss. Future published schemas require fixtures and explicit migrations. Current
metadata is never rewritten while deciding whether a file can be read.

The CLI `create` checks that its destination does not already exist before import
and again before final publication. Existing files (including unsupported/newer
containers and symlinks) are refused. This is a single-writer safeguard: another
process creating a target between the final check and snapshot rename remains
outside the guarantee; no concurrent-create protocol is implemented.

Diagnostic JSON exports use exclusive `create_new` publication and never replace
an existing file, hardlink or symlink. Product input/output path aliases are also
checked before import, repair or native-window startup. If another process creates
the report destination after preflight, the final exclusive open still refuses.
Reports are diagnostic exports rather than recoverable document snapshots.

CLI repair may replace only a regular destination whose canonical pathname is
the validated input pathname; a distinct existing destination is refused before
preparation and checked again before publication. A new destination is allowed.
Low-level snapshot APIs assume the caller supplies this understood target policy.
Concurrent creation/replacement remains outside the single-writer guarantee.
