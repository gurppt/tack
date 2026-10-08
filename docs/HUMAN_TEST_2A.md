# Phase 2A hands-on test

Use `./bin/tack` and `./bin/tack-server`; inspect `./bin/BUILD.txt` for the
current commit and checksums. `bash tools/build-test-bin.sh` rebuilds both
without installing services.

1. On the server computer, choose a new private data directory and explicitly
   start `./bin/tack-server --root /ABSOLUTE/PRIVATE/DIR --listen LAN_IP:7337`.
   This foundation is for a trusted LAN, with no TLS or authentication.
2. Publish a local board with `./bin/tack publish /PATH/LOCAL.tack LAN_IP:7337`.
   The JSON result prints its board ID. The original local board is unchanged.
3. On each computer start `./bin/tack join LAN_IP:7337 BOARD_ID`.
   Reuse the same ID. Shared status appears on the canvas and window title.
4. Move an image, create/edit notes and rectangles, change crop/opacity/flips,
   frames/groups and object order. Camera, selection and tools stay local.
5. Undo the latest edit in its originating window. Edit in another window then
   try the older undo: the refusal must remain visible and preserve authority.
6. Stop/restart the owned server. Shared windows remain alive/read-only once
   TCP reports the loss. Press F5 to reconcile; lost edits are never silently
   replayed. Check ordinary local boards stay independent throughout.
7. Import two different filenames containing identical image bytes: the server
   stores one content hash while preserving distinct document source/object IDs.
8. Relink a source, undo, reconnect with F5, then relink again. The server source
   revision floor prevents recycling the revision undone earlier.

Ordinary local Save/Save As and undo remain as before. Shared edits persist on
server acceptance; shared Save/Save As explains server ownership. User-facing
shared snapshot/export and Save Original As are deferred; the reverse streaming
snapshot API is documented and tested. Sources over the 512 MiB shared cache
admission stay explicit placeholders; inherited huge-image/codec limits remain.
Windows native collaboration and artist-feel validation are not claimed by
Linux automated checks.
