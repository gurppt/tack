# P0-WIN-IMAGE-CORE-02-B — private native Windows dialog candidate

**State: implemented, cross-compiled; physical Win10 Gate B pending.** This is not a Windows+HD release. C–G remain gated; no JPEG/PNG cap, LOD policy, save format, LAN protocol, clipboard policy or production binary is changed by this patch.

## Smallest safe fix and boundaries

`native_files::pick_with_options` now calls Windows Common Item Dialog directly, on the existing `tack-local-operation` worker. It no longer writes or runs a `.ps1`, spawns a shell or requires PowerShell. Existing Linux Zenity path and separate Windows clipboard `-Command` are retained. File picker cancellation returns an empty selection, while COM failures retain static stage, category and HRESULT in the UI and opt-in logs.

New leaf crate `tack-windows-dialogs` confines documented unsafe Win32 FFI; core/app retain workspace `forbid(unsafe_code)`. It uses the **already locked windows 0.58.0**, with COM/Shell/FileSystem feature modules. No new external crate version, toolkit, codec, runtime DLL bundle or daemon. The safe facade rejects invalid requests/foreign HWNDs, preserves UTF-16, bounds selection count/path result bytes and releases COM interfaces/task strings/STA on every return.

UI passes a cloned `Arc<Window>` to the explicit worker, retaining HWND lifetime. Scoped worker TLS propagates the owner to nested export pickers without a global cross-window owner. `Show(owner)` supplies native modality; physical focus/DPI/parenting must still be tested on Win10. STA initialization is balanced even for S_FALSE, and all COM objects die before CoUninitialize. A 100 ms cancellation timer exists **only during the blocking modal Show**; its same-apartment callback closes the dialog, then the guard removes timer/TLS before borrowed cancellation state expires. No unsafe COM pointer crosses threads. No forced kill timeout can leave a native modal orphan; Shell-provider stalls outside its message pump remain a physical test limitation.

Diagnostics' Windows free-space quota uses native GetDiskFreeSpaceExW so the private launcher can start if PowerShell is absent. Optional diagnostic version lookup is still independent and can fail without blocking picker access. Private diagnostic profile moved to `TackDiagnostics/P0-WIN-IMAGE-CORE-02-B`, preserving both normal and previous diagnostic profiles. Package tooling accepts an explicit mission/readme label while preserving its previous defaults.

Alternative considered: shell `-Command`/policy bypass keeps a policy-dependent executable and does not fix the proven architectural boundary. Broad GUI toolkit/helper process would add dependencies/lifetime/timeout failure paths without a need. Native direct COM adds only operation-scoped work. Windows API references: [Common Item Dialog](https://learn.microsoft.com/en-us/windows/win32/shell/common-file-dialog), [STA initialization](https://learn.microsoft.com/en-us/windows/win32/learnwin32/initializing-the-com-library).

## Verification receipts and honest limits

Main repo `benchmark-results/p0-win-image/` holds compact receipts, not committed private source files. Before: 2/2 source-contract witnesses failed. After B: native-picker witness passes; the UI import-error witness **still fails, deliberately left pending Stage C**, not suppressed or counted as passing. Real Windows policy-failure baseline comes from the 10 field attempts in Stage A.

- Linux workspace all-features release tests: **497 passed, 0 failed, 21 ignored**, including the existing storage/lease/recovery and loopback LAN tests. Ignored cases remain the repository's explicit native/manual/GPU/resource tests; no skip was added or weakened.
- Linux release build and Windows GNU release build of app/diagnostic launcher/server/updater/native probe succeeded.
- Initial Wine/private Xvfb native probe: **7/7 passed**, including pre-cancel, invalid owner, Open/Save/Folder/multiple cancellation, UTF-16 proposed filename and native disk quota with no installed PowerShell. Owned prefix was placed in tmpfs and removed; no real desktop interaction or production server changes. Repeated same-process probe, Windows unit tests and final lint/package receipts will be recorded below after final verification.
- Native Windows-target Clippy for the leaf facade passed `-D warnings`. Full workspace Windows/Linux lint and final manifest verification are the remaining build checks before handing over the private ZIP.
- **Not established**: actual Windows success selection, focus/DPI, path >260 chars, restrictive-policy file artifacts/reopen/export, UNC provider behavior, Windows driver/memory performance, Sharing after picker. Wine is not physical Win10 proof.

## Ordinary Linux local control

Same 20 unique RGB sources (10 baseline JPEG / 10 noninterlaced PNG, each 600×400) and 20 objects, 800×600, private Xvfb/llvmpipe, three fresh processes per side, logs disabled. Control binary is the preserved a9260b9 forensic build, whose product code is dev 9b1ce10 plus the selectively replayed diagnostics. Filesystem/driver caches are warm or uncontrolled; these are **not OS-cold-start or real-GPU measurements**. The after executable includes B; active compilation adds measurement noise.

| Median of 3 runs | Before | After |
|---|---:|---:|
| Startup → mapped window | 123.46 ms | 124.79 ms |
| Settled PSS | 74.97 MiB | 75.19 MiB |
| GPU texture accounting | 3.25 MiB | 3.25 MiB |
| CPU callback p50 | 10.26 ms | 10.60 ms |
| CPU callback p95 | 13.52 ms | 14.17 ms |
| CPU callback p99 | 22.52 ms | 19.68 ms |
| Ctrl+S → observed ack (external harness) | 203.23 ms | 207.50 ms |
| Settled threads | 11 | 11 |
| CPU ticks / IO delta, settled 3 s | 0 / 0 | 0 / 0 |

No meaningful ordinary regression demonstrated within these short/noisy samples; this is not proof for all hardware. Both sides zero errors, unchanged 3,403,776-byte renderer allocation accounting. Callback timing includes submission/present wait on software rendering; no claim of input latency or physical VRAM. Linux PSS includes driver/library mappings, not just heap. No new idle timer/thread/network activity is observed. Exact Windows idle and stripped-binary deltas require the final candidate/physical check.

## Data safety, provenance, rollback

Patch builds in `/home/gurp/softs/tack-p0-win-image`, branch `fix/windows-native-dialogs-p0`, baseline f702b1e after the separate diagnostic replays. Root dev, installed bin, user assets/boards and server :7337 untouched. The additive Stage A probes/fixtures only touch their marker-owned work directories. No storage schema/protocol change; existing streaming CRC/atomic/sync/lease semantics retained. Final code commit/binary provenance will be frozen before packaging, with a separate later report commit if needed. Rollback is to discard this isolated candidate or revert the B commit on this private branch; the normal installed build was never replaced.

## Physical Gate B handoff

Private package README contains the executable checklist: first Ctrl+S on new board, Open/Import/Save As/Save to Local/source and snapshot exports, keymap/profile pickers, repeated cancellation/success, UTF-16/spaces, long path/UNC where available, correct files/reopen, Sharing to a new disposable file. Keep policy restrictive and privileges ordinary. Use Start-Tack-Diagnostics, close Tack, Export-Diagnostic-Report; normal launch leaves diagnostics disabled. No report upload happens automatically.

The governing brief §5 requires **“actual physical Win10 evidence”**. Inspect that result before broad C/D fixes; if failures remain, isolate them first. See NEXT_GATE_C_D.md for the bounded subsequent scope. No public release or claim of giant-image support is authorized by this artifact.
