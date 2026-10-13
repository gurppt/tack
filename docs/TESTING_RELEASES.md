# Testing branch and manual in-app updates

The owner explicitly authorized the `testing` branch, published test prereleases
and update delivery on 2026-10-13. This overrides the previous P0/pre-push hold
for this scoped task. Existing untracked artwork/director notes/user reports are
preserved; neither `main`, `dev`, the default branch nor old release bytes are
changed by creating/pushing `testing`.

Three channels share the current bounded portable updater, without background
polling: **Dev**, **Test**, **Stable**. Test versions use `0.1.0-test.N`, distinct
from `0.1.0-dev.N`. GitHub test entries are explicit prereleases targeting exact
commits on `testing`; a branch alone does not deliver binary updates.

## Getting into Test without copying binaries

1. In the current Tack, open **Update**, leave channel **Dev**, click **Check**,
   **Download**, save the board, then **Restart and Update**. `0.1.0-dev.3` is the
   transition release: its archive uses the legacy file allowlist so dev.2 can
   extract it. The bridge updater learns Test and the optional diagnostic kit.
   Very old Dev.1 installations using update protocol 3 need one initial compatible
   portable installation; preserve their artwork and profiles when migrating.
2. In Dev.3, click **Channel** once to select **Test**, then **Check**, **Download**
   and **Restart and Update**. `0.1.0-test.1` includes the diagnostic executable,
   launch/export scripts and the same application fixes.
3. Later test builds need only **Check → Download → Restart and Update** on Test.
   Save/pending-work guards remain. In-place replacement waits for Tack to exit,
   validates checksums/package identity, retains rollback and preserves existing
   editable PNG icons/cursors/mouse sprites, profiles and board files.

Explicit channel changes between Dev/Test prereleases of the same core version
are allowed; older major/minor/patch versions and equal/older builds within one
channel are not offered. Stable never includes a prerelease. Returning from Test
to the bridge/Dev is possible using Channel; rollback before Dev.3 requires a
manual older installation because older preferences do not understand Test.

## Reports

Normal launch keeps diagnostics off. Use **Start-Tack-Diagnostics.cmd** (Linux
`.sh`) when reproducing an issue; the launcher uses an isolated test profile.
After closing Tack, **Export-Diagnostic-Report.cmd** produces a ready-to-send
**ZIP/STORE, without compression**. Windows selects the ZIP in Explorer. No
PowerShell archive command, additional compression dependency or automatic upload.
Known structural logs, summary/build identity and checksum manifest are included;
raw dumps/stderr, boards, profiles, clipboard/pixels/invitations are excluded.
Captured log bytes and reported hashes are identical even if a source log grows.
Six marked sessions/three exports and existing per-file/export quotas remain.

The namespace `TackDiagnostics/Testing` stays consistent across test updates and
is separate from earlier B1/CD diagnostic folders. Share only the ZIP after
reviewing it. New test packages carry updated tools through the same updater.

## Maintainer delivery

Increment the whole workspace version and pinned internal versions together,
refresh Cargo.lock, commit scoped source, then compile app/server/updater (and
diagnostic launcher on Test) from the exact clean commit with matching channel.
Use `tools/package_release.py --channel test --with-diagnostics` on both platforms,
merge verified manifests and include `SHA256SUMS`. `tools/release_notes.py` extracts
only the intended changelog section. Never replace old tags/assets. Standard
manual workflow stays dry-run/draft; public prerelease publication requires owner
authorization, provided for this testing setup. No automatic stable publication.

Before publishing: check version/channel/SHA/protocol, native pinned SIMD helper,
bounded ZIP members, Linux + Windows compiler checks, update/rollback and report
tests. Verify the old updater can stage the bridge and the bridge can stage Test.
After publishing verify unauthenticated discovery/download and uploaded hashes.
Keep compact receipts; reuse the target/cache and preserve the 10 GiB disk reserve.

Pre-alpha limitations remain explicit: Windows large-image/graphics and two-machine
LAN acceptance pending; giant ≥1 GB originals, giant progressive JPEG, GIF and
incremental Embedded save remain separate gates. A Test release is not a stable
or production-readiness claim. Product licence terms remain undecided.
