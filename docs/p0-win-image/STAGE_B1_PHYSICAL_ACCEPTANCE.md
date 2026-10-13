# Native Windows dialogs — B1 physical evidence, continuation authorized

User report in the persistent main `report/` folder:
`report-e9dc359416593e3d4f4b2ab3677f4921.zip`, SHA-256
`397b4c4b6bc62740ff802a4e216e49896acbbac84f6851f1d0d4e8ec7b967971`.
Read directly, no extraction/execution. Four unique logs: two Tack processes
(61 and 519 events), one server (112), one launcher (8), all code ddf7ded.
Original file stays at the user-designated location; `/report/` is now excluded
from Git to prevent accidental disclosure. Private compact evidence is in
`benchmark-results/p0-win-image/physical-win10/b1-acceptance-evidence.json`.

## Confirmed on Windows 10

Four COM selections succeed (Open, two Save As, Share); no native error. Save As
uses the verbatim initial-directory hint both before and after first save.
Three save acknowledgements succeed, dirty clears, ~20–22 ms worker time for
this actual small board. User confirms Save As and shared-board creation work.
No inference about large-image saves or physical focus/long-path/UNC coverage.

Windows server PID 19476 binds wildcard IPv4 port **7337**, writes readiness;
host reads ready after **41.48 ms**. The host's client receives snapshot revision
0 and reaches Connected. Server exits via managed shutdown code 0 when the host
closes; Tack and diagnostic launcher return normally. No crash established.

The logged Protocol refusals are identifiable by exact SHA-256 of known public
error strings: one Windows connection reset (`os error 10054`), other EOFs
(`LAN I/O: failed to fill whole buffer`). They occur after short-lived transfer
sockets close and do **not** establish incompatible protocols or failed publish:
the following host Hello receives a valid snapshot. This is noisy disconnect
classification, not proof that the Linux guest reached this server.

## Separate unresolved Linux guest reachability

User screenshot: `Join failed: LAN I/O: No route to host (os error 113)`.
Host Windows, guest Linux, both windows remained open; invitation endpoint was
not retained. Installed Linux `bin/tack --build-info` reports **768b921**, dev.1,
**protocol 4**, matching B1. A transport connect failure occurs before version
negotiation; this error is not evidence of build incompatibility.

Code-supported hypothesis: Windows address discovery chooses the first Preferred
IPv4 from Get-NetIPAddress, without filtering virtual/VPN/link-local adapters
or verifying reachability from the guest. Wrong-interface selection is possible,
but not confirmed without the actual invited IP. Route/firewall/isolation also
remain possible. No automatic firewall changes, peer scan, host kill or port
7337 probe/repurposing performed. Existing Linux listener PID 17206 is preserved.

## Continuation

User explicitly authorizes autonomous continuation of following stages. Native
fix branch 05c556a is fast-forwarded **locally** into dev after fetching unchanged
origin/dev 9b1ce10; all untracked user files remain intact. No push or public
release. Proceed with C and bounded D; keep remote guest access unvalidated.
This does not waive any final Windows/large-image acceptance or release veto.

References: [Linux connect](https://man7.org/linux/man-pages/man2/connect.2.html),
[Microsoft Get-NetIPAddress](https://learn.microsoft.com/en-us/powershell/module/nettcpip/get-netipaddress).
