# Phase 2A8 — alpha foundation

Implemented on `dev`, product version **0.1.0-dev.1**, channel **Dev**, LAN major
**3**. The delivered Linux/Windows binaries and unpublished portable candidate
identify **25abd866d7c5761a518662fa6d932e84e2de70f2**. Native/performance measurements at `8f1e027`
precede an updater-only Windows path correction; application behavior is unchanged. This report is a subsequent
documentation checkpoint; it does not change that binary identity. `main` remains
at `bc0c947`; no public release, Stable promotion or automatic next phase occurred.

The normative baseline is `6f6acb38bf5bd8694b7d0699462a09742a4e6842`. Measurements
use the retained Phase 2A7 binary at `bc0c947`, which also includes the requested
immutable F10 menu shortcut and removal of global UI Scale. The baseline binary
hash is recorded in [build evidence](../benchmarks/phase2a8/builds.json).

## Required deliverables

| # | Delivered behavior / evidence |
| --- | --- |
| 1 | Implemented foundation, compound Scribble and Frame links; source checkpoint above. |
| 2 | Cargo product version, SemVer Dev/Stable policy, About/build identity; [policy](ALPHA_FOUNDATION.md). |
| 3 | `dev` integration / `main` promoted snapshots, immutable published tags/assets; same policy. |
| 4 | Manual Linux/Windows [release workflow](../.github/workflows/release.yml), exact ref, default dry-run, optional unpublished draft; no public publication mode. |
| 5 | Bounded schema-1 [merged manifest](../benchmarks/phase2a8/update-manifest.json): version, channel, full SHA, protocol, platform, size, SHA-256. |
| 6 | Explicit Check for Updates near About; Dev default; no startup request/polling. Cancel/F10 retain tool and board. |
| 7 | Sibling staged portable helper; known bounded assets, validated archive/BUILD identity, installed trusted helper applies after PID exit. |
| 8 | One previous version, journal recovery, rollback/relaunch; controlled failure/artist-data tests. Physical replacement and power loss remain human checks. |
| 9 | Alpha-default network/updater enabled; local-minimal hides/refuses both and constructs no client. Dependency-removal limits disclosed below. |
| 10 | Timed backend interface only: probe/poster/seek/scrub/play/pause/close. No media codec/process/background work. |
| 11 | 17 cached hard-pixel cursors, explicit hotspots, UI/tool ownership; [manifest](ICON_ASSETS.md), native XFixes readback. |
| 12 | One annotation-lock action with ON/OFF artwork, common action previews; authored icons copied byte for byte. |
| 13 | Every visible selected image/group member has an outline; one group handle set; renderer and interaction checks. |
| 14 | Multi-stroke Scribble, swept Eraser, appearance-preserving Merge, atomic Undo/Redo, save/reopen/shared replay; [semantics](SCRIBBLE_FRAME_LINKS.md). |
| 15 | Flip supports images, rectangles, lines, arrows and Scribbles; Frame/Note/Text excluded. |
| 16 | Ctrl+L link acquisition/Frame picker, dotted lines/cursor/feedback, Ctrl+Shift+L unlink; indexed flat relations, Frame move carries children, resize/delete/unlink preserve their positions. |
| 17 | Final baseline/default/minimal startup, idle, resources and size comparison below; raw compact receipts. |
| 18 | Local full quality gate, Windows GNU SIMD build and actual Wine CLI/JPEG execution; portable manifests. Remote CI status recorded separately. |
| 19 | Updated [small-alpha human checklist](HUMAN_TEST_2A8_ALPHA.md), including Windows/real LAN/controlled published update. |
| 20 | This report, current feature inventory, document compatibility, public README and documentation index. |

## Correctness and acceptance scope

The full local gate at `8f1e027` (`bash tools/check.sh`, pinned Rust 1.95.0 and cargo-deny
0.20.2) passed: **419 Rust tests**, **15 explicitly ignored**, **31 Python tests**,
format/check/Clippy/docs, advisories/bans/licenses/sources. Minimal all-targets
check also passed. [Validation receipt](../benchmarks/phase2a8/validation.json).
The earlier implementation checkpoint passed 13 explicit software-Vulkan renderer
and LOD tests. After the artwork/atlas change, the affected GPU readback passed
again: only cell 35 in the new fifth row is colored, so an incorrect row/UV cannot
pass by sampling another green cell. No physical-GPU stress test is claimed.

