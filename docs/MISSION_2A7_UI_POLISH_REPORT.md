# Phase 2A7 — UI/input polish

Implementation and automated local acceptance complete. Baseline implementation
`80f7014fefc6420491f5ee65891280ee384eb84e`; source base `ff35c895` includes
its separate incident record. Artist feel, physical Windows desktop and physical
two-computer LAN acceptance remain pending.

## Delivered behavior

- Keymap uses Action / Shortcut / Behavior grid navigation, one mutually exclusive
  Normal/Hold/Release enum, staged collision confirmation, named keyset identity
  and a dirty marker. Save / Save As / Load share a bounded file workflow;
  Default is protected. Binding and keyset identity merge atomically across windows,
  and asynchronous Save preserves newer edits. Preferences export remains separate.
- Shared integer scrollbars replace pointer-position auto-scroll: bounded wheel,
  draggable thumb, keyboard reveal, independent Toolbar lists, no recurring timer.
  Short dialogs center on resize; larger editors stay top-left. Outside-dismiss
  remains a policy flag and cannot bypass unsaved-data confirmation.
- Toolbar-only 1×/2×/3× scales cells/icons/grip/separators and hit geometry.
  Normalized edge position survives resize on all four edges. Legacy pixel offsets
  migrate once. Menu/status/canvas UI scale stays independent.
- Mouse Hold activates before forwarding the initiating down, forwards release
  before restoring the previous tool, and cancels safely on focus loss. Explicit
  Right Mouse bindings work without losing the default unbound context popup.
- Reset Aspect Ratio restores the cropped source ratio while preserving center,
  displayed area, rotation, flips and crop, through one undoable semantic batch.
  Local and shared edits use existing history/authority paths.
- Note Enter preserves colored paper; Shift+Enter inserts a newline. Normal corner
  and side resize changes box/wrap only; Shift+corner scales the whole Note;
  Shift+side remains box-only. Paper adds at most six flat palette-derived rules,
  no texture or document objects. Numpad +/− dispatches contextual semantic edits:
  Note text size, drawing stroke or explicitly selected Rectangle fill opacity.
  Unsupported image/frame contexts report a no-op. Explicit text-size changes use
  bounded integer bitmap steps; imported/scaled legacy sizes remain intact.
- Rectangle Fill cycles OFF → 100 → 75 → 50 → 25 → OFF. Color Cycle changes
  both stroke and fill hue without altering alpha; object edits preserve future
  creation defaults. Each adjustment uses existing bounded history semantics.
- B opens compact camera-view capture with arbitrary legal key/pointer shortcuts,
  explicit collision consequences and Enter confirmation. Escape/outside/focus loss
  cancel safely. Up to 64 local profile views; no shared document mutation.
- Canonical transient feedback is the cached Tack status strip. The normal native
  title shows only Tack, file identity and dirty state. Native picker cancellation
  is a normal outcome; real helper failures still report errors.
- About uses the exact supplied transparent Tack logo at the top of the right
  column with only version beneath, then Captain Cool / Suspicious Sausage Records.
  Portrait touches the inner frame edges. Two bounded, nearest-sampled textures
  load on demand and release together; executable-relative user artwork wins over
  package fallback. All 50 baseline toolbar icon hashes and timestamps are intact.

See [architecture note](ARCHITECTURE_2A7_UI_PRIMITIVES.md) and
[independent review](reviews/phase2a7_ui_input.md). Six input/capture/keyset/Note
ownership findings were fixed and covered by focused regressions before delivery.
No GUI framework, dependencies, workers, sockets or services were added.

## Automated evidence

The pinned quality gate passes fmt, check, Clippy with warnings as errors,
**380 Rust tests**, docs, **28 Python tests**, and cargo-deny 0.20.2. The final
removal of the redundant Mode footer was additionally checked with 49 app-library
unit tests, fmt, workspace Clippy and both delivered native harnesses. Software
Vulkan passes **1 smoke + 1 selection + 10 product GPU tests**, including exact
nearest/alpha logo pixels and 20 artwork open/close cycles with texture release.
[Validation record](../benchmarks/phase2a7/validation.json) retains log hashes.

