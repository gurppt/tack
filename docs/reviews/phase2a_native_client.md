# Phase 2A native backend and optional client review

Status: **PASS within the scope below**, after reading the final source, the full
`check-final-04.log` gate and the strengthened `native-02` evidence. No concrete
unresolved blocker was found in this scope. The earlier `native-01` run is only
diagnostic evidence; its semantic convergence hid a complete original-supply
failure and is not accepted as a native image-supply pass.

This reviewer authored protocol DTO/framing and the native interaction harness.
The independent implementation review therefore covers the separately authored
client/cache workers, native/core backend integration and server publication/CAS
paths. It does **not** independently approve this reviewer's protocol or harness.
The separate [protocol/client review](phase2a_protocol_client.md) covers that
protocol. Neither note replaces performance, ordinary-local regression or human
interaction evidence.

## Findings and checked resolutions

All twelve initial findings were checked against the resulting source, not just
implementation reports. Later source/supply findings are included below. Tests
listed here appear as successful in `benchmark-results/phase2a/check-final-04.log`;
native checks refer to `benchmark-results/phase2a/native-02/checks.json`.

| Finding | Checked resolution and evidence |
| --- | --- |
| Full client event queue could lose the authoritative event and disconnection, leaving UI Connected. | Client overflow is retained atomically, transport closes, and drain emits Error plus Disconnected even after a full queue. Native also checks `connected()` before remaining writable. `bounded_event_overflow_is_explicitly_disconnected_at_drain` passes. |
| Source without a CAS binding rejected the entire snapshot. | Private metadata view uses a generated missing-source placeholder; canonical authority remains Embedded. `missing_binding_remains_a_usable_metadata_snapshot` passes. Missing originals remain explicit supply failures, never remote path authority. |
| Valid world coordinates exceeded the camera's smaller range during initial snapshot. | Native first-camera setup clamps to its supported range and records `camera_clamped`, preserving canonical object coordinates. Verified in the Snapshot branch; this extreme-coordinate case was not separately driven through the native harness. |
| Failed application of an accepted command left old authority writable. | Revision gaps or installation errors make the editor read-only and request authoritative reconciliation. `poll_shared` has a disconnected fallback. Source inspected; normal restart/rejoin succeeds natively. |
| Private source paths accumulated after source removal. | `ProductAssets::sync_document` retains only live source/revision descriptors. Accepted edits and snapshot replacement call it. Source inspected; existing supply revision/churn tests and relink/undo/rejoin native sequence pass. |
| Each rejoin detached decoder workers and created overlapping worker sets. | Existing ProductAssets receives `set_board` plus `sync_document`; workers are created only on first shared installation. Source inspected; the native harness exercises restart and multiple F5 rejoins. |
| Remote changes left an active local edit computed from old geometry. | Foreign acceptance visibly cancels active gesture/note/name state and queued unsent operations. Native active-drag conflict passes: mouse release produces no stale durable geometry. Its incoming operation is from an owned protocol peer, while the gesture is actual native pointer input. |
| Own queued requests all used one base revision, dropping later admitted imports. | Native submits one durable operation at a time, then rebases queued own requests after acknowledgement. Core queue remains bounded. Exact repeated-import records are normalized only for the existing import batch shape. Native distinct-path/same-bytes import yields two objects and CAS deduplication; same-path double import shares one source/asset and retains two objects. |
| Shared storage polling skipped supply cleanup and retained staged originals. | Authority updates synchronize supply; requests copy needed payload handles to the worker and release staging; snapshot/refusal/remote cancellation clear staging. Source inspected. Local storage/history remain their existing branch; `shared_commands_wait_for_authority_and_offline_refuses` and `shared_queue_is_bounded_and_disconnection_discards_unsent_requests` pass. |
| Unrelated command could forge a different source binding at the same revision. | Server binding deltas must match explicit AddSource/SetSource, and forward replacements exceed persisted high water. `binding_cannot_change_without_source_replacement` passes. |
| Post-rename sync failure left disk authority ahead of memory and omitted published CAS bytes. | Published authority advances then becomes poisoned until safe reopen; subscribers disconnect. CAS accounting changes immediately after rename. Both post-rename fault-injection publication tests pass. |
| Asset cancellation reused Ready(size zero), and Unicode reasons exceeded the byte bound. | Server emits explicit AssetCancelled and bounds reason truncation on UTF-8 boundaries. Protocol/asset tests pass; paths inspected. |
| Oversized Accepted reply or future snapshot could fail after authority commit. | Authority preflights encoded Accepted and joinable Snapshot before publishing. Source inspected; wire size bounds are tested. No separate near-limit native undo case is claimed. |
| Undo followed by reconnect lost the source revision high water. | Persisted Snapshot high water reaches client and seeds core editor's future replacement floor. Protocol validation covers live revisions. Native relink to revision 2, undo to 1, F5 rejoin and relink to fresh revision 3 pass. |
| Unverified cache filenames were exposed to codec jobs. | Shared sources are explicitly deferred until matching full-binding AssetReady. Deferred Job construction returns before any IO/codec admission. `deferred_shared_source_admits_no_codec_or_io_until_verified_supply_arrives` and `delayed_verified_shared_job_cannot_publish_into_a_new_deferred_revision` pass. |
| Metadata snapshot storage sidecars made every original cache admission fail. | Cache recognizes only exact generated regular zero-byte sidecars, bounds snapshot/lock counts and retires only owned files after respecting the lease. `snapshots_and_storage_sidecars_allow_verified_original_admission` covers eight cycles; `snapshot_retirement_respects_active_leases_and_rejects_nonempty_sidecars` preserves active/unowned data. Native-02 proves all three clients actually display originals. |
| Server startup removed arbitrary filenames ending in `.upload`. | Startup first requires a regular file and validates the exact SHA token before deleting owned interrupted staging. `cas_startup_only_removes_recognized_regular_uploads` verifies recognized cleanup, unknown filename preservation/refusal, directory preservation and symlink target preservation. Independently read this final server change. |
| Adjacent authority startup cleanup also selected `.pending` by suffix alone. | Independently read the final strict nonzero WireId.pending and regular-file checks before deletion. `pending_cleanup_preserves_unknown_and_nonregular_paths` covers owned cleanup and unknown filename/directory/symlink preservation. This narrow change follows the fourth gate; its execution evidence belongs to the final serialized server gate. |

