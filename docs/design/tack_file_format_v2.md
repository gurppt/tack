# Tack spatial schema 2

The 80-byte container header, checksums, source/asset encoding, payload directories
and bounds from [v1](tack_file_format_v1.md) remain unchanged. Container is 1;
schema at offset 12 is 2. Reserved capability bytes remain zero. Schema >2 is
refused; no editable partial document or replacement file is published.

Schema 2 is emitted only if frames or groups exist. Ungrouped image-only documents
continue to emit schema 1. Schema 1 records and image records remain byte-compatible.

Object records begin with u16 record version (1), u16 kind and u128 ObjectId.
Kind 1 is the existing 119-byte image record. Kind 2 is a frame:

| Field after common prefix | Encoding |
| --- | --- |
| center x/y, width/height | four little-endian f64 |
| name length | u16 byte count, at most 256 |
| name | exact UTF-8 bytes; nonempty/non-whitespace, no control characters |

Frames are axis-aligned and unflipped. Object order then contains the same u128
IDs in the existing explicit order. After order, schema 2 appends u32 group count
and that many group records, before embedded original directory records:

| Group field | Encoding |
| --- | --- |
| record version | u16, 1 |
| GroupId | nonzero u128 |
| member count | u32, 2..100000 |
| members | member count nonzero u128 ObjectIds |

Group count is at most 100000 and at most object count / 2. Cumulative members
cannot exceed object count. Length and enclosing slices are checked **before**
allocating member vectors. Duplicate IDs, memberships, missing objects, frame
members, invalid names/geometry and unknown kinds/record versions are refused.
Groups contain image ObjectIds, never GroupIds; no hierarchy exists.

Metadata remains capped at 64 MiB, independently of payload size. The authoritative
CRC covers all group and frame records. No selection, guides, snap preferences,
camera or temporary frame-name edits are encoded. Old schema-1 readers refuse
schema 2 at the version header, preserving the original file.
