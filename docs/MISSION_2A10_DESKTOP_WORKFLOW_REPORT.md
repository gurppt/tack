# Phase 2A10 — Desktop workflow and export

Implemented desktop chrome, Window commands, procedural Lines grid, original-source
batch export and bounded PNG board snapshots. Metadata schema 8, LAN protocol 4
and development version `0.1.0-dev.1` are unchanged. Stop after 2A10.

## Platform audit and final behavior

The audit preceded implementation. Pinned winit 0.30.13 exposes decorations,
minimize/maximize, native move/resize and window levels. Its public move contract
assumes a left press. X11 sends EWMH `_NET_WM_MOVERESIZE` with button 1; Windows
sends `WM_NCLBUTTONDOWN`; Wayland uses the latest pointer button serial.
See [winit Window](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html)
and [WindowLevel](https://docs.rs/winit/0.30.13/winit/window/enum.WindowLevel.html),
plus the pinned backend sources in the local Cargo registry.

| Feature | X11 | Windows | Wayland |
| --- | --- | --- | --- |
| Top-edge hover chrome | Owned xfwm4/Xvfb tested | Same native UI implementation; physical acceptance pending | Native Wayland, nested Weston tested |
| Minimize/maximize/restore/close | Native operations tested | winit native operations; physical acceptance pending | Compositor decides; kiosk smoke does not validate these operations |
| Left chrome drag / thin edge resize | Native operations tested | Native winit path | Native compositor requests |
| Native title-bar preference | Request/restoration tested | Native decoration request | Preference retained; Tack controls remain available even if no native decorations are supplied |
| Always on Top | WM ABOVE hint tested, including removal | Native window-level hint | Disabled in Window menu; an explicitly bound unsupported action explains the limitation |
| Empty-canvas right drag | Nonmodal status hint directs the user to left chrome drag | Same bounded fallback | Native move request with pointer serial; physical desktop acceptance pending |

The chrome is 24 logical pixels high, revealed within the top 4 logical pixels,
and hides on departure/focus loss. It has mini Tack menu, minimize, maximize/restore
and close controls. Edges use native resize requests. F10 remains the reserved
application-menu recovery shortcut. Harmless desktop/grid preferences are inline.

Right down records an eligible empty point; movement must reach 8 logical pixels.
A simple release or movement below threshold opens the existing context menu.
Images including locked ones, annotations, Frame tabs/handles, active gestures,
local UI and explicit right-button keymap bindings are excluded. Custom right-button
Pan was tested to move the camera while retaining native window geometry.

An attempted safe X11 EWMH button-3 bridge was rejected during native testing.
A second X connection cannot release winit's implicit grab; forcing a fresh grab
through the public API still failed to produce movement. Capture/timestamp
interference is a likely explanation, not a measured WM diagnosis. Calling winit's existing
button-1 move during a right gesture did move the window but broke subsequent
right-click delivery. Those experiments and the extra direct dependency were
removed. No unsafe display-pointer access, synthetic button injection, manual
window-position loop or recurring window probe remains. This is an explicit
backend fallback permitted by the brief, not a claim that X11 right move works.

## Per-application stacking feasibility

Research only; deferred on all three platforms. Windows owned top-level windows
stay above their owner, but acquiring a foreign HWND and managing its cross-process
lifetime/ownership is not a safe winit operation. It would introduce a separate
platform integration. [Microsoft window features](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features).

X11 ABOVE/BELOW are WM hints, not a portable guarantee of staying above exactly one
foreign application. Transient relationships are WM-dependent. No foreign-window
enumeration or watcher was added. [EWMH window properties](https://specifications.freedesktop.org/wm/1.5/ar01s05.html).

Wayland provides no general foreign-window stacking control through winit.
Always on Bottom is also deferred. No compositor hacks, injection, accessibility
integration, constant focus/raise/lower work or external-app polling was added.

## Grid

Preferences offers Dots or Lines. Lines reuse the existing procedural grid pass,
with quiet minor lines and stronger major lines every five subdivisions. World
anchoring and the existing adaptive spacing are retained. The three theme palettes
supply the colors; no texture or renderer subsystem is added. The uniform gains
16 bytes. Old profiles default to Dots; new settings round-trip through portable
profiles and merge without losing another window's changes.

## Source export

File offers Export Selected Sources and Export All Sources, using an OS folder
picker. Each operation creates `Tack Sources`, then `_2`, `_3`, etc. Existing runs
are retained. Selection of a Frame includes linked/contained children. Groups
export individual image objects; repeated instances receive deterministic filename
suffixes. Linked data is opened from the current external file on the explicit
worker operation, independently of cached LODs. Embedded data comes from the
current authoritative original payload/revision. Neither is re-encoded; copying
uses the existing 128 KiB streaming/atomic path and CRC validation of stored
Embedded ranges.

Export by Frame is an inline preference. Explicit Frame links take priority;
otherwise the first containing Frame by ID is used. Frames are flat. Names strip
filesystem control/reserved characters, trim dots/whitespace, handle Windows device
names, and are capped at 80 characters / 160 UTF-8 bytes. Case-insensitive duplicate
names get deterministic suffixes. Safe PNG/JPG/JPEG/BIN extensions retain their
original spelling. Unframed objects go under `_unframed`.

Text Blocks become deterministic UTF-8 `notes.txt`, in document order, separated by
two newlines. Bounds: 4096 Frames, 4096 exported image/note items, 1 MiB notes per
folder and 8 MiB notes total. Directory/symlink and copy errors are surfaced.
Cancellation or an error retains any explicit partial run folder; it never broadly
cleans a user destination. Shared originals must first be saved to an independent
local board using Save to Local.

## Board snapshot

Export Board Snapshot is a separate PNG save operation. With a selection, its
bounds define the rendered region; otherwise the full board defines it. All
visible content intersecting that region is included. Padding is 24 world units,
and one world unit becomes one output pixel. Frame label extents are included.
The production image/annotation renderer and common Frame label drawing are reused;
selection handles, caret, toolbar and menus are omitted. Images retain crop,
transform, flip, opacity, grayscale and chosen sampling. Text, shapes and Frames
are included, along with the grid when enabled.

Before raster allocation, reject nonfinite/empty geometry, sides above 4096 pixels
or more than 8,388,608 pixels. Additional bounds: 4096 board objects/assets/sources,
16 MiB annotation metadata before cloning, 64 MiB cumulative decoded display textures,
and existing annotation/overlay/glyph capacities. Budget overflow is an explicit
error rather than silently omitted content. No enormous tiled export subsystem is
introduced. Shared boards require Save to Local first.

Image display representations are limited to 2048 pixels per source edge, using the
existing streamed decoder. This is a display-quality snapshot, not a full-original
resolution export. Selected small regions of a board exceeding the global metadata
bounds are still rejected. Readback/encoding and the bounded GPU wait occur only on
the explicit local worker. Output is published atomically. If an automatically added
`.png` suffix would select an existing file that the picker did not confirm, the
operation refuses replacement and requests the explicit PNG filename.

## Verification and performance

Quality: pinned formatting/check/Clippy, workspace tests/docs, 31 Python checks and
cargo-deny 0.20.2. The first final quality attempt lacked cargo-deny on PATH; the
prepared pinned tool was then used and the complete pipeline passed. Targeted checks
were repeated after the filename bound, overwrite guard and nonmodal fallback fixes.
460 Rust/workspace/doc-test passes; explicit GPU tests are separate. Ten production
renderer GPU checks cover Dots/Lines, major anchoring, flat fill and palettes. Two
snapshot GPU tests cover pixels, PNG publication/cancellation/overwrite preservation,
selected bounds, nine distinct products, Linked and Embedded sources, text, shapes
and a Frame outline. Source tests cover current Linked bytes, pinned Embedded bytes,
Frame folders, names, duplicates, notes, selected Frames, cancellation and symlinks.

Native runs use owned Xvfb displays, private xfwm4/DBus or nested Weston, Lavapipe
Vulkan and two rendering threads. They never drive the user's physical desktop or
existing Tack processes. The X11 harness covers chrome/F10, native drag/resize/window
state, nonmodal right fallback, simple/subthreshold right click, object exclusion,
inline grid preference, real folder/PNG pickers, byte-exact exports and custom right
Pan. Nested native Wayland covers F10, explicit unsupported Top feedback, retained
hover controls and clean renderer shutdown. Its timed kiosk smoke is not an idle,
move or physical compositor acceptance test.

After exports, the X11 harness measures a settled three-second interval via `/proc`:
zero CPU ticks and zero deltas for all process I/O counters. This observation has
clock-tick resolution; it is not GPU timing or a promise about every machine.
Desktop chrome stores only small event state. Applying window hints happens when
preferences change. Export GPU resources belong to a finite worker call and drop
when it ends. No new idle timer, export daemon, foreign-window watcher or grid texture.

Compact final receipts are retained under `benchmarks/phase2a10/`; raw development
logs/captures are ignored under `benchmark-results/phase2a10/`. Reproduction:
`tools/run_phase2a10_checks.py` and `tools/run_phase2a10_wayland.py`. The latter needs
Weston and its modules prepared under the repository's native-tools directory.

## Disk discipline and artwork

Shared targets/native libraries and release artifacts were reused. Early debug-test
fanout unexpectedly lowered free space to about 7.8 GiB, temporarily breaching the
10 GiB reserve. Large writes were stopped and only inspected obsolete incremental
directories and old test executables were deleted, reclaiming approximately 8 GiB.
The ignored deletion manifests preserve the exact lists. No user file, board,
profile, brief, native library or full Cargo target directory was removed.

Subsequent checks/builds started with about 14–15 GiB free. Windows Wine CLI/decoder
verification may temporarily use up to 2 GiB in an owned prefix: this is a documented
exception to the preferred 512 MiB phase-generated-data budget. The prefix is removed
afterward; compact receipts are retained. This excludes reused Rust/native build
caches. Builders seed missing artwork only; existing editable runtime icon hashes
are compared before and after packaging. Pre-existing README edits and user assets
are kept outside this mission's commits.

## Human acceptance / remaining gates

- [ ] Independent review of this nontrivial change, per CONTRIBUTING.md; no independent
  reviewer result is fabricated in this report.
- [ ] Physical Windows: hover chrome, native drag/resize, min/max/restore/close,
  Always on Top and OS folder/PNG dialogs at 100% and higher DPI.
- [ ] Physical Linux desktop: stacking versus a creative application, multiple monitors,
  DPI transitions, and repeated moves/resizes; confirm X11 right-drag fallback is acceptable.
- [ ] Physical Wayland: native left/right move serial handling, native resize/window
  actions and decoration fallback with the actual compositor.
- [ ] User review of quiet Lines/Dots in all three themes and exported source folder names.
- [ ] Open snapshots from representative production boards to assess text, Frame labels,
  crop/sampling and the documented display-quality/size limits.

These are controlled-test/physical release acceptance gaps, not claims of validation.
No next phase, public release, WAN service or compositor integration is started.

## Delivered builds

Implementation commit: `980dc342fa19ad43a175be76dbbdef3f231f16cb`.
Delivered Linux native checks: 24/24; nested Wayland: 4/4. Linux SHA-256:
`0972e8bd348a027b18f69bd2ac8fdb992f3d8234627334fb253b2e48f90cdcf7`.
Windows ZIP SHA-256:
`4010df9dc15fd00f11e7f7cddc2269ef68ee8cdd32b9b00c6a2ece7d32325cfc`.
Windows CLI/JPEG checks passed under owned Wine/Xvfb: JPEG board creation/reopen,
scalar/SSE2/automatic SIMD decode, identical decoded outputs. The temporary prefix
peaked at 1,696,539,835 bytes and was removed. All three Windows executables contain
the implementation identity; packaging checks external runtime imports.
The settled delivered Linux interval measured 0 ticks / 0 I/O, about 122 MiB RSS.
169 existing editable icon PNGs retain their exact hashes. Final free disk space
is approximately 14.3 GiB before the small documentation commit.

Final build identities, executable/package hashes and native receipts are recorded
in `benchmarks/phase2a10/builds.json` and the adjacent validation receipts after the
implementation commit. Linux: `bin/tack`. Windows x64: `bin/tack-windows-x86_64.zip`,
with static libjpeg-turbo 3.2.0 SIMD SSE2/AVX2 decoder and editable artwork.
