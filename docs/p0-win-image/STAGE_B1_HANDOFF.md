# P0 native dialogs — B1 private Win10 retest

## Outcome and remaining gate

The first physical B report demonstrates five successful native selections and
three failures at the initial directory (`0x80070057`). Saved-board Save As and
Share Board are affected. No server spawn/ready follows the failing Share
attempts; no application crash is established. The exact user ZIP is retained
byte-for-byte, SHA-256
`bf0d1b457ead22a22a1e31a64d702138260ee02d325557b546e47e02a9294b9f`.

B1 corrects a matching isolated reproduction: Windows canonical `\\?\` DOS/UNC
directory hints become equivalent Shell parsing names, without changing storage
paths. It also keeps the owner's UI message pump alive during native-dialog
cancellation at shutdown. Details and evidence boundaries are in
[STAGE_B_WIN10_FOLLOWUP.md](STAGE_B_WIN10_FOLLOWUP.md).

**Physical Gate B is pending B1 retest.** Import/decode/LOD limits, Embedded save
format and LAN architecture remain unchanged. The supplied trace cannot assign
a first rejection reason to each large user source. Prior Stage A valid fixtures
prove the 64 MiB admission rejection; they do not prove these actual files have
the same cause. No giant-image support or online stability is claimed here.

## Frozen private artifact

Main repository directory: `bin/P0-WIN-IMAGE-CORE-02-B1/`.

- ZIP: `Tack-P0-WIN-IMAGE-CORE-02-B1-windows-x64.zip`, **15,290,120 bytes**.
- SHA-256: `6e9313a8982a54b990451279b9220033787f9761e94b2c56e81285cb172606cd`.
- Compiled source: **ddf7ded42751fbd8465fe30ac3f453fd11cbb7c0**, clean tracked
  tree, branch `fix/windows-native-dialogs-p0` (baseline dev **9b1ce10**).
- Shutdown guard: **fce94a9**. Canonical-directory fix: **ddf7ded**.
- Package BUILD.json: 131 hashed payload files, 132 ZIP members; all contents,
  byte lengths and hashes verified. Packaged app/server/updater `--build-info`
  independently report the compiled SHA and protocol 4 under Wine.
- Static JPEG helper/NASM SIMD provenance and decoder hash preserved; no new
  external dependency version or runtime DLL import. Existing authored PNG
  assets are copied, not regenerated. No normal installed binary is replaced.
- New profile: `%LOCALAPPDATA%\TackDiagnostics\P0-WIN-IMAGE-CORE-02-B1`.
  Normal and previous B profiles remain untouched.

The report-only commit following packaging does not change the frozen binary
identity. The branch remains private/unintegrated; dev/main, public release and
tags are unchanged. The earlier tested B ZIP and manifest are preserved alongside
this handoff before retiring the extra temporary worktree.

## Checks and resource cost

Root `benchmark-results/p0-win-image/` contains bounded private receipts:

- `wine-verbatim-before/`: ordinary DOS hint passes, canonical hint fails with
  `0x80070057`; before/after witness is a real COM execution, not text matching.
- `wine-folder-after/`: **9/9** native probe cases, **7/7** facade Windows unit
  tests pass. Includes repeated cancellation and owner message pumping.
- `wine-app-cancel-tests/`: **3/3** worker cancellation Windows tests pass;
  83 unrelated tests filtered explicitly, not counted as passing.
- `clippy-b1-{linux,windows}.log`: full workspace/all-targets/all-features release
  Clippy `-D warnings` passes on both targets. `cargo fmt --check` and
  `git diff --check` pass. Windows app/launcher/server/updater release build passes.
- Prior B Linux suite: **497 passed, 0 failed, 21 existing manual/native/GPU
  ignores**. Not rerun wholesale for this Windows correction; final Linux lint
  checks the unchanged exit delegation. No test was weakened or newly ignored.
- `package-b1-verification.json`, `wine-b1-package/`: manifest/ZIP/DLL/identity
  checks pass. Wine is not physical Win10, GPU/LOD, LAN or focus acceptance.

| Stripped executable | First B | B1 | Change |
|---|---:|---:|---:|
| tack.exe | 26,139,136 B | 26,140,160 B | +1,024 B |
| diagnostic launcher | 1,764,864 B | 1,764,864 B | 0 |
| server | 3,591,680 B | 3,591,680 B | 0 |
| updater | 4,360,192 B | 4,360,192 B | 0 |

Compared with the pre-dialog forensic control, app delta is +117,248 B (~0.45%).
No extra idle worker, polling timer, network work or decoder allocation is
introduced. The cancellation timer exists only within Show. Directory UTF-16
handling is bounded to 32,767 units and only runs for an explicit picker.
Ordinary Linux baseline and its measurement limitations are recorded in
[STAGE_B_NATIVE_DIALOGS.md](STAGE_B_NATIVE_DIALOGS.md); physical Windows idle
private memory/VRAM remains unmeasured. Do not extrapolate Wine CLI checks.

Builds use two jobs, debug=0 and no incremental cache. About 10.73 GiB remains
before retiring newly owned build outputs; existing root caches are untouched.
Only the mission-owned extra worktree will be removed, after preserving its
committed source branch, user archive and private artifacts. The earlier brief
reserve breach and owned cleanup have separate receipts; it is not concealed.

## User retest and continuation

Extract the whole ZIP in a new local writable folder; launch
`Start-Tack-Diagnostics.cmd`, keep ordinary privileges and restrictive PowerShell
policy. Use disposable board copies.

1. Save a new board, then **Save As after this first save**, reopen and retry.
2. Try **Share Board to a new file**; distinguish picker cancellation/failure
   from any error after selection/server startup. Do not stop an existing server.
3. Repeat Open/Import of small JPEG/PNGs, exports, cancellations and native
   close/focus checks; try Unicode, long paths and UNC when available.
4. Close Tack; the diagnostic exporter can retain a report locally for analysis.

The master brief §5 requires actual physical Win10 evidence before broad C/D.
After Gate B passes: implement precise import failure UX separately, then audit
and measure bounded large JPEG/PNG admission/rendering per NEXT_GATE_C_D.md.
The GitHub organization addendum applies after this active mission's code/tests/
report work; no organization changes or push have been performed. Root user
untracked material and existing diagnostic worktree remain preserved. Main's
marketing README and all release/branch settings remain untouched.
