# Alpha foundation (development)

Phase 2A8 starts from `6f6acb38bf5bd8694b7d0699462a09742a4e6842`;
`bc0c947` additionally reserves F10 menu access and removes global UI Scale.
The workspace version is the product version. The current integration version
is `0.1.0-dev.1`; no release has been published for it.

`dev` integrates work; `main` holds promoted snapshots. Intermediate commits need
no version bump. Every deliberately published dev release needs a new SemVer
`-dev.N` version. Stable has no prerelease suffix. Release bytes/tags are immutable.
Testers receive GitHub Release packages, never a branch checkout.

## Manual portable updates

`Check for Updates` uses the existing local worker to run a sibling helper only
on demand. Default channel is Dev; Preferences can select Stable. No startup
request, polling, resident updater or media process is introduced. Dev discovery
lists Releases (up to 120 bounded entries), excludes drafts and compares SemVer;
an incomplete search is an error, not an "up to date" claim.

The small `tack-update` crate carries schema 1, version/channel, full commit,
protocol major, platform filename, exact length and SHA-256. HTTPS and ZIP
libraries live in `tack-updater`, outside the app binary. Responses/manifests
are bounded to 32 KiB; archive/unpacked limits are 128/256 MiB and 256 files.
Only known application assets are admitted. Paths, symlinks, duplicate paths and
Windows device names are refused. Package BUILD identity must match the manifest.

Download creates an owned installation-local stage. Cancellation retains a
partial stage; retry enumerates known bounded files before removing it. Unknown
personal data is retained and blocks cleanup. Restart requires a clean document
and completed pending work. A copy of the **installed trusted helper** is launched
outside the replaced assets; staged executables are never executed directly.
The helper re-verifies the archive and re-extracts it, waits for the old PID to
exit, journals replacements and renames files on the same volume. It preserves
existing editable icons/cursors, boards and profiles, keeps `.tack-previous`, and
relaunches Tack. Success removes the owned stage.

An interrupted transaction can be restored with:

```sh
tack-updater recover /absolute/path/to/portable-folder
```

Run this only after Tack has exited; restart the restored app afterward. The
installed helper may itself have been moved into `.tack-previous` during a crash;
use that previous helper if necessary. Failed replacement/relaunch rolls back.
A new update first restores an unfinished transaction and reports this recovery
instead of silently proceeding. Journal JSON/TMP recovery covers interrupted
publication. Actual power loss and physical Windows replacement remain human
acceptance gates; no synthetic test proves every filesystem failure.

Non-writable installations offer manual download without elevation. Source
trust currently rests on HTTPS to `gurppt/tack` Release assets plus their manifest;
SHA-256 protects consistency, not a compromised publisher. A signed manifest and
embedded helper verification key are a future broad-Stable distribution option.

## Packaging and workflow

`Portable alpha candidate` accepts an identified commit and Dev/Stable channel.
The default is dry-run; optional draft remains unpublished. The workflow has no
public-publish mode. Both platforms must pass quality gates and agree on compiled
version/SHA/channel/protocol. Native SIMD JPEG provenance is checked. Linux ships
libXi 1.8.3 with relative `$ORIGIN/lib` lookup; other desktop/driver system libraries
remain prerequisites. Fresh packages use repository defaults, never runtime user
artwork/profile directories. Intermediate artifacts expire after seven days.

`tools/package_release.py` produces fresh platform ZIPs and fragments, then verifies
both ZIPs and merges `update-manifest.json`. Existing outputs cannot be replaced.
Two packages are deliberately required before the merged manifest is issued.
`--allow-dirty-dry-run` is only a local development aid; release CI does not use it.
An updater stage can temporarily retain archive plus two unpacked copies (640 MiB
worst case); success removes it. This bounded runtime exception is separate from
the 512 MiB phase-evidence budget. The previous package is retained once.

The bundled webpki trust data uses [CDLA-Permissive-2.0](https://cdla.dev/permissive-2-0/);
the agreement text is distributed with the package. Tack's own license remains
undecided.

## Capability boundaries

`alpha-default`: app default features `network` and `updater`.
`local-minimal`: `cargo build -p tack-app --no-default-features`.
These currently gate admission/UI: no Share/Join/update actions and no client
construction in the minimal build. **Protocol code is still linked.** Existing
hosting, leases, offline transitions, import and local-worker DTO paths cross
many composition modules. Making the entire dependency optional now would need
invasive churn; the brief permits an assessment and a small seam instead. Chrome
now consumes generic `UiConnection`, not a transport/client state type. Document,
renderer and local storage are identical in both builds. The admission-gate cost is measured in the final phase report.

`TimedAssetBackend` defines probe/poster/frame/seek/scrub/play/pause/close only.
No codec/player is implemented or instantiated. Contextual frame stepping is
bounded to one frame or ten with Shift for a future active timed asset. Future
SWF is passive main/Graphic/static MovieClip visualization: no VM, scripts,
interaction, network or independent timelines.

Seventeen 16×16 hard-alpha cursor assets have fixed hotspots and cached handles.
Runtime files are seeded only if missing, just like toolbar artwork. About retains
the editable local logo, accepting PNG variants up to 128×35 pixels/16 KiB and
drawing them at natural integer pixels; the packaged author logo is now 75×35. The manual
seed script is never a build hook. Annotation lock ON/OFF has one action identity.
Visible selected images, including group members, derive individual outlines;
handles remain one selection frame. Flip includes images and non-text annotations.

## Board editing and acceptance

Compound Scribble, Mini Eraser, Merge Scribbles and flat Frame linkage are implemented.
See [semantics and limits](SCRIBBLE_FRAME_LINKS.md) and [human checklist](HUMAN_TEST_2A8_ALPHA.md).
These require LAN protocol major 3 (older peers must upgrade); old board schemas remain readable.
Native/package validation and comparative performance receipts are recorded in the final phase report. This document is a development record,
not an alpha acceptance claim. Physical artist feel, Windows desktop and two
physical computers on LAN remain human gates. The earlier system freeze is still
undetermined; automated UI checks use an owned software-Vulkan display.

## Foundation checkpoint verification

Local quality gate: 399 Rust tests passed (15 explicit GPU/long tests ignored),
31 Python tests passed, fmt/check/Clippy/docs and cargo-deny passed.
The minimal configuration compiles all app targets. Updater includes 11 tests,
with controlled re-extraction/apply/relaunch, exact rollback, wrong hash and
identity, partial-stage retry, interrupted journal, backup retention and
personal-data protection. Native integration and release dry-run are next.

The October 10 artist icon update is included without regeneration; see
[asset mappings and preservation](ICON_ASSETS.md). The shared toolbar atlas is
128 × 80 with 36 symbols and the existing 32-visible-quad bound.