Native automation at `8f1e027` passed **86 checks**: foundation
14, UI regression 43, Scribble/Frame editing 20, two-client convergence 9.
[Compact checks](../benchmarks/phase2a8/native-checks.json) include supplied About
pixels, toolbar scales 1/2/3, cursor restoration, F10, manual-check cancellation,
visible inter-stroke draft, local save/reopen/Undo/Redo, Frame move/resize/delete,
Eraser/Merge, protocol-3 join/publish/link/compound edits and replay. Displays and
profiles are owned disposable fixtures; the user's desktop/profile was not driven.
The shared test is two native clients and one owned server on loopback, not a real
two-computer LAN. Authority tests separately cover conflicting child leases, large
inverse refusal before publication and peer Undo/restart history.

Independent reviews covered command/authority behavior and the final icon import;
[artwork review](reviews/phase2a8_artist_icons.md) found no blocker. Native tests
caught and corrected two implementation issues before delivery: completed strokes
were initially absent between pointer releases; Frame border/title picking initially
lost priority to image bodies. Annotation lock now skips Frame picking cleanly.

Merge deliberately refuses overlapping intervening unselected non-Frame objects:
one merged object cannot preserve alternating painter order. Shared Frame motion
uses the existing 256-target lease bound; excessive operations are refused, never
silently truncated. Generated inverses must fit the 1 MiB operation limit. Details
and schema-6/7 migration are in the semantics/compatibility documents. Legacy schemas
1–5 remain readable; older readers and LAN-major-2 peers cannot read newer data.

The first Windows CI run exposed an updater path bug: native `PathBuf` separators
were treated as forbidden raw ZIP backslashes. The fix validates native components
separately, keeps raw ZIP backslash/traversal/device rejection, and recognizes
known nested stage directories during retry cleanup. Linux updater tests and
Clippy passed; all seven Windows updater tests then passed under Wine, including
nested recognized-stage cleanup and rejection of raw ZIP backslashes. The 14
foundation checks were repeated on the final source successfully. Remote CI is
recorded below.

## Performance and footprint

Serial 800×600 owned X11/software Vulkan with two llvmpipe workers, no concurrent
compiler. One sample per configuration/board, warm or unspecified OS page cache;
startup differences are observations, not a statistical speed claim. The baseline
uses its 20-symbol loader and the currently authored source artwork; the delivered
build has 36 symbols. The boards are one 160×100 image and 100 legacy Scribbles.

| Board | Current build | Startup baseline → current (ms) | RSS baseline → current (MiB) |
| --- | --- | ---: | ---: |
| 1 image | alpha-default | 63.19 → 64.38 | 106.81 → 107.25 |
| 100 Scribbles | alpha-default | 63.26 → 62.74 | 108.18 → 108.36 |
| 1 image | local-minimal | 60.11 → 60.31 | 106.65 → 107.07 |
| 100 Scribbles | local-minimal | 63.66 → 60.76 | 108.11 → 108.52 |

Every final run recorded **0 idle redraw, 0 CPU ticks, 0 voluntary context switches,
0 I/O delta** during the three-second settled window, and **no TCP/UDP socket**.
There are **11 threads** in each process, including software-driver threads and the
existing bounded image workers. X11 Unix sockets are operating-system display
connections. Foundation checks also observed no updater child; no media process or
polling path exists. `/proc` cannot attribute physical VRAM: accounted image peak
is 104,960 bytes for the image and 0 for the Scribble-only board. The Scribble-only annotation
allocation is 65,536 bytes (0 on the image-only board); grid allocation is 0. The final icon texture+vertices
account for 44,800 bytes, versus 36,608 before the new atlas row.

