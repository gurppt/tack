# Phase 2A5 — concurrent editing and interaction corrective

Date: 2026-10-10. Baseline: phase 2A4, repository checkpoint `d8e0cdcc`.
Status: **Linux technical checks PASS; physical two-computer acceptance PENDING.**
The Windows x64 SIMD package is built and its CLI/decoder execute under Wine.
The exact remote CI run is recorded below when available. No physical Windows desktop acceptance
is claimed. The next drawing/text mission has not been started.

## Result

The authoritative server orders and persists all accepted edits as before, but
conflicts now use affected object stamps and explicit dependency/order barriers.
A newer board revision alone does not cancel an unrelated gesture. Temporary,
connection-owned leases arbitrate continuous manipulation; foreign objects have
a red pixel outline. Menus, camera, tools, selection, key capture and unrelated
drafts retain local state through unrelated remote operations.

Share transitions the current view after successful publication and snapshot
preparation, keeping the original local file untouched. Its connection panel
confirms that the original remains on disk. A noticed disconnection immediately
opens a centered explanation. Save to Local streams complete verified originals
into a new owned asset folder and publishes a fresh editable local identity,
without a sharing companion or offline merge. Missing originals refuse this
operation. Close Board checkpoints/reaps a hosted server and returns to an empty
Tack window; dirty local files retain Save/Discard/Cancel.

## Corrective behavior

| Area | Implemented behavior |
| --- | --- |
| Conflicts/undo | Captured request bases, durable object clocks/tombstones, conservative metadata barriers and inverse ownership checks |
| Leases | Atomic acquisition, 5-second TTL, active-only renewal, disconnect cleanup, 256 targets/connection and 1,024/board |
| Snapping | One raw-to-snapped preview update avoids rotated-move oscillation; 15° rotation and temporary Shift use the same snap engine |
| Selection chrome | Rectangle/Frame outlines sit outside the object's own edge |
| Toolbar | Centered edge defaults, explicit persisted offset, remembered visible placement, two-column Add/Remove/reorder/Reset editor |
| Preferences | Typed three-way save preserves other windows' recent-file updates; genuine same-field conflicts show status |
| Shared UI | Bottom status even with local strip hidden, readable invite, one-shot COPIED feedback, semantic palette accents |
| Keymap | Double-click capture, collision reassignment unbinds the old action, real Normal/Hold tool semantics, separate preferences/keymap exports |
| Views | B then a mapped digit stores a local camera slot; a digit recalls it; Escape cancels |
| Annotations | Remappable selection filter, 3-pixel creation strokes unaffected by object editing, readable small arrowheads |
| Notes | Enter finishes; Shift+Enter inserts a newline |
| Duplicate | Fresh IDs, metadata-only asset reuse, groups preserved, shared selection applied after acceptance, one undo step |
| Rectangle | Reverse/tiny creation, move/resize/rotate, duplicate/filter/style and exact undo regression coverage |
| Z-order | Ctrl+Up/Down for one step, Ctrl+Shift+Up/Down for ends; displaced layout defaults use Ctrl+Alt+Up/Down |
| Frames | Gray default, existing palette cycle, width-based wrapping and global integer title scale (default 2×) |

Existing customized keymaps are retained. Reset adopts changed defaults; digits
are logical shortcuts, allowing number-row and NumLock-enabled keypad input.
Camera slots are limited to 10 per board and 64 total profile entries. They are
local preferences, never a shared document edit. Older named bookmark management
remains available.

## Validation

The final local gate passes formatting, check, Clippy with warnings denied,
**350 Rust tests**, documentation, **28 Python tests** and cargo-deny 0.20.2.
Advisories, bans, licenses and sources pass; inherited duplicate-version warnings
remain informational. Explicit NVIDIA Vulkan renderer tests pass: 1 GPU smoke,
1 selection test and 9 product tests.

[Three-client evidence](../benchmarks/phase2a5/native-three.json) records **16
passing checks** using three native Linux clients on separately owned displays
and an actual NVIDIA RTX 2060 Vulkan adapter. It verifies simultaneous image
move/scribble, separate rotate/move, same-object denial/red outline, menu survival,
safe exact-target deletion, abrupt owner's lease reclamation, shared Duplicate
and Undo, local B7/B8, disconnect before failed editing and settled idle redraw.
TCP integration tests additionally cover atomic multi-object denial, expiry,
socket cleanup, old-base independent edits and persistence/restart.

[Desktop evidence](../benchmarks/phase2a5/desktop-ui.json) records **13 passing
checks**: repeated toolbar toggles on five visible placements and Hidden restore,
editor Reset, native shortcut capture/collision, same-window Share, unchanged
original, invite Copy, managed server reaping, editable Save to Local, Close Board,
and persisted Note text with Shift+Enter/Enter. Temporary test profiles/files and
owned X servers isolate these checks from user boards and desktop input.

Native testing exposed and fixed an additional B release defect: consuming that
release left InputState believing B remained held, so a second assignment did
nothing. The final B7/B8 native check covers the corrected behavior. Earlier
automation failures also identified harness assumptions about initial camera
choice, sRGB red thresholds, modal search Escape and X server readiness; those
failed attempts remain separate from the final passing receipts.

## Performance and bounds

Two serial paired local runs use the same small image board, host and display.
[Raw local measurements](../benchmarks/phase2a5/local-idle.json) retain exact binary
hashes, startup timings, process snapshots and frame evidence. Warm/unspecified OS
cache, two repetitions and a modern GPU limit statistical and low-end conclusions.

