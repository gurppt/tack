# Native dialogs — physical Win10 follow-up

Physical report `report-3b2cf323ad7651e55be8916172381a2a.zip`, SHA-256
`bf0d1b457ead22a22a1e31a64d702138260ee02d325557b546e47e02a9294b9f`,
tests candidate b2193cf. Exact user archive is preserved in the main repository's
private `benchmark-results/p0-win-image/physical-win10/`; it is not committed.

## Confirmed observations

One application session, 822 events, normal return and launcher exit 0. Eight
picker attempts: five successful selections, three failures. First Save As,
Save to Local, source exports and snapshot export select successfully. Save As
after a saved board and two Share Board attempts fail with `0x80070057` at
`initial directory`. No host spawn/ready event follows. This is still a picker
failure, not evidence of a server or LAN crash. Physical Gate B has **not passed**.

The user reports the big sources still do not import. The dialog-only candidate
retains admission/decoder limits; this archive does not identify each source's
format/bytes or first failed import stage. Do not infer a per-file diagnosis.

## Separate modal shutdown hazard

Code review identified a possible Windows UI shutdown deadlock: COM's modal
worker synchronously reenables its owner while UI teardown joins that worker.
An isolated Win32 owner probe on Wine reproduces the blocking control (owned
18-second timeout); retaining the UI pump returns in 159 ms and reenables the
owner. This is **not an observed physical Tack/system freeze**. Receipts:
`benchmark-results/p0-win-image/wine-owner/results.json`.

The Windows exit guard cancels an active native picker and continues the owner
message pump until the worker finishes, draining results without starting new
saves/imports. A chosen path arriving after cancellation is dropped. No idle
timer, extra thread, storage/protocol change or Linux exit policy is introduced.
Full Windows workspace/all-targets/all-features release Clippy passes with
`-D warnings`; physical modal close/focus behavior still needs validation.

The initial-directory HRESULT is investigated separately. Canonical board
paths can use Win32 verbatim prefixes that are not necessarily Shell parsing
names; the prior log does not distinguish Shell parsing from SetFolder.

References: [WM_ENABLE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-enable)
and [modal dialog ownership](https://learn.microsoft.com/en-us/windows/win32/dlgbox/about-dialog-boxes).
