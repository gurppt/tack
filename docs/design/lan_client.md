# Optional LAN client, publication and lazy originals — Phase 2A

Local startup never constructs `SharedClient`. Linking `tack-shared` creates no
socket, worker, heartbeat, discovery or account request. A shared connection is
an explicit pay-for-play backend below the existing semantic tools and renderer.
The transport is trusted-LAN TCP with independent framing/versioning; there is
no TLS or authentication claim. See [lan_protocol.md](lan_protocol.md).

## Client authority and connection lifetime

`ClientConfig` contains a numeric IP:port, stable board/client IDs and an absolute
private cache directory. All connect, framed reads/writes, hashing, disk I/O and
transfer operations run off the input/render thread. Hostname discovery and DNS
are not implemented. The exposed states are Disconnected, Connecting, Connected,
Reconnecting, Refused, ServerUnavailable and BoardUnavailable.

The authoritative stream produces a validated initial snapshot, then semantic
Accepted operations in consecutive revision order. An acknowledgement at an
already observed revision is a deduplicated receipt and is not applied twice.
A revision gap disconnects the client until reconciliation. Stale-revision
refusal is visible and triggers an event-driven Hello/rejoin for a current
snapshot. Other explicit reconnects are user-driven; there is no idle timer,
heartbeat or pretended offline merge.

Completed manipulations become one durable semantic request. Local selection,
hover, camera and active drag previews stay local. Shared undo/redo requests go
to the server; private history is not replayed over another client's work. The
server's strict base-revision and conservative inverse-operation policy is the
conflict authority. Native tools use `DocumentEditor`'s shared backend request
queue and accept only server-authoritative durable operations.

Reconnect replaces the authoritative snapshot and discards pending work from a
previous connection epoch. Prepared source uploads also carry that epoch: an
old preparation cannot submit a command after a later reconnect. Durable
operations are never automatically retransmitted after an ambiguous transport
failure. The new snapshot reconciles what the server actually accepted.
Snapshots also carry the persisted source-revision high-water mark, so undo of
a relink cannot make a reconnect reuse an obsolete source revision.

## Workers, queues and priority

Only an explicit shared session owns these workers:

| Worker | Behavior | Bound |
| --- | --- | --- |
| Control writer | Blocking command receive; persistent edit/Hello/undo/redo connection | Four queued requests |
| Control reader | Blocking framed read; validates snapshot/order and emits events | Eight queued events; at most one unconsumed snapshot |
| Visible original supply | Lazy SHA-256 CAS download/cache lookup | Four queued requests, one active download |
| Source preparation | Hash/CRC and upload before submitting source-changing edits | Four queued preparations, one active preparation |

Visible supply has a separate worker/socket from background source publication;
a large upload cannot occupy its FIFO or delay edit acknowledgements. At most
three network connections are active: control, download and publication. Transfer
connections exist only while explicit work is being performed. Reader/writer
share one bidirectional control connection. No worker count grows with image
count.

Each preparation retains at most 8 MiB of command metadata and 4096 original
payload handles. Handles pin existing bytes; they do not copy entire originals
into memory. Wire semantic operations retain the smaller protocol payload cap.
Chunks are at most 64 KiB, hex-encoded inside bounded frames, using stop-and-wait
asset acknowledgements. This is a simple bounded transport, not maximum-throughput
transfer. The final performance report measures its actual cost.

UI methods use nonblocking `try_send`. A full queue refuses an operation visibly;
it does not silently overwrite or create an unbounded backlog. Drain processes
at most eight queued events per call. Event congestion stops the connection and
persists an overflow flag; the next drain always exposes Disconnected plus an
error, even if the original queue had no room for that notification. Rejoin is
required before editing resumes.

Control reads have no idle timeout or heartbeat. Connect/write and active
transfer read timeouts are five seconds. Cancellation is checked between hash
reads, upload/download chunks and before semantic submission. Closing a client
sets cancellation and shuts down the control socket; worker joins happen on a
short-lived cleanup thread, never on the UI thread. Transfer cleanup is bounded
by active I/O timeouts.

`SharedClient::stats()` returns atomic actual framed byte and successful message
counts for control and transfers. Byte counts include actual partial I/O, not
TCP/IP packet overhead. Reading these counters performs no networking. Native
measurement probes are diagnostics only; no probe/heartbeat is installed on
ordinary local boards.

## Canonical sources and client cache

Remote source paths are never authority. The canonical document uses Embedded
source descriptors; a separate binding identifies `(SourceId, revision, SHA-256,
encoded length)`. Client source resolution uses only validated hashes under its
private directory. The renderer and local codec stack remain unchanged.

