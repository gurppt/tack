Tack — P0-WIN-IMAGE-CORE-02-B — PRIVATE native-dialog candidate
This tests Stage B ONLY. Large JPEG/PNG limits and import error UI are not fixed.
Do not replace your usual installation or use your only production board copy.

1. Extract ALL files into a new local writable directory, e.g. Tack test é.
2. Keep Windows PowerShell policy restrictive. No admin or policy changes needed.
3. Double-click Start-Tack-Diagnostics.cmd. This uses an isolated profile.
   Native file dialogs do not call PowerShell. Optional diagnostic version lookup
   and the separate clipboard feature may still call PowerShell; neither is the
   picker. The diagnostic disk quota now uses the Windows API directly.
4. Create a disposable local board. Ctrl+S must open Save, with Untitled.tack
   proposed. Save into a folder with spaces/é/Unicode, reopen the file, Ctrl+S.
5. File menu: Open, Import (select several small JPEG/PNGs), Save As, Save to
   Local, Save Original As, Export selected/all sources (folder), Snapshot.
   Also try import/export keymap, preferences and profile if available.
6. For each picker try Escape/Cancel at least twice, then success. Check that
   Tack gets focus back and the picker never remains hidden or blocks closure.
   Check overwrite confirmation, filters, proposed filenames, initial directory.
7. Try a long local path (>260 characters), and network share if available.
   Note precisely which flow/length failed; existing Tack descriptor limits stay.
8. Start Sharing to a NEW disposable filename; report any error AFTER the
   picker. Do not stop the existing server or change router/firewall settings.
9. Close Tack, then double-click Export-Diagnostic-Report.cmd. Explorer opens
   the report folder. Compress it to ZIP and return it with action/outcome notes.
   Nothing is uploaded automatically. No native crash dumps are enabled.

Diagnostic profile/log root:
%LOCALAPPDATA%\TackDiagnostics\P0-WIN-IMAGE-CORE-02-B
Your normal profile and the previous diagnostic sessions are separate.
3 GiB free disk required. Logs are bounded/privacy-filtered (no pixels, notes,
raw source paths, invites, clipboard contents). Normal tack.exe leaves logs off.

Expected known limits: JPEG/PNG only; still 64 MiB per imported source; giant
progressive/multiscan JPEG unsupported in bounded path; PNG native huge tiles
remain opt-in. A successful picker is NOT proof of a successful file write:
check the created file, reopened board and exported source contents.

Gate B: actual physical Windows 10 required before broad imaging changes.
This candidate is NOT a public release and does not claim Windows+HD resolved.