## Native evidence

The isolated harness used Linux X11 `:99`, three actual simultaneous native
shared clients and a fourth independent local window. It owned its small images,
server, profiles, windows and subprocesses. Its 32 checks passed in 17.56 seconds.
Evidence includes native keys/pointer edits A to B/C and B to A/C, shared undo and
redo, conflicting undo refusal, a single-transaction drag, note text, visible
server loss, rejected offline edits, server restart, explicit rejoin, relink,
source undo/high-water restoration and multi-file imports.

Every joined client capture has 180,000 fixture-colored image pixels. Converged
captures have 84,875 / 84,875 / 84,392 pixels for A/B/C. The final captured B view
was visually inspected and contains actual red and purple image content, note
text and annotation geometry. All three final reports show zero errors, missing,
unavailable and foreign sources, five recognizable images in the last frame,
and non-null first recognizable image times (483.28 / 477.45 / 468.77 ms).

All three native reports converge to revision 15, nine objects and four sources,
matching persisted server canonical SHA-256:
`2442195e2c628db1f394091f24483ccfbce161bb5be6ed1149b2b181d36d011a`.
The alongside local board reports no shared backend and remains byte-identical.
Raw checks, captures, client JSON and provenance are retained under
`benchmark-results/phase2a/native-02/`.

Recorded client executable SHA-256:
`a38d5294e7ad528da991c3f8243166fefb8a432eed69e572ec2ca728b2e8d5c7`.
Recorded server executable SHA-256:
`e4d1c5174f35e8e932b0e02bce0d8370aa0586593f4db97511149f882fe7a2ee`.

The full fourth quality gate passes workspace tests/documentation, formatting,
Clippy and cargo-deny (`advisories ok, bans ok, licenses ok, sources ok`). This
includes the final deferred-supply, cache-sidecar and CAS cleanup regressions.
This note does not claim a Windows desktop run, physical LAN bandwidth, physical
input latency, artist feel or acceptance of separately measured performance and
local-regression budgets. The explicit shared mode targets a trusted LAN without
TLS/authentication. Ordinary local use creates no shared backend; cache and
worker activity stay off the input/render thread.
