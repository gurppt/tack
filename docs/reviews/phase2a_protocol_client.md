# Phase 2A independent protocol and client review

Reviewer scope: the independently authored `tack-shared` protocol, client,
cache/publication workers, and the native/core-editor adapter. This reviewer
authored `tack-server`; this document therefore does **not** independently
approve that server. The separate
[native/client review](phase2a_native_client.md) reviews server publication and
integration from the protocol author's perspective. Neither note substitutes
for native and performance receipts.

Status: implementation review, final workspace quality gate, strengthened
three-client native/image-supply checks and shared performance receipts passed.
Final local-regression acceptance is pending complete paired runs. The first
native run is insufficient for complete acceptance, as explained below.

## Findings and resolutions

| Finding | Resolution checked in source / remaining proof |
| --- | --- |
| Native snapshot setup exposed a cache hash filename before integrity verification. | Snapshot marks each source explicitly deferred, so no codec job is submitted before verification. Only a matching full-binding `AssetReady` event exposes the verified CAS path. Obsolete ready events are ignored. A domain/backend test checks deferred supply without speculative decoder errors. |
| Source undo followed by rejoin could reuse a revision previously accepted by the server. | Snapshot carries independently persisted `source_high_water`; framing requires it to cover live sources, and the native editor seeds its next replacement revision from it. Protocol and native relink/undo/rejoin/relink tests exercise this. |
| Background source publication and visible supply shared one FIFO. | Publication and visible original downloads now have separate bounded workers/queues and sockets. The persistent control connection is separate from both. |
| Rejoin left transient frame-name editing active against old authority. | Snapshot cancels gesture/note/name state, closes the context, and prunes selection before replacing the editor. |
| Old prepared edits could cross a reconnect. | Requests and source preparations carry a connection epoch; control checks it before sending, preparation checks before and after upload. Ambiguous durable requests are not retransmitted automatically. |
| First native run reported semantic convergence while all shared images were unavailable. `snapshot_view` leaves a storage `.tack-lock` sidecar that `OriginalCache::open` treated as an unknown file. | Confirmed in `native-01/checks.json` diagnostics and a real captured placeholder. Fix recognizes only exact generated snapshot-sidecar names, validates regular zero-byte locks, bounds snapshot/lock counts, and retires sidecars only after acquiring and releasing their storage lease. Regression covers verified original admission after snapshot, eight reconnects, active leases and preservation of an unowned nonempty sidecar. Native-02 now proves original pixels in all three native clients and zero final supply errors. |

The final deferred-source resolver also reports a deferred representation as
unavailable to demand scheduling, preventing a missing original from continually
requesting redraw. Its unit test checks no decoder/I/O work while deferred and
normal decoding after verified supply arrives. Shared preferences are applied
after window/camera initialization, and an initial failed join displays actual
shared state/error plus F5 rather than a permanent local loading title.

The queue-overflow, same-client serialization, foreign-client draft cancellation,
source-path pruning and worker-reuse fixes identified by the other reviewer were
also checked in their final paths. Full event queues retain an overflow flag,
shut down transport, and expose Disconnected during drain. Native submission has
one durable request in flight; a foreign accepted edit cancels visibly queued
unsent work and active drafts rather than silently rebasing it.

## Bounds, authority and local operation

Framing checks magic/version and a nonzero maximum 64 MiB declaration before
body allocation. Operations have an additional 1 MiB payload limit, and binary
asset chunks are at most 64 KiB. JSON DTOs reject unknown fields; constructors
validate geometry and references. IDs and SHA-256 hashes are strict hexadecimal
tokens. Remote source paths are refused in canonical metadata. These are bounded
allocations, not a claim of a 64 MiB RSS ceiling: JSON, hex decoding and domain
validation need additional temporary memory.

Shared client queues hold four control requests, four visible downloads, four
source preparations and eight events, with at most one unconsumed snapshot.
The core backend separately bounds queued semantic work to 32 requests / 8 MiB,
with 512 KiB per retained command. Only explicit shared sessions create LAN
workers or sockets. Ordinary local editors retain the original history/storage
branch and `shared: None`; linking the crate creates no service or discovery.

All connect/read/write/hash/cache/snapshot work runs on shared workers. UI queue
admission uses `try_send`; close sets cancellation and shuts down sockets without
joining a worker on the input thread. Transfer cancellation is checked between
chunks, with five-second active network I/O timeouts. There is no idle heartbeat
or polling reconnect loop. Shared-only telemetry deadlines are two diagnostic
samples; the ordinary local path has no such deadlines.

The cache accounts at most 512 MiB / 4096 original entries, plus at most two
separately bounded metadata snapshots. It rejects unknown files and symlink
ancestry, uses owned partial names and verifies SHA-256 before first exposure.
Native import staging retains the pre-existing separate 4 GiB session ceiling;
decoded display supply and parser scratch are additional costs. The explicit
client admission limit refuses originals larger than 512 MiB. Missing bindings
and missing originals retain navigable canonical metadata with placeholders and
an explicit error; they never authorize a remote filesystem path.

