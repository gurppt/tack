# P0-WIN-IMAGE-CORE-02 — Stage A, factual baseline (2026-10-13)

Stage B is the next isolated patch. Gate B requires physical Windows 10 validation before broad C/D changes. This document does not claim the imaging work complete.

## Provenance and preservation

Fetch completed 2026-10-13. Target `origin/dev` remains `9b1ce10c01dc480705450daffc104d80d77efbee`; origin/main advanced only its README. Root dev and installed bin are untouched. Working branch `fix/windows-native-dialogs-p0`, worktree `/home/gurp/softs/tack-p0-win-image`, starts at origin/dev. Existing `diag/windows-lan-save-lod` at 2f3b4c7 is preserved. Its instrumentation commits 6478288/e017482/947b3fe/23161a1/a9260b9 were selectively replayed (d6fb848/0b98985/3a3c8d2/c533855/84974c5), without its later report changes. These are opt-in diagnostics, not image fixes. All root untracked user material, production boards, authored icons and server :7337 remain untouched.

Disk audit: 14 GiB available before this stage. Build only owned outputs, jobs=2, debug=0, incremental=0; maintain at least 10 GiB free. No existing cache cleanup or production-file deletion is authorized by this brief.

## Confirmed: reports read directly

Reports live in the preserved diagnostic worktree's `report/`. `tools/p0_image/inspect_reports.py` reads ZIP members without extraction or execution and de-duplicates each JSONL by SHA256. Private receipts: main repo `benchmark-results/p0-win-image/stage-a/report-evidence.json`.

- 17b9921 and ebac153 contain the same two JSONLs, despite different ZIP SHA256.
- d086f16 contains those same two plus the second Tack/launcher pair. Four unique JSONLs, **two unique Tack sessions**, 520 and 193 records.
- Tack sessions 16c3713a2078cda675b41a8ce7ba072e and 8f028dbb19a006a274c96e0a31a9cba7 both emit `app/return_ok`.
- First has 9 picker attempts (7 Save, 1 Import, 1 ExportSources); second has 1 Save. All ten fail with the captured PowerShell `UnauthorizedAccess`, `running scripts is disabled`, helper exit 1, followed by a generic worker failure.
- First failure 2026-10-12 20:43:36.979 UTC (22:43 Paris); second session failure 21:34:22.910 UTC. Timelines retained in receipt. **No crash established.** Picker failure precedes storage/export and does not prove their operations fail.
- This corpus does not identify the scan/layout or encoded bytes of the user's production rejects. The brief's approximate MB and dimensions remain unverified per-file; no claim that all fail for the same reason.

## Code-supported: first failing boundaries and allocations

