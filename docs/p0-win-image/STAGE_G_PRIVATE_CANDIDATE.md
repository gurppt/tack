# Stage G — private C/D candidates and remaining physical gates

**Private test artifacts, not a release.** Frozen clean compiled source:
`871f6361ee755ae1dadc5084bc3ee1f28bc0cf37`, `dev`, version `0.1.0-dev.2`,
channel dev, protocol 4. Documentation/report commits may follow this source SHA.
No installed binary, published tag/release, `main`, container format or version
is replaced. Diagnostic launcher uses its own **P0-WIN-IMAGE-CORE-02-CD** namespace,
so B1 profiles/reports remain separate. Icons match existing author artwork;
no editable installed icons are regenerated.

## Artifacts and checks

In `bin/P0-WIN-IMAGE-CORE-02-CD/`:

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Tack-P0-WIN-IMAGE-CORE-02-CD-windows-x64.zip | 15,314,644 | 1c255c874f8529269c7d98527d9bbb957f2fef8586509dcdf5c91af58423f1aa |
| Tack-P0-WIN-IMAGE-CORE-02-CD-linux-x64.zip | 12,118,021 | f327f2bba0b76b5b91eeb7a3f21fa59cbd7a8dcdb2a76d2598995841ba1de7f9 |

132 Windows /134 Linux members; ZIP CRC and all BUILD.json member hashes verify.
All three application/server/updater build identities match the frozen SHA,
channel/version/protocol. Windows identities execute under Wine; Linux identities
execute natively. Static libjpeg-turbo 3.2.0 helper/source hash and NASM/SIMD
provenance verify; DLL inspection finds no unbundled GCC/C++/pthread/TurboJPEG
runtime. Needed notices/artwork are present; briefs, user boards, raw logs and
corpora are excluded. Linux staging README was corrected to Linux instructions
before final package validation; final checksum above supersedes the initial
unshared staging ZIP. Windows ZIP is unchanged.

Windows stripped app: 26,205,184 B vs B1 26,140,160 B (+65,024 B). ZIP difference
is +24,524 B. No new production dependency or format is enabled. Source-specific
streaming admission and bounded error information account for this series; GIF
experiment dependencies are outside the workspace.

## Quality evidence

Private receipts: `benchmark-results/p0-win-image/`, particularly `stage-g/`.
All Rust commands use pinned 1.95, two build jobs, 16 MiB compiler thread stack,
release debug=0 and incremental=0, sharing the existing target directory.
No fresh sibling worktree/target per benchmark.

- Workspace release tests: **504 passed, 0 failed, 21 existing ignored**, 83
  groups including doc tests. Ignored names/reasons are retained in the log:
  explicit GPU exports/readbacks, a supplied lifecycle fixture and measured
  regional/cache comparisons. No assertions were weakened or ignores added.
- After the final protocol-label/profile-namespace change: app library **90
  passed, 0 failed, 2 existing GPU ignores**. These overlap workspace tests.
- Full workspace/all-target/all-feature Clippy passes on Linux and Windows GNU;
  final affected app/dialog target checks pass again after the last source change.
  `cargo fmt --all -- --check`, diff check, docs and final native-dialog docs pass.
  An initial rustdoc `Arc<Window>` HTML warning was fixed as code quoting.
- Python tools: **31 passed**; before/after source-contract witnesses **2 passed**.
  cargo-deny 0.20.2: advisories, bans, licenses, sources OK (duplicate warnings).
- `--no-default-features` local admission check passes. This is **not** proof
  `tack-shared` is absent: `network=[]` and its non-optional dependency remain.
- Wine: **8 native picker cancellation/error smoke modes pass**, all clean exit;
  **6 genuine large-source supply cases pass** (gray JPEG/PNG, ordinary-dimensions
  RGB JPEG, each Linked/Embedded). Each produces 128/512/2048 representations,
  durable Save/reopen, equal document, CRC and original SHA. No physical Windows
  GPU/Defender/SMB claim follows. Helper-first JPEG thumbnail ~1.5 s under Wine;
  native Linux times are in Stage D. Embedded saves ~208–383 ms here.

The broad checks were run in release mode individually rather than rebuilding a
large debug tree via `tools/check.sh`. Rust checks correspond to that script's
format/check/Clippy/test/docs/dependency checks; Python discovery is identical.
Independent peer review has not been completed in this autonomous pass. These
remain reviewable commits/private candidates, not release approval.

