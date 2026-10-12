@echo off
setlocal
"%~dp0tack-diagnostics.exe" launch %*
echo.
echo Fermez Tack pour terminer la capture. Export-Diagnostic-Report collecte les traces.
pause
