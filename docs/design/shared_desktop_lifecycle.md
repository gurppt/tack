# Desktop shared-board lifecycle (Phase 2A5)

File → Share Board → Share from this computer → Start Sharing creates an
independent `name-shared.tack` and transitions the current native window after a
validated connected snapshot arrives. The original local file remains independent
on disk. During connection the original editor/lease remains available; failed
publication, snapshot preparation or explicit cancellation retains that editor.
File → Join Shared Board accepts one pasted invitation. Copy Invite uses the
existing native clipboard worker. Primary panels and ordinary shared titles
show artist-facing state and a readable invitation; Advanced exposes technical
details. Copy briefly shows COPIED, then its one-shot feedback expires without
recurring idle redraw. Shared state always occupies the bottom status strip,
including when the ordinary local status-bar preference is disabled.

## Identity and authority

Share forks document identity once; source/asset/object identity is retained in
that new incarnation. Filename is a convention, never the authoritative ID.
The bounded, versioned `<board>.tack.sharing.json` companion records Board ID,
canonical invitation and optional local hosting identity. Its Board ID must
match the snapshot. Missing/corrupt/mismatched metadata never creates a merge.
Keep the companion beside the file when moving it. This does not change the
sidecar schema. Frame colors now use optional `.tack` schema 5 and active object
leases require wire major 2; older peers fail explicitly rather than misinterpret.

The host profile holds `hosting-identity` and `hosted/<BOARD_ID>/`, containing the
existing server authority and CAS. Reopen on that profile offers Open Offline
(read-only) or Put Online. Put Online reuses committed server authority rather
than publishing over it. The same Board ID and old invitation work with the
same reachable IP/port; address changes, firewall behavior and multi-interface
selection require real LAN validation. Retain the hosting profile/authority
when relocating a hosted board. A copied companion alone does not transfer
hosting authority to another machine/profile.

While online, only existing server acceptance supplies durable editor state.
Offline inspection queues no edits, Undo/Redo, local Save, recovery or merge.
Save to Local explicitly forks identity and authority; general offline
collaborative editing remains deferred.
Relative linked paths are resolved for the new shared snapshot; original payload
handles stream through existing checked publication/snapshot code.

## Explicit process lifetime

Only Share/Put Online starts sibling `tack-server`. Normal local startup creates
no server, IP socket or collaboration worker. The desktop host explicitly
listens on port 7337 on local IPv4 interfaces. The ordinary server CLI retains
its loopback default. One hosted board can own that port at a time; collision
produces a visible failure. There is no daemon, autostart or discovery service.
Address selection is an explicit bounded interface query, including isolated
LANs without a gateway; no periodic network scan is introduced.

Readiness is bounded; publication reuses the existing publisher, and join waits
for a connected server snapshot. The host owns the server's stdin lifetime.
Stop/normal host close closes that pipe, shuts subscribers under the existing
authority lock, streams a hash/CRC-checked current offline snapshot and reaps the
server. Stop work runs outside the UI, with a 60-second bound and explicit error
on timeout/checkpoint failure. Durable server authority remains available on
failure. Stop reloads the read-only snapshot and cannot silently restart hosting.

A waiter is created before server spawn for cancellation/drop cleanup; it is
blocked while hosting and does not poll until shutdown. Normal Stop/abort reaps
on the operation worker. EOF also handles abrupt host loss; application-owned
successful child windows use bounded blocking waiters (maximum 16 per parent)
so closed windows do not remain zombies. Those waiters exist only after explicit
launches. None is a local-startup or recurring UI worker.

## Existing transport boundary

Share on a Tack server under the alternative/Advanced route accepts an existing
numeric server address, using the same publish/join backend. LAN UI is separate
from socket/wire implementation, permitting a later remote transport adapter.
DNS, TLS/auth, public deployment, traversal, relay, cache-management UI and
presence counts are not implemented. No peer count is fabricated. Existing
bounded content cache and validation remain automatic; no startup full scan.

The existing trusted-LAN access model and silent TCP interruption limitations
are described in [the collaboration design](lan_collaboration.md). Physical
acceptance is tracked in [the two-computer checklist](../HUMAN_TEST_2A4_LAN.md),
separately from same-host automation and subjective UI review.

## Disconnect, local copy and Close Board

A noticed disconnection immediately opens one small centered panel with Reconnect
and Save to Local. Editing remains disabled. No repeated nagging timer or offline
merge exists; silent transport loss still depends on OS detection or active I/O.

Save to Local requires a fresh destination. An operation worker streams each
original once into a new private `<target>.assets` folder (64-KiB copy buffer),
validates SHA/size for shared originals or CRC/length for stored originals,
syncs those bytes, then publishes a new `.tack` with relative linked descriptors.
The copy has a fresh DocumentId and no sharing companion. Offline cached display
representations alone are insufficient: unavailable originals refuse publication.
An unpublished failure removes only its owned generated folder. Successful copy
open switches this window to an editable local editor, retaining its camera;
the original shared authority stays independent.

File → Close Board keeps the application alive and opens a fresh empty local
document. Pending workers/saves/authority receipts prevent unsafe closure. Dirty
local boards use Save/Discard/Cancel. Hosted closure checkpoints and reaps its
owned server through the existing explicit shutdown path before replacing the
editor; it never leaves a background collaboration daemon.

The updated physical acceptance checklist is
[HUMAN_TEST_2A5_LAN.md](../HUMAN_TEST_2A5_LAN.md).