## Real Linux GPU comparison, same control/settings

RTX 2060, NVIDIA 580.173.02, Vulkan on private Xvfb; no real desktop input. Three
fresh processes per side, 20 unique 600×400 sources (10 JPEG/10 PNG), same seeds,
settings and warm/uncontrolled filesystem cache. Control clean `a9260b9`; candidate
frozen SHA above. Private virtual presentation contributes to callback time;
numbers **do not represent normal-desktop FPS, GPU timing or input latency**.
PSS includes driver mappings/allocations, not just Rust heap. Physical VRAM is
not measured; the table reports application payload accounting.

| Median, control → candidate | 800×600 | 1920×1080 |
|---|---:|---:|
| Startup to mapped | 125.78 →124.72 ms | 124.51 →125.78 ms |
| Settled PSS | 198.36 →198.78 MiB | 210.39 →210.76 MiB |
| Callback p50 | 23.25 →23.61 ms | 84.54 →84.32 ms |
| Callback p95 | 26.85 →26.02 ms | 87.98 →88.74 ms |
| Callback p99 | 28.99 →27.56 ms | 88.82 →90.37 ms |
| Observed Ctrl+S ack | 205.35 →204.61 ms | 210.47 →210.41 ms |
| Accounted GPU payload | 3,403,776 B unchanged | 4,887,552 B unchanged |
| Threads | 8 unchanged | 8 unchanged |

All 12 runs have zero errors and settled disk I/O. At 800, control idle CPU ticks
over 3 s =0/0/0, candidate =1/1/1; at 1920 control =2/1/1, candidate =1/1/1.
No strict zero-idle claim on this NVIDIA environment. No significant active
regression is established by these short/noisy samples. Existing software Vulkan
comparison and 1k/4k/10k-object tests remain in Stage D, with one unique source
explicitly stated for the object-count stress.

Two >64 MiB Embedded gray sources also render on the real GPU backend, with
overview then Actual Size: JPEG six ready/requested native tiles, PNG zero by
current default. CPU/GPU payload ≤17,497,184 B, zero supply/render errors.
Worst callback: JPEG 28.11 ms control /28.39 candidate; PNG 29.17 /28.88.
Idle JPEG 2 /3 ticks; PNG 6 /6 ticks over 3 s. About 100 voluntary switches/s in
an unnamed thread and Vulkan analysis/update activity occur in **both** builds;
supply workers have zero idle ticks, no TCP peer socket is present and I/O
is zero. The NVIDIA experiment reports `strict_zero_idle=false` separately;
the original software-Vulkan zero-tick assertion remains unchanged and passed.

JPEG/PNG visual captures were inspected. An initial one-second error-panel shot
was blank in **both** builds and is not accepted as a UI witness. A separate
settled native run waits for clipboard completion: the inspected screenshot shows
`0 admitted; 1 failed`, bounded filename, `Read header`, exact unsupported-format
reason and 17 bytes. No renderer change was needed. All private displays/prefixes
are closed; real desktop and user-owned PID 17206/:7337 are preserved.

## Physical Windows/LAN acceptance still pending

B1 evidence confirms observed native Open/Save As/Share selections, successful
small-board saves and host server ready/local connection, without script-policy
change or observed crash. It does not validate every export/import/cancel/UNC
case or the new C/D large-image rendering. See the included Windows README for
physical Win10 ordinary actions, genuine >64 MiB JPEG/PNG in both source modes,
unsupported Photoshop variant reason, cancel/close, original export integrity,
Save/reopen, pan/zoom and normal-user baseline.

Linux guest errno 113 is pre-protocol TCP reachability failure. Both old installed
client and candidate are protocol 4; their version alone does not prove a
mismatch. Exact Windows invite IP/port was not recorded. Wrong selected interface,
firewall or routing remain hypotheses; no guessed endpoint scanning or global
firewall changes. Advanced Sharing now prints the real protocol constant rather
than stale literal 2. Giant Online upload/cache/lifecycle redesign is deferred.

**Release veto remains:** physical Windows large-file/GPU acceptance, exact giant
Photoshop outcomes and verified LAN peer test are unfinished. 1.33/1.478 GB input,
giant progressive JPEG, GIF and incremental Embedded saves have targeted measured
follow-ups, not claimed implementation. Do not publish or replace public dev.2.
