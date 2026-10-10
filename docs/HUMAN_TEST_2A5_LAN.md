# Two-computer LAN regression — Phase 2A5

Status: **PENDING physical acceptance**. Three native clients on a single host,
GPU automation and Windows cross-compilation do not certify this test.
Record build SHA, OS/GPU, network/firewall conditions and concrete artist friction.
Keep the app, server, JPEG decoder, About and icon assets together. All peers
must use protocol-major-2 builds. Normal steps require no terminal.

1. Open/import a local board. File → Share Board creates `name-shared.tack` and
   transitions the current window. Verify the original local file stays available
   and sharing failure leaves it open.
2. Read/copy the invitation, join from B, verify brief COPIED feedback.
3. A holds/moves/rotates one image while B draws a scribble. Both commit.
4. A holds a context menu while B moves a different object. Menu stays usable.
   Delete its exact target remotely: actions must refuse/close gracefully.
5. A starts manipulating one image. B sees its red outer frame and cannot steal
   it. Release/cancel, then B manipulates it. Test abrupt client loss/reclaim.
6. Disconnect B. An offline banner must appear before an attempted edit, and the
   status remains bottom/red. Silent link failure follows TCP detection.
7. Save to Local with a fresh name. Verify independent editable local document,
   images intact and no reconnect sidecar. If an original was evicted and is
   unavailable offline, reconnect/download it first; no partial board is published.
8. Reconnect the original shared incarnation; offline edits must not be merged.
9. Move an already rotated image with snap enabled; inspect jitter and release.
   Disable snap and hold Shift for move/resize/rotate.
10. Rotate with snap to 0°, 90°, 180°, 270° and nearby 15° steps.
11. Toggle toolbar repeatedly through menu/keymap at all placements. Default dock
    is centered; dragged offset persists. Add/remove/reorder via two-column editor.
12. Duplicate image, Note, Rectangle and mixed/group selection; undo and compare.
    Verify fresh copies and unchanged originals/source byte count.
13. Double-click a shortcut, capture it, Escape cancel, reassign a collision.
    Old action becomes unbound. Check tool Normal/Hold and released restoration.
14. File → Close Board: empty window stays open. Hosted shutdown checkpoints and
    reaps the server; reopen shared copy to verify the final accepted state.
15. Invite/link readability and online/connecting/offline bottom colors at native
    DPI. Verify copied/capture feedback settles without idle flicker.

Also test B → Num7, B → Num8, then jump between local views without shared
revision changes; NumLock must be on for digit shortcuts. Top-row digits work
too, and all ten view actions are remappable. Test annotation selection filter,
Note Enter / Shift+Enter, Rectangle reverse/tiny drags, colors/stroke defaults,
frame title wrapping/color/global integer title size, and z-order shortcuts.