| Measurement | Phase 2A4 | Phase 2A5 |
| --- | --- | --- |
| Local settled RSS | 334.61–334.77 MiB | 334.53–334.54 MiB |
| Local threads | 41 | 41 |
| Local IP sockets | 0 | 0 |
| Idle redraws over 3 seconds | 0 | 0 |
| Process CPU ticks over 3 seconds | 1, 1 | 0, 0 |
| Native startup | 433–474 ms | 462–487 ms |
| First recognizable content | 459–498 ms | 486–512 ms |
| Stripped Linux application | 19,671,488 bytes | 19,947,776 bytes (+1.40%) |

Startup medians differ by about 21 ms in this small sample; it does not establish
a significant improvement or regression. Local boards acquire no collaboration
workers/network activity. In the settled three-client shared interval the server
uses zero CPU ticks/data I/O; clients use 0/1/1 ticks, no data I/O and no redraws.
Earlier intervals showed 1–2 ticks in NVIDIA driver threads, retained in raw
attempts rather than described as application heartbeat work.

The server's empty process RSS is 2.72 MiB; after three joined native clients,
4.10 MiB. A single active lease produces no measurable additional RSS in that
sample. The explicit capacity measurement below covers the full 1,024-entry cap;
RSS includes JSON parsing, queues and allocator behavior, not exact entry sizes.
On the separate 1,025-Frame/five-connection fixture,
[capacity evidence](../benchmarks/phase2a5/lease-capacity.json) shows RSS stable
at 8.45 MiB for 0/256/512/768/1,024 leases, refusal of the 1,025th target, zero
CPU/I/O over 5.2 seconds across TTL expiry and an empty table on explicit rejoin.
Allocator reuse means this does not estimate a literal zero-byte lease entry.
Six injected edit-to-all-receipt samples include roughly 400 ms of scripted mouse
delays and polling; they are upper bounds for the injected workflow, not pure
transport latency or monitor response measurements.

No Cargo dependency or lockfile change was introduced. Clock tombstones cap at
4,096; eviction raises the durable stale-base floor. Native/control/event/server
queues retain explicit existing caps. Renewal and expiry deadlines exist only
while manipulation leases exist; settled idle has no permanent timer.

## Builds and disk discipline

Linux: `bin/tack`, sibling server, JPEG decoder, icons and About artwork.
Windows: `bin/tack-windows-x86_64.zip`, including `tack.exe`, sibling server,
statically compiled libjpeg-turbo 3.2.0 SIMD JPEG decoder and artwork/notices.
The package checks SSE2/AVX2 symbols and rejects external MinGW/turbojpeg runtime
DLL imports. Build source archive SHA-256 and NASM/compiler provenance live in
the package's `BUILD.json`/`LICENSES/native-build.json`. Reproduce with
`tools/prepare_turbojpeg.py --target windows-gnu` and `tools/build-windows-gnu.sh`.
The fresh-download target-shadowing bug is covered for native/cross preparation.

[Windows execution evidence](../benchmarks/phase2a5/windows-execution.json)
records creation/reopen of an embedded JPEG board using `tack.exe` and identical
160×100 decoder outputs with forced scalar, forced SSE2 and automatic SIMD
dispatch. AVX2 linkage is checked separately; the runtime routine is not
instrumented. This is execution under Linux Wine, not
physical Windows desktop validation. The ZIP is about 10.8 MiB; its Windows
application is 24,311,296 bytes, server 3,095,552 bytes and decoder 872,960 bytes.
Only system runtime DLLs are imported.

Builds reuse the shared target directory and run serially with GPU measurements.
Phase fixtures/baseline/captures use about 22 MiB; at least 10 GiB stays reserved
(about 17 GiB free after builds). No user data/profile/cache directory is cleared.
**Temporary verification exception:** an owned Wine prefix may use up to 2 GiB
to execute the Windows CLI/decoder. It will be removed after verification; this
is tool scratch, not retained phase fixtures or a Windows desktop PASS. The first
attempt initialized about 1.6 GiB, above its planned 1 GiB estimate, and was fully
removed; the documented bound was revised before the next verification run.

## Compatibility, review and remaining acceptance

LAN protocol major **2** requires matching upgraded clients/server. Authority
JSON remains version 1 with an optional compatible clock field. Nondefault
Frame colors use optional storage schema **5**; default-only boards retain the
earlier applicable schema. Older writers cannot edit schema-5 boards safely.

[Independent source review](reviews/phase2a5_concurrency.md) passes after targeted
fixes, including adjacent-only undo rearming, commit receipt lease separation,
Duplicate selection and modal/camera state. Runtime/performance receipts were
produced by the integrator; they are not falsely attributed to a second execution
by that reviewer. The review is scoped to these paths, not a repository audit.

**Physical two-computer artist acceptance remains pending.** Follow
[HUMAN_TEST_2A5_LAN.md](HUMAN_TEST_2A5_LAN.md). Same-host clients, Wine execution and
CI cannot establish physical LAN/Windows desktop comfort or subjective snapping
quality. Consequently the brief's complete product PASS is not yet claimed.

Intentionally deferred: toolbar drag reorder (keyboard/buttons implemented),
quadratic Bézier, Post-it redesign, Plain Text, media/audio, WAN/TLS/accounts,
presence/cursors/chat, offline merge and generalized docking/vector frameworks.