| Path | Boundary / working memory | Consequence |
|---|---|---|
| Native File actions | native_files.rs creates `.ps1`, runs powershell `-File` on explicit local worker; output capped 64 KiB, 300 s | Restrictive policy rejects before picker opens. Clipboard uses a separate `-Command`; preserve it. |
| OS drop / Import picker | LocalWorker → ImportRequest.run → canonicalize → source_fingerprint (stat) → image_metadata → encoded gate | Header/variant failure can precede byte-size failure. No thumbnail generated in this worker. |
| Import metadata JPEG | Signature → jpeg_header.read, bounded 1 MiB prefix; axes ≤65,500; >6000×4500/nonordinary requires streamed header | Only 8-bit baseline interleaved RGB/gray large path, ≤16 MiB scratch; giant progressive/multiscan gets a precise worker error, then UI discards it. |
| Metadata PNG | Signature → png_dimensions; axis/pixel/row checks; cumulative budget 256 MiB | Metadata success is not full decode success; noninterlaced rows can later exceed encoded-read budget. |
| Encoded admission | local_import MAX_ENCODED_IMAGE = 67,108,864 bytes | Both Linked/Embedded reject >64 MiB after header. Approx 67 MB is ambiguous, not definitely over. |
| Embedded staging | pin_copy 128 KiB stack copy + CRC; independent writer cursor; 64 MiB/file, 2 GiB/batch, 4 GiB/session | Source unchanged validation follows copy. 1.33+1.478 GB together exceed 2 GiB batch even if per-file cap lifted. Original is not RGBA buffered. |
| UI rejection | ImportUpdate::Failed path/message discarded; final generic rejection count | Proven information loss; precise worker cause unavailable to user. |
| Ordinary thumbnail | representation encoded limit + native_thumbnail MAX_SOURCE 64 MiB; output ≤32 MiB | Must audit independently from admission; cannot simply raise global import constant. |
| Large display | ProductAssets bounded workers/queues → baseline JPEG child scanlines or PNG row path → overview / demand tiles → existing CPU/GPU budgets | Highest ordinary detail edge 2048; current PNG native tile path opt-in `--huge-tiles`; admission alone not rendering proof. |
| Save / reopen | streamed payload ranges, u64 offsets/lengths, atomic staged write, CRC/sync/rename; lease/recovery | Existing whole-container Save recopies embedded originals; no schema change authorized here. |
| Online | independent cache/original cap 512 MiB, 64 KiB hex stop-and-wait | Future offline large admission must not silently raise network limits. Network feature currently empty; optionalization needs an ADR. |

## Fixture and baseline status

Valid deterministic prior diagnostic fixtures exist (8000×4000 L8 baseline JPEG, noninterlaced L8 PNG, seeds and SHA256 in `benchmark-results/dev-diag-winlan/fixtures/fixtures.json`). They are genuine images, but <64 MiB and not substitutes for unavailable Photoshop / 1.478 GB originals. They are reused read-only. No multi-GB artificial padding counts as a supported image fixture.

Prior controlled Linux receipts in `benchmark-results/dev-diag-winlan/native-final/` show JPEG 24/24 native tiles, PNG default 0/0 with 2048 detail, opt-in PNG 24/24, no reported errors, zero CPU ticks/IO over settled 3 s. llvmpipe was isolated, **not physical Win10 GPU evidence**. Prior 50,296,588-byte Embedded JPEG Save p50 ~144–156 ms warm ext4; original copying + fsync dominate. Actual user's large Save remains unmeasured.

Needs current baseline: fresh 20 ordinary image 800×600 control, cold/warm/idle PSS/frame and artifact-size comparisons before/after B. Physical Windows baseline unavailable from this Linux session. Exact VRAM/input latency needs hardware evidence. No unmeasured figure is reported as passing.

## Failing witnesses and bounded next stages

`python3 tools/p0_image/check_contracts.py` fails 2/2 before fixes; receipt `contracts-before.log`. These are **source-contract witnesses**, not a replacement for Win10 policy/COM execution tests. Native runtime contracts will be checked through the Windows implementation/tests and private user handoff.

- B: native COM dialog on the existing explicit worker; parenting, UTF-16, cancel/error, no shell policy. Private Win ZIP, no installed replacement. Await physical Gate B.
- C: structured import stage/reason and bounded failure samples, accurate compact copy progress, privacy tests. Separate from size/codec support.
- D: valid >64 MiB baseline JPEG and PNG import/render path end-to-end; format-specific caps/cancel/mutation, measured ordinary regression. Progressive needs a measured coefficient strategy; no global constant patch.
- E: bounded GIF poster experiment/ADR; no eager animation or extra idle timers.
- F: embedded Save ADR and disposable prototypes, standalone `.tack`, integrity/atomicity preserved; format migration not authorized.
- G: real Win retest includes Sharing **after picker**, no server :7337 mutation or LAN lifecycle redesign.

References: [Microsoft Common Item Dialog](https://learn.microsoft.com/en-us/windows/win32/shell/common-file-dialog), [COM initialization](https://learn.microsoft.com/en-us/windows/win32/learnwin32/initializing-the-com-library).
