# Tack annotation schema 3

Schema 3 retains the container-1 header, directories, checksums and byte/count
limits of [v1](tack_file_format_v1.md), and the frame/group records of
[v2](tack_file_format_v2.md). Only documents with annotations emit schema 3.
Without annotations, image-only documents still emit schema 1, frames/groups 2.
Readers accept 1/2/3 and refuse newer schemas, unknown required kinds/versions,
alignments or reserved values without publishing an editable partial document.

Annotation records start with u16 version=1, u16 kind, nonzero u128 ObjectId,
then u32 payload byte length, all integers little endian. The enclosing slice
and length (maximum 65,664 bytes) are checked before payload-owned allocations.

| Shared payload field | Encoding |
| --- | --- |
| center x/y, width/height, rotation | five f64 world values |
| flip x/y | two canonical Boolean bytes |
| stroke | four RGBA8 bytes |
| fill present | one Boolean byte |
| fill | four RGBA8 bytes, all zero when absent |
| stroke width | f64, finite 0.1..256 world units |
| opacity | f64, finite 0..1 |

Shared fields total 67 bytes. Geometry must pass Transform and expanded visual
bounds admission, with finite world coordinates inside the existing ±1e9 limit.
Width/height are positive; rotation and flips reuse image semantics. RGBA8 bounds
are intrinsic. Stroke remains world width when the object box is resized.

| Kind | Suffix after shared fields |
| --- | --- |
| 3 Text | f64 font size (4..256), u8 alignment (0 left, 1 center, 2 right), u32 UTF-8 byte length (0..16384), exact bytes |
| 4 Rectangle | none |
| 5 | unsupported; never reinterpreted |
| 6 Line | four f64 for two normalized x/y endpoints |
| 7 Arrow | same endpoints, one bounded end head derived at render/hit time |
| 8 Scribble | u32 point count (2..4096), count pairs of normalized f64 x/y |

Points are finite, inside [0,1]; line/arrow endpoints must differ. Text rejects
control scalars except newline/tab, invalid UTF-8 and trailing payload fields.
Length/count/enclosing bytes are checked before allocating String/Vec. Stored
scribble points are the completed simplified vector, not raw transient capture.
Horizontal/vertical lines retain both endpoints in a positive minimum-size box.

Image/frame records remain unchanged. The explicit object order includes all
kinds and defines the common canvas order. Schema 3 always includes the schema-2
group-count section, even when zero. Groups remain flat and image-only; annotation
members are rejected. Authoritative CRC covers all these records. Metadata remains
capped at 64 MiB; no glyph atlas, caret, tool, capture, selection or history is stored.

Malformed/future authority is refused before editing or repair/replacement, with
input bytes unchanged. The integration tests cover six-kind exact round trips,
old schema 1/2, frames/groups, unknown versions/kinds, invalid lengths/styles/text
and non-finite geometry. Source/original/overview storage is unchanged.

Phase 1F removes the prototype oval object. Tag 5 is rejected by the ordinary
unknown-kind check; there is no legacy decoder or migration. Other tags retain
their values, so current schema-3 documents without that kind remain readable.
