Tack P0-WIN-IMAGE-CORE-02-CD — PRIVATE WINDOWS TEST CANDIDATE
Pre-alpha; not a release. Version dev.2 is unchanged; BUILD.json identifies SHA.

Extract this entire ZIP into a NEW writable directory. Do not overwrite the
installed Tack, B1 candidate or its profiles. Keep all executables/artwork together.
Double-click Start-Tack-Diagnostics.cmd. Use only disposable copies of boards.
The launcher uses an isolated profile in:
  %LOCALAPPDATA%/TackDiagnostics/P0-WIN-IMAGE-CORE-02-CD/
The normal tack.exe leaves diagnostic logging off. Nothing uploads automatically.

What changed since the physically tested B1:
- Native COM file dialogs are retained; do not loosen PowerShell execution policy.
- Import failures show bounded filename samples, stage, category and actual reason.
- Eligible baseline JPEG / static noninterlaced PNG above 64 MiB can use existing
  streamed supply. Cumulative decoder reads remain capped at 256 MiB; admission
  reserves header/layout overhead, so usable source allowance is slightly lower.
- Embedded copy progress uses actual copied bytes, without an idle animation.
- Source Info can show a current-revision preview/decode failure.
- Advanced Sharing displays the actual protocol number (4).

Known limits, please do not treat them as solved:
- 1.33/1.478 GB originals still exceed the validated decoder/admission budget.
- Giant progressive/multiscan JPEG, interlaced/animated PNG and some color/precision
  variants have separate explicit restrictions. Ordinary progressive JPEG has its
  existing bounded route; there is no universal giant Photoshop compatibility.
- GIF import/animation is unsupported. Only an independent prototype was studied.
- Default PNG high zoom still uses the existing detail representation. Native
  huge PNG tiles remain an explicit experimental --huge-tiles path.
- Every actual Embedded Save still copies the original payloads. A multi-GB
  storage-generation prototype exists only as an experiment; .tack is unchanged.
- Existing sharing is trusted LAN only; no TLS/auth or giant-asset transport fix.

Physical Win10 acceptance (retain restrictive PowerShell policy):
1. On a local disk path with spaces/accents, create an ordinary 20-image board.
   Save As, edit/Save, close/reopen, Import, Export All, folder selection; cancel
   each dialog repeatedly. Confirm no lingering dialog or changed focus afterward.
2. Import one real baseline JPEG and one noninterlaced PNG >64 MiB but <240 MiB,
   initially one file at a time. Record exact bytes, dimensions and source origin.
   Try Linked and Embedded separately. Verify overview, Actual Size, pan/zoom,
   close/reopen and exported Embedded-original identity (compare SHA if available).
3. Try the files previously rejected by Photoshop export. Record the displayed
   filename/stage/reason, especially progressive or encoded-size restrictions.
   Import a missing/broken source too; a failed batch must not silently look ready.
4. Cancel a large Embedded import and close an active decode. Reopen prior valid
   saved boards. Do not inject crashes into your only original/user board.
5. Separately create a disposable shared board. Leave Windows host open. Join
   from the included-current Linux test candidate or protocol-4 client. Record the
   exact endpoint IP/port WITHOUT the invitation key. Both GUIs staying open does
   not prove routing: prior Linux errno 113 occurred before protocol negotiation.
   Do not kill an existing port 7337 server or alter firewall globally for this test.
6. Close Tack, then double-click Export-Diagnostic-Report.cmd. Review the folder;
   Explorer Compress to ZIP. Reports contain bounded structural logs, not images,
   notes, clipboard, source paths or invitations. Note times and visible behavior.

This candidate is cross-compiled/checked on Linux and smoke-tested under Wine.
Physical B1 validates native Save/Open/Share on the received sessions, not the new
large-image/GPU changes. Those physical Windows gates remain pending.
Bundled tack-jpeg-decoder.exe is pinned static libjpeg-turbo 3.2.0 with SIMD/NASM;
do not substitute a random system codec executable. SHA/provenance: BUILD.json.
