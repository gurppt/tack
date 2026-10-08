# Tack LAN authority

`tack-server` is an optional headless process. It links the semantic model,
bounded storage codec and LAN protocol; it does not link Winit, WGPU or a GUI.
An ordinary local Tack session never launches it.

```sh
./bin/tack-server --listen 127.0.0.1:7337 --root /path/to/server-owned-data
```

An explicit LAN listen address is allowed. This phase has **no authentication or
TLS**: all peers on the trusted LAN can read and edit hosted boards and request
CAS assets. It is not an internet-facing production service.

## Authority and conflict policy

Each board has a stable 128-bit document ID and one monotonic revision. An edit
must name the exact current base revision. A stale request is refused and the
client rejoins from the authoritative snapshot. Accepted operations are sent in
the authority's serialized order; no durable offline merge is attempted.

An operation ID plus client ID identifies a durable request. A retained receipt
acknowledges an exact retransmission without applying it twice or broadcasting it
again. Receipts are bounded to 128 per client. An older retransmission outside
that window retains its original old base revision and is refused as stale.
The client must not manufacture a fresh base for an old unresolved operation.

Undo and redo submit a new operation against current authority. There are up to
32 inverse commands per client. Undo is conservative: if another accepted edit
has intervened, the inverse is refused visibly. Consecutive edits by the same
client can be undone consecutively. Each client history, including receipts, is
limited to 8 MiB. At most 16 client histories are retained; a seventeenth writer
evicts the least recently edited history, with client ID as a deterministic tie
break. Returning evicted clients can edit normally but their previous undo
history and receipts are unavailable. Local document undo is separate.

Source replacement revisions have a persisted high-water mark, independent of
undo restoring an older descriptor. CAS-binding changes require the matching
source creation/replacement semantic command. An unrelated transform cannot
silently change an image's hash under an unchanged source revision.

## Durable ownership

The server holds a cooperative root-directory lock. Remote values never select
filesystem paths: board IDs and SHA-256 hashes use fixed lowercase hexadecimal
names beneath the configured root.

`boards/<board-id>.board` is one authority envelope containing metadata encoded
by Tack's existing snapshot codec, source bindings, revision, source high-water,
undo/redo and deduplication receipts. The envelope has a fixed version, length
and SHA-256 checksum. Opening it validates the checksum, document semantics,
source bindings, history bounds and receipt identities/order before use.
Original bytes are not copied into each metadata update.

Publication writes a private temporary file, syncs it, atomically renames it and
syncs the parent directory on Unix. Startup discards unfinished `.pending`
files. If rename succeeds but directory sync fails, in-memory authority advances
to the published revision and is poisoned: peers are disconnected, and a rejoin
must reopen and validate the published envelope and successfully sync its parent
before editing. The server cannot overwrite that new revision from old memory.
The Windows compile path uses atomic file replacement; directory sync is Unix
only. Windows native server durability has not been measured in this phase.

## Asset and transport bounds

Originals live in `assets/<sha256>`. Uploads stream into a private `.upload` file,
are checked by SHA-256, synced and atomically published. An identical existing
hash with the same length deduplicates. Failed, cancelled and disconnected
uploads release reservations and remove their temporary file. A publication's
disk accounting advances immediately after rename even if directory sync fails.

The default CAS quota is 4 GiB and includes reserved uploads. `--asset-quota`
sets an explicit byte cap. One upload per connection, an 8 GiB per-original
maximum and 64 KiB chunk/range limits keep memory independent of original size.
Asset requests never interpret remote path strings. A missing source binding or
missing CAS file remains an explicit unavailable image rather than authority
corruption. Existing CAS files are not all rehashed on every startup; download
clients must verify the full resulting hash before using a cached original.

The same persistent length-framed protocol serves control and transfers. The
client may keep a separate connection using that protocol for asset work so
chunks cannot head-of-line block edit acknowledgements. Asset transfer messages
work without a board join, permitting original upload before publication.

Limits are 16 loaded boards, 16 active clients per board and 32 active TCP
connections. A rejoin replaces an older half-open session of the same client ID.
Each connection has a blocking reader and writer; there is no idle heartbeat,
poll timer or discovery task. Write stalls time out after five seconds.
Outgoing queues have 64 slots and an 8 MiB byte budget; one larger snapshot may
occupy an otherwise empty queue. A slow peer is disconnected and must rejoin.
Accepted edit frames are checked against their size cap before publication,
and every authority state is constrained to remain snapshot-rejoinable.

Incoming frame allocations remain independently capped by the protocol's
64 MiB frame maximum. The 32-connection cap is therefore also part of the
trusted-LAN memory boundary; this is not a public-service DoS-resistance claim.

## Verification

`cargo test -p tack-server --locked` covers persisted dedupe, stale refusal,
conflicting/chained undo, source binding restore, high-water protection,
corrupt snapshot rejection, CAS chunk offsets/hash/quota/cancellation/ranges,
history eviction, and post-rename sync-failure fault injection.
The real-process integration test runs three simultaneous TCP sessions with
edits in both directions, reconnect replacing a half-open session, server
restart, persistence, duplicate delivery and duplicate original import.
Native renderer/client measurements are additional phase evidence, not implied
by these headless process tests.
