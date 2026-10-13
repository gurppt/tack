Tack — optional test diagnostics

1. Extract the entire ZIP into a NEW writable folder. Keep all files together.
2. Double-click Start-Tack-Diagnostics.cmd (Linux: Start-Tack-Diagnostics.sh).
   This uses a separate profile; your usual Tack installation/profile is untouched.
3. Make a disposable LOCAL board. Import your test images with Embed selected.
   Try Save, reopen/edit/Save, then Save As to a NEW unused filename.
4. Try Start Sharing to a NEW filename. Do not stop somebody else's server.
5. Zoom into JPEG and PNG images separately; wait 30 seconds, pan away/back.
   Record whether inputs came from SMB and their approximate dimensions/byte count.
6. Close Tack. If it crashes, simply proceed to step 7 (no command line needed).
7. Double-click Export-Diagnostic-Report.cmd (Linux: Export-Diagnostic-Report.sh).
   A ready-to-send ZIP is created automatically, using STORE (no compression).
   Windows selects the ZIP in Explorer. Review it and send with description/time.
   Nothing is uploaded automatically.

Logs: Windows LOCALAPPDATA/TackDiagnostics/Testing/sessions;
Linux XDG_STATE_HOME (or HOME/.local/state)/TackDiagnostics/Testing.
Logs are already privacy-filtered: structural states, error codes, hashes, timings;
no pixels, clipboard, notes, raw filenames, invites or raw network messages.
Arbitrary error messages/stderr are fingerprinted; known failure signatures retained.
Up to 6 marked sessions; 3 x 4 MiB per process; bounded queue; dropped-event count.
3 GiB free disk required. Native crash dumps are NOT enabled or exported.
A power loss/forced kill can lose the last records. The launcher captures process
exit status independently; physical Windows 10 GPU behavior is not yet validated.

Normal tack.exe launch leaves these diagnostics OFF. This optional launcher
keeps a separate test profile; standard updates stay manual and preserve artwork.
