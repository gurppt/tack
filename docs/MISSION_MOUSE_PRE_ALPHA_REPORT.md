# Mouse pre-alpha polish and Link overlay corrective

2026-10-11 — implementation `24440e1af2013c05c71be3d1b75af3a8f361615c`,
`dev`, version `0.1.0-dev.1`, LAN protocol major 3 unchanged.
Delivery source `64f09f24a498cdd54cbbd1d6de0793d11f33c73e` adds only
bounded diagnostic frame counters and the Link idle harness check.

Link-to-Frame no longer exceeds the renderer limit through per-dot geometry.
Mulot is an optional semantic tool in Keymap and Edit Toolbar, with no default
shortcut, toolbar entry or menu entry. Theme choices are directly inside
Preferences: click or Enter applies and persists immediately, with no modal.

## Artwork and effect bounds

The nine supplied PNGs are imported byte for byte; see
[hashes](../benchmarks/mouse-polish/artwork-import.json). Builds continue to seed
missing defaults only. The explicit import updates the existing runtime
`mulot_icone.png` to the requested `work_icons` version; other existing runtime
artwork remains untouched. `minimulot` uses hotspot 7,7 on canvas; the normal
pointer returns over UI, then Mulot returns on canvas.

| Direction | Authored orientation | Mirror X | Mirror Y |
| --- | --- | --- | --- |
| N | N | no | no |
| NE | NW (45°) | yes | no |
| E | W | yes | no |
| SE | NW (45°) | yes | yes |
| S | N | no | yes |
| SW | NW (45°) | no | yes |
| W | W | no | no |
| NW | NW (45°) | no | no |

A single-axis mirror swaps the authored L/R source to preserve handedness.
Paws alternate at 18 logical pixels, with ±3-pixel lateral offset, integer
placement and 800 ms lifetime. One shared queue holds at most 64 paw/ping
effects; synthetic huge motion emits at most 64 per event. Cached binary masks
reuse existing pixel overlay primitives. Ping has four hard-pixel arms at
radii 3/6/9, one 120 ms step each, gone at 360 ms. No new dependency, effect
worker, texture, audio, protocol message or per-paw timer.

## One-shot image insertion

Activation arms one future deadline at exactly 600 seconds. Cancellation removes
it; repeated active state does not rearm it. An injected clock tests the
600-second boundary, cancellation and reactivation without waiting ten real
minutes. Native diagnostics accelerate to five seconds (1.5 seconds for
lifecycle checks); the override requires diagnostics and is not a preference.

At expiry, one normal embedded PNG import inserts a nearest-sampled image
48/32 logical pixels beside the pointer, above existing content. The ordinary
asset/source/object transaction is one Undo step; later activations reuse the
asset. No persistent Poo variant is introduced. A 32-bit AssetId namespace with
96 retained random bits recognizes generated images for the exact existing
status text `Puzzo puzzo !`, including after reopen; concurrent first imports
have independent source/asset IDs. This is an identity convention, not a new
file field or network message. Existing image editing, deletion, order and
shared document semantics apply.

Pending insertion waits for existing Save As/worker completion without a retry
timer, remains undoable after source rebase/history reset, and is discarded for
closing/replaced boards. Late import completions are checked against DocumentId.

## Link crash and bounded representation

Before the correction, the delivered build `25abd866` (binary SHA
`edf204e34f17fab24c9cf9c9162006c7de8b0699cd41aaac7646f04518921b6d`)
exited with `Error: "overlay primitive limit"` in an isolated native test:
32 selected images, 32 linkable units, zoom 0.0111089965, cursor 100/100,
1280×1024. The exact old loop replay using the native camera and saved fixture
produces 4096 dot quads after 22 lines; the renderer limit is 2048. Each dot was
one quad. The count is a reconstruction of the actual old loop, not old renderer
telemetry; the process failure is directly observed.

