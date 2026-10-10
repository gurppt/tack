# Optional LAN document backend (Phase 2A / desktop 2A4)

Tack remains a local native board application. File → Share Board / Join Shared
Board provide the artist-facing [desktop lifecycle](shared_desktop_lifecycle.md),
including explicit ownership of the existing headless server. CLI remains
available: `tack publish LOCAL.tack IP:PORT`
explicitly transfers the selected board's originals and publishes its validated
metadata. `tack join IP:PORT BOARD_ID` opens a shared window. The separately
started `tack-server --root PRIVATE_DIRECTORY --listen IP:PORT` is headless.
The default listener is loopback. IP addresses are numeric; discovery, accounts,
DNS, TLS and authentication are outside this trusted-LAN foundation.

## Document and input ownership

`DocumentEditor` is the common semantic command boundary used by existing tools.
Local editors execute commands and retain their existing bounded private undo.
A shared editor instead queues at most 32 requests / 8 MiB of command metadata,
with a 512 KiB retained-command admission limit. It displays accepted authority;
existing transient gesture and text previews remain client-local. No private
undo command can mutate shared authority directly. `BackendRequest` contains
only a semantic Command, Undo or Redo; the core has no network dependencies.

The native adapter submits one durable request at a time. Another request waits
for acceptance or visible refusal. Phase 2A5 captures request bases and checks
[object conflict clocks and transient leases](object_revisions_leases.md).
Unrelated accepted edits preserve active gestures and local UI. Exact target
changes invalidate only intersecting gestures/drafts/unsent edits; menu actions
check fresh live targets. Refusal does not count as a mutation or clear unrelated
queued requests. An accepted command advances ordinary query/render generation.
Camera, selection, tools, preferences and renderer caches never enter the wire.
A fixed pixel indicator and artist-facing status/title expose connection state.
Revision/address debugging belongs in Advanced or explicit diagnostics; F5
explicitly reconnects.

## Event and supply ownership

Only `SharedClient::start` creates LAN workers. It is called only for `join`.
Ordinary local startup has no collaboration socket, thread, timer or service.
One blocking control worker, one blocking reader and two serialized transfer
workers (visible supply and source publication) use bounded queues and wake the native loop through its existing event
proxy. The persistent control connection has no heartbeat. Separate connections
using the same protocol exist during visible supply and source publication; 64 KiB stop-and-wait
chunks keep background transfers away from edit acknowledgements. No hashing,
source copying, network read/write or snapshot filesystem access happens in
renderer/input callbacks. Optional report output takes just two traffic-counter
samples at 10 and 15 seconds; these diagnostic deadlines do not exist in normal
shared or local windows.

The server's canonical document uses Embedded source descriptors and independent
SHA-256 source bindings. Remote filesystem paths cannot enter canonical metadata.
A worker creates a disposable metadata-only `.tack` supply view resolving private
CAS paths; it is never editor authority or a second document semantics. Missing
bindings resolve to placeholders. `ProductAssets` receives private source paths
only after the client verifies downloaded/cached bytes. It otherwise uses the
same workers, representation policy, LOD, geometry and GPU renderer as local
boards. Rejoin reuses those workers rather than overlapping decoder pools.

Original cache admission is 512 MiB / 4096 content hashes. An original larger
than this cache admission is visibly unavailable to the shared renderer; its
metadata and server-side bounded streaming storage remain usable. This is an
explicit Phase 2A limitation, not a claim of arbitrary huge-source remote ROI.
Eviction does not invalidate already decoded pixels. Visible-source eviction
cannot trigger a render-driven fetch loop: a source can be requested again after
it leaves and re-enters the viewport or on explicit rejoin. Ordinary local huge
PNG/JPEG limitations in Phase 1L remain unchanged.

Local import admission is reused. Shared source preparation hashes/uploads on
the transfer worker before submitting its semantic edit. Embedded import staging
has the inherited 64 MiB/file, 2 GiB/batch, 4 GiB/session disk caps, separate from
the disposable CAS quota. Original payload handles are released after preparation
is submitted; the private spool is retained until window close. This existing
staging cost is bounded but is not included in the 512 MiB CAS cache claim.

## Authority, conflict and undo

The server serializes edits; every operation must match the current base revision.
The first accepted operation advances revision; competing stale operations are
visibly refused. There are no invisible property merges or offline durable edits.
Unplugging a network cable leaves a local board independent. Shared loss is
read-only after the TCP stack reports it. With no heartbeat, silent cable loss
can remain undetected by TCP until an operation/reconnect; an unacknowledged edit
never mutates durable client state. F5 discards unsent work and installs a fresh
validated snapshot. Accepted durable requests are never automatically replayed.

Undo/redo is a new server operation, using the inverse produced by the common
validated core. It requires a current base revision and a matching history head;
any intervening accepted edit from another client refuses the inverse. Each of
16 most recently edited clients retains at most 32 undo/redo entries and 128
receipts, sharing an 8 MiB serialization budget. Older client histories are
explicitly evicted by global LRU. Exact retained duplicates acknowledge their
original receipt; older duplicate frames keep their obsolete base revision and
are refused rather than reapplied. Source revisions have a separately persisted
high-water mark, supplied on rejoin, so source undo cannot recycle revisions.

## Persistence, bounds and reverse workflow

See [the server storage note](../../crates/tack-server/README.md) and
[wire specification](lan_protocol.md). Accepted metadata, revision, source floor,
inverses and receipt state publish together in one checksum-protected atomic
snapshot. Original CAS files are stored separately and never rewritten on edits.
A post-rename durability error poisons the board and disconnects its subscribers;
rejoin validates and reconciles the published state instead of continuing from
old in-memory authority. Per-operation bounded metadata rewrite is deliberately
simple and must be measured under stress; it is not an append-log scalability
claim. CAS orphan collection is deferred; quota exhaustion is explicit.

Publish preserves board/source/asset/object/group IDs and geometry and never
modifies the selected local board. Publishing an already hosted ID is refused.
The reverse path is the tested `publish::save_local_snapshot` API: supply pinned
original handles, validate their hashes, then stream them into a new ordinary
Embedded `.tack` with an exclusive new-file lease. Missing originals, wrong hashes
and existing destinations fail. A user-facing remote-export picker and shared
Save Original As are deferred; existing shared Save/Save As explains server
ownership rather than overwriting the disposable supply view. Publish CLI is
fully implemented; reverse export is an architectural API per the brief's option.

This trusted-LAN server has no roles or secrets. Anyone with listener access can
read/write hosted documents and CAS content. Lengths, IDs, source bindings, paths,
queues and quotas are bounded and tested; that does not make public internet
exposure safe. No renderer, browser, executable/plugin payload or shell execution
is introduced on the server. Windows compilation/tests do not prove Windows
native desktop collaboration behavior.