The stripped Linux application is **20,461,088 bytes**, local-minimal **20,142,104**
(delta **318,984**, about 1.6%). Windows application is **24,882,176** bytes.
The comparative minimal Linux ZIP is **8,589,503** bytes and omits the server/updater;
it is a measurement fixture, not an update asset. Full portable sizes/hashes follow.
[Build sizes](../benchmarks/phase2a8/builds.json),
[minimal ZIP scope](../benchmarks/phase2a8/minimal-package.json),
[paired measurements](../benchmarks/phase2a8/local-paired.json).

Unique host normal external dependencies: baseline **144**, default **145**, minimal
**145**. The admission feature gate does not remove the linked shared model/protocol
crates; claiming dependency isolation would be false. A later extraction can move
those types behind a narrower boundary. HTTPS/ZIP dependencies are in the standalone
helper, outside the app. The measured binary reduction comes from removing enabled
capability paths, not from a new rendering architecture.
[Dependency inventory](../benchmarks/phase2a8/dependencies.json).

## Portable candidates and Windows

Fresh committed-source candidates under
`benchmark-results/phase2a8/candidate-25abd86/` have matching version/channel/SHA and
LAN major 3. No release was published and the release workflow was not dispatched.

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `tack-linux-x86_64.zip` | 11,120,535 | `69fb3f3f15198516c4b7a912a3baa114e8ecd097048f5db7e7ebdc1b1dafe7ec` |
| `tack-windows-x86_64.zip` | 13,852,631 | `38129c085e456f4765942f7d09f27cc9644e975035222fd9cc7acac791f15533` |

Linux bundles the tested libXi sibling and relative runtime path. Windows was
cross-built with the pinned libjpeg-turbo 3.2.0 static SIMD SSE2/AVX2 decoder and
checked for unexpected runtime DLLs. Wine executed all three fresh packaged
binaries' build identities and JPEG create/inspect successfully; the package has
one embedded source and one image after inspection.
[Windows execution](../benchmarks/phase2a8/windows-execution.json).
`bin/tack-windows-x86_64.zip` is the separately assembled human-test bundle
(13,693,156 bytes); its contents/hash differ from the fresh release dry-run by design.
Physical Windows desktop/update replacement remains unvalidated.

GitHub [Quality run 38085769917](https://github.com/gurppt/tack/actions/runs/38085769917)
passed all three jobs for the exact delivered implementation `25abd86`: Linux,
Windows and dependencies. This includes the native Windows MSVC test suite and
minimal checks; Linux also ran the explicit software-Vulkan tests. The previous
Windows failure and its correction are retained in the [CI receipt](../benchmarks/phase2a8/ci.json).
A later documentation-only commit can trigger another run; this evidence identifies
the implementation commit rather than claiming an unobserved later run.

## Artwork, disk and remaining human gates

At the owner's explicit request, 36 action symbols, 13 cursor drawings and the
75×35 About logo were imported from `bin/gfx/icons/work_icons/` **without generating
or altering pixels**. Normal builds/updater still preserve existing editable files
and seed missing defaults only. All **177 runtime PNG files** remained unchanged
across final Linux/Windows builds. Working originals, mouse illustrations and
alternate unused drawings remain in place.
[Import hashes](../benchmarks/phase2a8/artwork-import.json),
[preservation](../benchmarks/phase2a8/artwork-preservation.json).

The shared Cargo target/native libraries were reused; no full Cargo clean or user
cache deletion occurred. Approximately 307 MB of known superseded expanded
candidate copies were removed, retaining their original ZIPs/manifests/receipts.
The single temporary Wine prefix used the documented 1.7 GiB exception and was
removed after its owned wineserver stopped. Retained phase evidence is below
512 MiB, with about 15 GiB free and the 10 GiB reserve intact.
[Generated cleanup receipt](../benchmarks/phase2a8/generated-cleanup.json).

Still pending: artist feel, actual Windows interaction/apply, real two-computer
LAN, a deliberately published controlled Dev release and actual power-loss
replacement. Controlled helper tests do not substitute for these. The October 10
whole-system crash remains undetermined; see the
[incident record](INCIDENT_2026_10_10_SYSTEM_CRASH.md). No causal claim about Tack is
made. No playback codecs, account system, telemetry, plugin ABI or Studio Server
were added. Phase 2A8 stops here.