[UI receipt](../benchmarks/phase2a7/ui.json): **43 PASS**, delivered Linux hash.
Exercises Keymap grid/collision/Save/Save As/Load, wheel/thumb/no-hover-scroll,
Middle Mouse Hold Pan, Right Mouse view recall, Note finish/newline/Numpad,
Rectangle color/alpha persistence, native cancellation, public title, idle and
unsaved confirmation. About centering/nearest pixels and proportional toolbar
resize pass at 800×600 with 1×/2×/3× toolbar, and at 1024×768.
[Selected unmodified captures](../benchmarks/phase2a7/ui/) are retained.

[Three-client receipt](../benchmarks/phase2a7/shared-three.json): **19 PASS**,
delivered Linux hash. Repeats 2A5/2A6 lease, parallel gesture, menu, duplicate/Undo,
owner crash/reclaim, disconnect, local-view and idle checks. Adds actual native
image distortion → Reset Aspect Ratio → exact Undo and Rectangle fill/context
opacity → exact Undo, with three-client metadata convergence. Owned software
X11 clients on one host do not establish physical two-computer LAN acceptance.

Windows x64 package includes static libjpeg-turbo 3.2.0 SSE2/AVX2 decoder and
checks imported DLLs. [Windows execution receipt](../benchmarks/phase2a7/windows.json)
records CLI JPEG create/reopen, scalar/SSE2/automatic decoder execution and identical
output under Wine. Physical Windows UI acceptance remains pending.
Linux/Windows/dependency CI evidence will be recorded after implementation push.

## Measured cost against 2A6

Two serial paired runs on the same owned Xvfb display, explicitly selected
llvmpipe Vulkan, same small local board, warm/unspecified OS page cache. Final
Linux executable hashes are used in both repeats. [Raw paired observations](../benchmarks/phase2a7/local-paired.json)
and [full frame telemetry](../benchmarks/phase2a7/local-frame-telemetry.json) are retained.
No simultaneous hardware-GPU stress was launched.

| Metric | 2A6 | 2A7 |
| --- | ---: | ---: |
| External Cargo dependencies | 278 | 278 |
| Stripped Linux executable | 19,968,512 B | 20,036,896 B (+68,384; 0.342%) |
| Settled local threads | 39 | 39 |
| Mean settled RSS | 107.67 MiB | 107.27 MiB |
| Accounted scene GPU peak | 104,960 B | 104,960 B |
| Idle CPU ticks / redraw / app I/O / IP sockets | 0 / 0 / 0 / 0 | 0 / 0 / 0 / 0 |
| Mean native startup | 71.15 ms | 58.72 ms |
| Mean first frame | 73.75 ms | 62.31 ms |

Callbacks contain only three startup samples per run; medians range 3.94–4.11 ms
baseline and 3.69–3.77 ms current. This small software-driver sample establishes
no recurring work and no extra permanent thread; it does not establish a general
hardware, startup or huge-board speed improvement. Existing decoding/streaming
paths are retained. About textures exist only while open.

## Resources, incident and acceptance

Shared build targets reused; one 20 MB baseline retained. Compact committed proof
is below 256 KiB; generated phase evidence is well below 512 MiB. Owned Wine
scratch has a documented temporary allowance of 2 GiB, shut down and removed
by the harness. Superseded generated attempts are inspected and selectively
removed after retaining their outcome summary and complete delivered evidence.
At least 10 GiB disk reserve remains. No user artwork/board/profile/brief or broad
cache tree is deleted; [preservation receipt](../benchmarks/phase2a7/artwork-preservation.json)
records all baseline icon hashes/timestamps unchanged and identical logo copies.

The [2026-10-10 whole-system incident](INCIDENT_2026_10_10_SYSTEM_CRASH.md)
remains undetermined: persisted root-only EFI kernel trace awaits a readable
owner copy. These software checks neither diagnose nor attribute that crash.

[Human checklist](HUMAN_TEST_2A7_UI_POLISH.md) remains unchecked for artist feel,
physical Windows and two-computer LAN. Plain Text, minimal quadratic curves,
Studio Server, WAN/auth, media, importers and all other deferred work were not
started. Stop after Phase 2A7.
