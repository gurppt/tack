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

## B1 corrective candidate

**Reproduced:** the same real-directory Save probe cancels normally using its
DOS name, but fails with `0x80070057` using `Path::canonicalize()` before this
fix. Receipt: `wine-verbatim-before/results.json` (Wine, not physical Win10).
This matches the field failure and canonical BoardLease path flow; the original
private raw Windows path was deliberately not logged, so exact equality of
the user's cause remains to be physically confirmed.

Only the Shell-facing directory hint is normalized: ordinary canonical drive
and UNC names become equivalent DOS/UNC parsing names, preserving UTF-16.
Storage paths/leases stay canonical. Device/volume namespaces, traversal,
trailing dots/spaces and reserved DOS names are left intact to avoid silently
redirecting the dialog to a different directory. Such unsupported Shell paths
can still fail with an explicit error. No global long-path policy is changed.
Errors now distinguish parsing the initial directory from SetFolder; opt-in
diagnostics record only path class/hash, Unicode and verbatim-prefix presence.

After: **9/9** isolated native probes pass, including the previously failing
canonical-directory case, repeated same-process cancellation and the owned
modal cancellation pump; **7/7** Windows facade unit tests pass (DOS/UNC,
Unicode including unpaired UTF-16, long names and ambiguous-path protections).
`wine-folder-after/` contains exact results and cleanup proof. No real desktop,
GPU or live server was used. Windows workspace Clippy passes `-D warnings`;
the final launcher namespace/checks and package provenance are recorded at
handoff. B1 uses a fresh `TackDiagnostics/P0-WIN-IMAGE-CORE-02-B1` profile;
previous B and normal profiles remain intact.

**Physical retest required:** Save As after first save and after reopen; Share
Board past the picker, native focus/cancellation/closure, UNC and long paths.
The 64 MiB import cap, JPEG/PNG decode limits and LAN design remain unchanged.
No public release, tag, installed binary replacement or dev integration has
been performed. C/D remain gated by the master's physical Win10 requirement.

Windows namespace reference: [Naming files, paths and namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file).

References: [WM_ENABLE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-enable)
and [modal dialog ownership](https://learn.microsoft.com/en-us/windows/win32/dlgbox/about-dialog-boxes).
