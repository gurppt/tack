# Two-computer LAN acceptance — Phase 2A4

**Pending physical acceptance.** Same-host native automation does not establish
two-computer connectivity, firewall/adapter selection or Windows desktop behavior.
Use `./bin/tack` on the owner machine and the same build on a second computer.
Keep sibling `tack-server`, `tack-jpeg-decoder`, About and icon assets together.
Normal testing below requires no terminal/server command.

1. A launches Tack, opens a local `.tack`, imports a reasonably large image,
   then chooses File → Share Board → Share from this computer → Start Sharing.
   Accept proposed `name-shared.tack`. Keep original local window alongside.
2. A clicks Copy Invite and sends that text to B through an existing channel.
   B launches Tack, chooses File → Join Shared Board, pastes and joins.
3. Both move/flip shared-supported content in turn. Verify convergence and
   independent pan/zoom. Verify fixed green online indicators and original local
   board independence. Note any artist-facing networking jargon or friction.
4. A chooses Stop Sharing. B observes offline/red after disconnect. The stopped
   host copy is inspectable read-only. Close the hosted window; no server should
   remain. Close Tack normally.
5. A reopens the same `-shared.tack` with its companion and original hosting
   profile intact, chooses Put Online. B reconnects with the previous invitation
   (existing remappable Reconnect action; default F5). Verify same board/content.
6. Close the online host normally; confirm B becomes offline and reopening
   preserves the last accepted edit. Test abrupt host loss separately if useful.
7. B tries an invalid invitation, an offline host and Cancel during connection:
   current local board must survive and no pending joined window remains.
8. Interrupt Wi-Fi/cable; attempt an edit/reconnect, restore connectivity and
   reconnect. No offline edit should silently become authority. With no heartbeat,
   silent interruption detection depends on TCP/explicit activity.
9. Reopen/rejoin visited image content and inspect cache reuse; edit then retry
   with a large image. Shared original admission remains 512 MiB/file-cache bound.
10. Record actual OS/GPU, IP/port stability, adapter/firewall conditions, build
    SHA from `bin/BUILD.txt`, steps passed/failed and concrete friction. Keep
    technical failures here/report; subjective toolbar/color comfort belongs in
    `HUMAN_REVIEW_PENDING.md`.

One desktop-hosted board owns default port 7337 at a time. The stored invitation
assumes its host address remains reachable; DHCP/adapter changes may need separate
diagnosis under Advanced. Remote-server alternative and numeric URI parsing
preserve the transport boundary, but this phase does not provide Internet hosting.
