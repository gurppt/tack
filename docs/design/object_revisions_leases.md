# Object conflict clocks and manipulation leases — 2A5

The authoritative server retains one durable, monotonic global revision for
receipts, persistence and reconnect. An edit's base is captured when queued;
a transform captures it at gesture start. A newer board revision alone is
not a conflict. `CommandScope` resolves the actual targets and metadata
barriers; `ObjectClock` refuses a base older than a touched target.

Clock storage has one u64 stamp per live object, up to 4,096 deletion stamps,
and two conservative dependency/order barriers. Evicting a tombstone raises
the durable floor so old commands cannot resurrect forgotten deleted IDs.
Legacy authorities seed their clock at their current revision. Future bases,
bases below the retained floor, missing references and invalid commands fail
before persistence. Duplicate operation receipts are checked first.

Transforms on separate images and fresh scribbles coexist. Source/asset
replacement stamps the referencing images. Groups touch their members plus
the descriptor barrier; index-based ordering reads the order barrier. These
coarse metadata barriers are deliberate, bounded conflict protection; no CRDT
or offline merge is introduced.

Undo checks the inverse targets against both its captured base and accepted
history revision. Refusal preserves history. Rearming previous undo requires
an exact adjacent history boundary and matching targets; interleaving may
conservatively refuse an older undo rather than erase another user's change.

Manipulation reserves all targets atomically. Ownership is the server
connection, not a client-supplied identity. Lifetime is five seconds; caps are
256 targets per connection and 1,024 per board. Reservations are transient,
never document or persisted authority data. Commands and undo inverses cannot
mutate another connection's live targets. Disconnect, duplicate-client
replacement and outgoing queue failure release ownership; incoming activity
lazily expires stale leases. The server has no maintenance heartbeat.

The client waits for grant before preview/commit, renews only an active
reservation every two seconds, and releases on cancellation or commit receipt.
An acknowledged operation cannot lend its lease to a subsequent gesture.
Foreign targets have a pixel-red outer outline and cannot begin manipulation.
Client expiry/renewal deadlines exist only while reservations are present.
Queues remain bounded: native document requests 32/8 MiB, control requests 4,
client events 8, server outgoing 64 messages/8 MiB. Exhaustion disconnects safely
and requires snapshot reconciliation; it does not grow an unbounded backlog.

Unrelated accepted edits preserve camera, selection, menus, drafts, tool,
key-capture and modal state. Only intersecting object edits invalidate a
manipulation or existing-object text draft. Menus keep their geometry; action
execution checks fresh selection/targets, so deleted objects cannot be used.
Reconnect snapshots replace authority and intentionally cancel transient edits.

This is wire protocol major 2. All clients/server in a shared session must use
the same version. Old major-1 peers are refused before parsing their body.
Authority JSON remains version 1 with a compatible optional clock field.