A snapshot worker writes and opens a metadata-only local view, replacing source
descriptors with relative hash filenames. Missing bindings use a safe nonexistent
`.missing-source-ID` descriptor so metadata remains navigable with placeholders.
The canonical document passed to the editor is unchanged. The native adapter
defers sources without codec jobs or render retries until the corresponding cache download/hash validation
has succeeded; a pre-existing hash filename alone is not a verified original.
Ready events contain the full binding, so obsolete revisions/hashes can be ignored.

The disposable original cache admits at most 512 MiB, with at most 4096 original
entries. A source larger than that client admission limit is explicitly refused;
this phase does not implement a ranged remote codec reader. Current PNG/JPEG
codec admission limits still apply after transfer. Downloading one original does
not imply its full decoded pixels become resident.

Cache admission evicts least-recently-used originals; events invalidate the
native request bookkeeping. Pixel representations already resident can remain
useful after an original is evicted. Original requests must follow actual visible
need rather than continually refetching every evicted source in an oversized
visible working set. No eager mirror of the entire shared board is performed.

Downloads are written to a private sibling staging file, checked for exact
offset/length and SHA-256, synced and atomically published. Cancellation/errors
remove incomplete staging. Startup removes only recognized owned aborted
partials. A cache sentinel, regular-file checks, strict hash names and symlink
ancestry checks prevent cleanup or source resolution from reaching arbitrary
paths. Unknown entries cause refusal rather than deletion.

Cache hits are SHA-256 checked before first use in a session. After successful
validation, private unchanged size/mtime permits reuse without rehashing the
same payload for hundreds of distinct source IDs. Eviction, missing paths or
detected corruption clear this memo. This cache is disposable, not document
authority or a defense against a malicious local owner rewriting its private
files and forging timestamps.

The 512 MiB limit counts original CAS payloads. Up to two local metadata snapshot
files are separately bounded by the existing metadata-file ceiling. Ordinary
display caches, parser scratch, connection framing and import staging are separate
costs. Native shared image import retains the inherited session spool ceiling
of 4 GiB, 64 MiB per file and 2 GiB per request; that staging is not hidden inside
the 512 MiB original-cache claim. Its retained disk cost remains an explicit
limitation until finer staging retirement is implemented.

## Publish and reverse local snapshot

Explicit publication is:

```sh
./bin/tack publish LOCAL_BOARD.tack 192.168.1.20:7310
```

The optional `--output REPORT.json` writes a compact receipt. The command reads
the board off the renderer path, pins/streams its embedded or linked originals,
checks embedded CRC, hashes with SHA-256, uploads only CAS misses, then publishes
canonical metadata/bindings. Original source dimensions, object IDs, layout,
groups and document ID are retained. Known empty or oversized encoded payloads
are refused before hashing. The source local board is never rewritten.

`prepare_edit` supplies the same off-thread workflow for native imports/relinks:
originals must finish uploading before the semantic edit is submitted. Failed
upload/preparation produces a refusal, not an accepted metadata edit whose asset
was never validated. Server CAS quota and deduplication remain the server's
authority.

The reverse workflow is implemented and tested **as a bounded storage API**,
`publish::save_local_snapshot`, not presented as a finished native export action
or a fake CLI command. A caller supplies authoritative metadata and pinned
original payloads. The helper verifies hashes/lengths, pins file descriptors,
computes CRCs and uses existing atomic `.tack` storage to embed the originals.
Storage rejects mutation between validation and copying. Destination must be
new; missing originals fail before publication. Tests reopen the resulting
ordinary local board and verify IDs and original SHA-256.

An eventual UI workflow must obtain required originals explicitly and preserve
the cache/disk bounds while exporting; it must not quietly mirror every shared
source. Save Original As for an arbitrary shared source is also not claimed as
a completed native workflow in this foundation. Those limitations are visible
in the shared UI and final report.

## Validation scope

Focused tests cover explicit event overflow, duplicate authority receipts,
revision gaps/stale rejoin, no-I/O counter stability, cache corruption/missing
accounting, aborted staging cleanup, oversized sparse admission, preservation of
unowned/symlink files, usable snapshots with missing bindings, embedded CRC and
cancelled hashing, and the reverse snapshot's IDs/hash/no-overwrite guarantees.
Protocol and server tests supply framing/version/conflict/persistence validation.

Native three-client, reconnect/relink, asset import, congestion, idle and local
regression receipts are owned by the integrator and reported separately. Unit
green alone is not a native collaboration PASS, a Windows desktop PASS or a
low-end-memory claim. Existing Phase 1L codec/tile limitations remain unchanged.