Accepted commands alone mutate shared durable state. Revisions must be consecutive;
duplicates at an already observed revision are ignored. A gap/application failure
makes the backend read-only until authoritative reconciliation. Snapshot replacement
clears unsent work; private local undo history is never replayed over authority.
Publish preserves source local files and stable IDs. Reverse local snapshot is
an implemented tested API, with native shared export explicitly deferred in
[client documentation](../design/lan_client.md).

## Evidence and acceptance limits

`benchmark-results/phase2a/check-final-06.log` ends with successful workspace
tests/documentation and `advisories ok, bans ok, licenses ok, sources ok`.
It includes the snapshot-sidecar regression, deferred-source supply tests and
CAS and pending-file startup cleanup regressions. Both cleanup fixes have
independent source review from the other reviewer. The earlier
`check-final-03.log` Clippy type-complexity failure and `check-final-05.log` stale
test assertion are resolved in this final gate; those failed runs are not PASS
evidence.

`benchmark-results/phase2a/native-01/provenance.json` records 25 passing semantic
checks on isolated Linux X11 `:99`, but its client diagnostics include
`invalid LAN message: unowned file in shared cache`. Its joined B capture shows
a placeholder. This run establishes semantic convergence/restart/conflicts, not
successful native original supply. The replacement harness now requires actual
fixture-colored pixels and zero unavailable sources in all three native clients
before granting complete acceptance. `native-02` subsequently passed 32/32
checks on isolated X11 `:99`: each joined capture contains 180,000 original
fixture pixels, and final captures contain more than 84,000 fixture pixels.
All three final reports have zero errors, missing, unavailable and foreign
sources, revision 15, and the same canonical hash
`2442195e2c628db1f394091f24483ccfbce161bb5be6ed1149b2b181d36d011a`.
The recorded client hash is
`a38d5294e7ad528da991c3f8243166fefb8a432eed69e572ec2ca728b2e8d5c7`;
server hash is
`e4d1c5174f35e8e932b0e02bce0d8370aa0586593f4db97511149f882fe7a2ee`.
The latest `native-03` reran all 32 assertions on the final binary and server,
again with zero final errors/missing/unavailable sources in all three clients,
revision 15 and canonical hash
`70f2da2d58084eee164bb59929e9599e1b76932d3f19a53c133cfb6587a3136c`.
Its client hash is
`0cd2e13e6679b829316f64fff9c71900655a61826a0a0cd52feb5d4efc605a9b`;
server hash is
`3e43ef7addba4eae197761f87e6287c2b570bb6ab2121a5950c93c8cd0417108`.
The preserved raw checks, reports, captures and provenance support this
Linux-native conclusion; they do not prove performance or Windows desktop use.

The performance harness has been independently read. It isolates exact framed
traffic, measures server/client idle with 0/1/2/3 clients, exercises 1/10-object
commands, performs 40 accepted remote edits during native pan/wheel input,
measures 10,000 shapes / 100 notes, full-snapshot rejoin across a revision gap,
CAS throughput/deduplication, and a fresh client after removal of one owned CAS
original. Its reported callback timing is a proxy, not physical end-to-end input
latency. Local regression is serial against one preserved 1L binary; it records
startup/useful-view, idle ticks/RSS/threads/GPU accounting and socket ownership.
`performance-01/summary.json` uses the same final client/server hashes as
native-03. All three clients record zero idle byte/message deltas over their
five-second probe intervals. The server records zero CPU ticks, context-switch
deltas and I/O across each three-second idle interval with 0/1/2/3 clients;
RSS is 21.0–23.1 MiB. Forty remote operations converge during native pan/wheel
input. Input callback distributions are explicitly broader than the concurrent
interval; the busiest client's event p99 is 25.72 ms, wheel p99 0.00385 ms.
These are processing proxies, not physical latency or an artist-feel verdict.

The 10,000-shape/100-note board completes 100 authoritative edits at p50 34.04 ms,
p99 42.80 ms, maximum 58.37 ms; native rejoin confirms revision 101 and 10,100
annotations. The authority rewrites bounded metadata per operation: the run
records approximately 219 MB of writes, so this is a measured foundation rather
than an append-log scaling claim. Rejoin across the 101-revision gap transfers
one 2,168,647-byte metadata snapshot in 14.50 ms with no durable replay.
CAS uploads 4,561,776 bytes at 39.92 MiB/s on loopback and reports an existing
identical hash on the next begin. Hex framing doubles payload wire cost; neither
this throughput nor the latency is a physical-LAN benchmark.

After removing only the owned fixture original, a fresh native client remains
Connected at revision 42 with all 1,000 objects and an explicit unavailable-source
diagnostic. Its measured three-second idle interval has zero CPU ticks, zero
main/shared-worker context switches, no I/O and zero redraw frames. GPU-driver
background thread context switches remain visible in raw receipts and are not
misrepresented as application wakeups. Final paired local receipts remain to
be read before declaring local pay-for-play acceptance.

No Windows desktop run, physical network bandwidth, low-end-memory guarantee,
authentication or internet-exposure safety is claimed. The supported collaboration
scope is an explicitly enabled trusted LAN, with current PNG/JPEG limits unchanged.
