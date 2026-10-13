# Test delivery — completed 2026-10-13

Owner requested a dedicated GitHub test branch, published Windows/Linux binaries,
manual in-app test updates and already-zipped reports without compression. All
four are delivered. [Usage and maintenance](TESTING_RELEASES.md).

## Frozen published builds

| Release | Compiled commit | Public assets |
| --- | --- | --- |
| [Dev.3 transition](https://github.com/gurppt/tack/releases/tag/v0.1.0-dev.3) | `28f2554c4cef57efc6d4899f35c46c575930f3db` | Linux/Windows x64 ZIPs, manifest, SHA256SUMS |
| [Test.1](https://github.com/gurppt/tack/releases/tag/v0.1.0-test.1) | `ece2c57686bf2d067a3c9df88a063bf08a7d783b` | Linux/Windows x64 ZIPs, manifest, SHA256SUMS |

Each executable's compiled version/channel/commit/protocol matches its manifest.
GitHub upload sizes and SHA256 digests match local artifacts; annotated tags target
the frozen commits. Both entries are published prereleases. Existing public Dev.2
release identity and asset metadata remain unchanged. Documentation-only commits
may advance `testing` without changing these frozen binaries or tags.

Test.1 Linux ZIP: 12,138,582 bytes,
`6eaf6f9310a3ffd5c42708aa867f91baeb848d8247d5136c3a1b92c8206e2d81`.
Test.1 Windows ZIP: 15,348,524 bytes,
`f296188a7bc3634fe0e4af72511ac845438da6ebedf0960776219e9521f82f85`.

## Verified update path

1. Genuine Dev.2/protocol-4 updater discovers only Dev.3 on Dev, downloads its
   public GitHub ZIP, validates it and stages its legacy-compatible file set.
2. That old helper applies Dev.3 and starts the updated application. Its window
   maps on an isolated X11 display using software Vulkan.
3. Installed Dev.3 helper discovers Test.1 on Test, downloads the public ZIP,
   stages the optional diagnostic kit, applies it and starts a mapped Test.1
   application with the correct compiled identity.
4. Board/preferences sentinel files and a valid custom PNG icon keep identical
   SHA256 values across both updates. These are owned preservation fixtures,
   not validation of loading the sentinel as a real board. Rollback backup exists.
5. Test.1 on Test reports Current; explicit Test-to-Dev returns Dev.3. The old
   Dev.2 helper still skips Test.1 after publication. No real desktop was driven,
   existing user installation replaced, or live user server stopped.

The initial test harness captured subprocess stdout, which remained open in a
spawned GUI/Explorer and caused a harness timeout after a successful operation.
Using regular log files allowed completion; the corrected apply test passed.
Windows report export itself had already produced a complete, valid archive.

Dev.1/protocol-3 needs one initial compatible installation. No protocol was
mislabelled to bypass this boundary. Dev.2 users can migrate entirely through
Update: Dev.3, then Channel Test, then Test.1. Subsequent updates use Test directly.

## Quality and report validation

- Full Rust workspace: **506 passed, 0 failed, 21 existing ignored**.
- Linux and Windows GNU all-target/all-feature Clippy passed; formatting/diff
  checks passed. Python tool tests: **32 passed**.
- After the Test build version change, affected application library tests passed
  again, including the compiled Test default channel.
- Linux executables and all four packaged Windows executables under Wine expose
  the expected build identity. Native pinned SIMD JPEG decoder is included.
- Linux and Windows-under-Wine diagnostic exports are actual ZIP/STORE archives:
  every member has compression method 0, valid CRC and manifest hash. Boards,
  preferences, raw stderr and ownership markers are excluded from exported ZIPs.
- Normal application launch keeps diagnostics off. Report ZIP code is confined
  to the optional diagnostic executable. No production dependency, background
  update polling or automatic report upload was added.

Raw compact receipts remain locally in
`benchmark-results/test-release-bootstrap/`: publication receipts, GitHub
before/after metadata, bridge/test discovery/stage/apply receipts, quality logs
and Linux/Wine report receipts. Public binaries are also retained locally under
`bin/releases/0.1.0-dev.3/` and `bin/releases/0.1.0-test.1/`.

## Scope and remaining acceptance

`main` remains `55f516d5e33b4db53945a035dfe8e0982b42193d`, `origin/dev` remains
`9b1ce10c01dc480705450daffc104d80d77efbee`, and the default branch remains `main`.
User untracked artwork, briefs, director notes and reports were preserved.
This scoped publication follows the owner's later explicit authorization; the
earlier P0 pre-push hold remains a historical record of its earlier decision.

Physical Windows update/restart, large-image/GPU acceptance and two-machine LAN
acceptance still need human tests. Wine and cross-compilation do not replace those.
The prior bounded JPEG/PNG corrections are included, but giant ≥1 GB originals,
giant progressive JPEG, GIF and incremental Embedded saving remain separate gates;
see [P0 evidence](p0-win-image/README.md). This is a test release, not stable approval.

End cleanup removes only the marked disposable updater experiment installation,
after checking no process uses it. Compact receipts, exported report samples,
published artifacts and reusable compiler/native caches are retained. No new
sibling folder/worktree, blanket cache sweep or user artwork regeneration.