The replacement clips to the visible screen, snaps logical pixel endpoints and
uses one thin quad/six vertices per visible unit. The existing overlay fragment
path applies static 4-on/4-off dashes, without antialiasing or per-dash vertices.
It shares the existing overlay submission range; it adds no render pass or draw
call. Thin triangles prevent shading the diagonal's whole bounding rectangle.
The same native fixture now renders all 32 lines with 32 quads / 192 vertices,
then commits all 32 links. Undo/Redo is exact.

At most 256 preview lines are admitted. Remaining capacity determines a
stable stride through linkage units; refusal skips decoration and keeps the
operation usable. Selection decoration reserves room for target and chrome.
Hover highlight, open-chain cursor and brief confirmation remain. The hard
2048 renderer correctness check is retained. Tests cover one image, grouped
images plus annotation, 1024 images, 10000 anchors, extreme zoom/distance and
a three-quad budget. Escape removes Link geometry and feedback deadlines.

For 32 software-Vulkan pointer-movement frames, median/p95 CPU preparation
was 0.383/0.462 ms; callback 11.815/12.668 ms; present 11.375/12.157 ms.
These are diagnostic software-renderer samples, not hardware GPU timings.
After Escape, the native test observes zero retained lines and zero redraws
for two seconds. CPU/frame measurements and exact artifact hashes are in the
[compact evidence](../benchmarks/mouse-polish/verification.json). The old case
fails before a successful Link frame, so there is no valid before/after frame
speed comparison and no claim of a general renderer optimization.

## Validation and human acceptance

`tools/check.sh` passes (430 Rust and 31 Python tests): formatting, all-feature check/Clippy/tests/docs,
Python tests and cargo-deny. Minimal no-default-features check passes. All nine
explicit product GPU tests pass on bounded software Vulkan, including exact
4-on/4-off hard-pixel output. Independent
[review](reviews/mouse_pre_alpha.md) has no remaining blocker.

Native owned X11/software Vulkan checks cover inline persisted themes, Keymap
catalog, manual toolbar addition, exact cursor pixels, paws/ping, cancellation,
one image only, Undo/Redo, persistence and exact hover status. Three two-second
idle windows (inactive, active stationary with real 600-second deadline, then
deactivated) show zero CPU ticks, zero main-thread wakeups, zero additional
file I/O, stable thread count and no TCP/UDP socket. Diagnostics output polling
is disabled for these idle measurements.

The 20 Mouse and nine lifecycle native assertions used `24440e1`; the six
Link assertions used delivered `64f09f2`. The only intervening runtime change
is diagnostic counters inside the existing optional `--output` frame capture.

Nine additional native lifecycle assertions cover real Save As with source
revision 2/absolute path rebase, deferred insertion and fresh Undo; clean Close
Board replacement with no leaked deadline/image; and dirty Close Board Cancel
followed by a successful single activation. Link native checks reproduce the
old-crash camera, commit all units and verify Undo/Redo and bounded vertices.
Raw evidence remains in ignored `benchmark-results/mouse-polish/`; harnesses
are tracked. Generated data stays far below 512 MiB; shared Cargo/native caches
are reused: raw evidence is 2.23 MiB and 14.22 GiB remains free, retaining
the 10 GiB reserve. All 201 unrelated existing runtime PNGs are unchanged;
only the explicitly requested Mulot icon import differs among 202 snapshots.

Human checklist: automated coverage passes for theme, discovery/cursor,
effects/deadline cancellation, image admission and Link correctness. Still
pending: artist judgement of eight-direction gait and dot clarity, actual
uninterrupted ten-minute activation, and physical Windows desktop acceptance.
No real ten-minute wait was performed. Linux and Windows portable builds retain
the native SIMD JPEG decoder; exact deliverables are recorded in the evidence.
Deliverables: `bin/tack` and `bin/tack-windows-x86_64.zip` (13,716,298 bytes),
with server/updater companions and the pinned static libjpeg-turbo 3.2.0
SSE2/AVX2 Windows decoder. The binaries embed source `64f09f2`; later report
commits do not change that code. The existing minimal binary is not rebuilt
in this pass; its source configuration is checked separately.
No public alpha was published and no further mission was started.
