# Phase 2A LAN protocol, major 1

Tack uses an optional, persistent, bidirectional TCP protocol with explicit semantic
records. Linking `tack-shared` creates no socket, worker, timer or service. Only
an explicit shared-board action starts collaboration. The server is for trusted
LANs: there is no authentication, encryption or internet-exposure guarantee.

A client uses one control connection and a separate asset transfer connection,
both speaking this protocol. This keeps bounded asset chunks and filesystem work
away from authoritative edit acknowledgements. There is no heartbeat or discovery
traffic. Blocking readers sleep in the OS until a message or connection shutdown.
Connection lifetime and queue limits are enforced by the client/server owners.

## Framing and independent version

Each frame has a ten-byte header followed by exactly the declared UTF-8 JSON bytes:

| Offset | Length | Meaning |
| --- | --- | --- |
| 0 | 4 | ASCII `TLAN` |
| 4 | 2 | Protocol major, unsigned big endian; currently 1 |
| 6 | 4 | Payload byte count, unsigned big endian |
| 10 | count | JSON message |

A zero-length or over-64-MiB payload is refused before body allocation. An unknown
major is refused before reading its body. Truncation, invalid UTF-8/JSON, unknown
variants/fields and additional JSON values are errors. There is no compression.
A received frame has at most 64 MiB of encoded data; decoded metadata and temporary
validation allocations are additional bounded memory, not a claim that total
process RSS is limited to that frame size. Normal control operations have a
1-MiB encoded limit. Snapshot and Publish alone may use the larger frame limit.

Snapshot also carries `source_high_water`, at least the greatest live source
revision. Unlike restored document bindings, this floor cannot move backward on
undo. Clients seed their next source replacement from it on every rejoin, so an
old binding restored by undo cannot make future relink revisions collide.

Each stable ID is exactly 32 lowercase hexadecimal digits, nonzero. A SHA-256
content hash is exactly 64 lowercase hexadecimal digits. These tokens are never
filesystem paths. Revisions and offsets are unsigned 64-bit integers, indexes
unsigned 32-bit integers. Geometry is finite and passes the existing core
constructors, including crop, opacity, text and stroke limits.

## Messages

All messages have a `type` field with the following stable names. The Rust
`Message` enum in `crates/tack-shared/src/protocol.rs` is the exact field contract.

| Message | Purpose |
| --- | --- |
| `hello` | Board ID, client ID and last observed revision |
| `snapshot` | Authoritative board/revision, validated metadata, persisted source-revision high water, source bindings and connected-client count |
| `publish` | Create a board from canonical metadata and original hash bindings |
| `edit` | Operation ID, base revision, semantic command and source-binding delta |
| `accepted` | Authoritative board/revision, origin client/operation, command and source-binding delta |
| `undo`, `redo` | Operation ID and base revision; inverse selection belongs to the server |
| `refused` | Optional operation ID, current revision, typed reason code and short printable explanation |
| `asset_begin` | SHA-256 hash and declared total length |
| `asset_status` | Hash/length and whether the verified original already exists |
| `asset_chunk` | Hash, absolute offset and hex-encoded bytes |
| `asset_progress` | Hash and next accepted offset |
| `asset_commit` | Request full-content verification and publication |
| `asset_ready` | Complete verified original's hash and length |
| `asset_get` | Hash, absolute offset and requested bounded length |
| `asset_data` | Hash, offset, total length and hex-encoded bytes |
| `asset_cancel` | Cancel the connection's transfer of that hash |
| `asset_cancelled` | Explicit cancellation acknowledgement; never a complete-content receipt |

Refusal codes distinguish stale revision, invalid operation, conflict, version
mismatch, unavailable board, busy, unavailable/invalid asset, persistence and
protocol failures. Explanations are limited to 512 bytes and cannot contain
control characters. Socket closure is also a transport failure, never an accepted
edit acknowledgement.

## Canonical document and semantic commands

`DocumentRecord` carries a spatial metadata schema, three record counts
(sources/assets/objects) and hexadecimal metadata bytes. The metadata codec is
the same explicit validated codec used by local `.tack` snapshots; no Rust memory
layout, caches, renderer data or executable content crosses the wire. Schemas
1 through 3 are supported. The LAN envelope permits less than 32 MiB of binary
metadata so the encoded snapshot remains within its 64-MiB frame bound. Source
bindings and JSON overhead count toward that final frame bound too.

The canonical shared document retains stable IDs, object order, groups, every
implemented object kind and all durable properties. Source descriptors become
Embedded with the same source ID/revision; native paths and observed local file
fingerprints are stripped. Remote Linked paths are explicitly refused on decode.
A separate `SourceBinding` maps a source ID/revision to strong hash and length.
Duplicate bindings, mismatched document source revisions and oversized originals
are refused. An omitted binding means an explicitly missing source, not permission
to resolve an arbitrary remote path. Any local cache path is constructed privately
from a validated hash after decoding, outside document authority.

Commands have stable `kind` names for all current core operations: flat batch,
add/remove/set source, add/remove/set asset, add/remove object, transform, crop,
opacity, filtering, add/remove group, frame name, annotation style, text and order.
The object DTO includes all image properties and frame or annotation metadata;
annotations include rectangle, text, line, arrow and scribble. Ellipses are absent.

A batch is flat and limited to 4,096 edits as well as the encoded operation budget.
Core validation still checks references, order, group membership, geometry and
source revision semantics when authority applies it. Parsing a command does not
constitute server acceptance. Client selection, camera, hover, tools, dialogs,
preferences and caches are never commands in this protocol.

## Assets and allocation bounds

Originals are SHA-256 addressed. A single original is at most 8 GiB in this
foundation. Hashing reads fixed 64-KiB chunks; transfers carry at most 64 KiB of
binary content per chunk, encoded as at most 128 KiB of lowercase hex. Requests
must have a nonzero length within that bound, checked offsets and a range within
the declared total. Full original bytes are never a JSON frame or a document
record. Hash verification and durable publication belong to the server's asset
store, not to framing alone. Asset-ready is reserved for verified complete bytes.

The published constants are a 64-message queue cap, a 100,000-record domain cap,
4,096 flat edits and existing core limits (16-KiB text, 4,096 stroke points,
100,000 group members). Owners may impose stricter limits; they must not silently
create unbounded channels or caches. Queue exhaustion or refusal is explicit.

## Reconnect and security ownership

Per-client history and duplicate receipts are bounded. The server retains at most
16 history identities and can evict the least recently edited identity, with the
client ID breaking ties. An evicted client has no retained private undo; a repeated
original request whose receipt expired retains its old base and is stale rather
than being applied again. This is a bounded idempotence window, not an unbounded
operation archive.

The server decides authoritative ordering, duplicate-operation receipts, stale
base handling, inverse conflicts and durable publication. Reconnection starts
with Hello and authoritative reconciliation, not an opaque replay of a client's
private history. See the authority/storage and conflict notes for those policies.

Framing tests cover every truncation of a valid frame, wrong magic/version,
overdeclared size, invalid IDs, unknown fields, domain-invalid opacity, malformed
asset ranges/hex, all command kinds, all snapshot object kinds, flat-batch limits
and rejection of remote filesystem paths. These tests do not prove persistence,
server restart, cache eviction or native-client interaction; those need their
separate integration and independent-review evidence.
