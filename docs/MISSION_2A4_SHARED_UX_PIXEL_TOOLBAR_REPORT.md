# Phase 2A4 — desktop sharing and primitive pixel toolbar

**Technical verification PASS; exact-source Linux/Windows CI green. Physical
two-computer LAN acceptance remains PENDING.** One Linux machine with multiple
real native processes and its real LAN address establishes the technical workflow,
not partner connectivity, firewall behavior or human usability on two machines.

Baseline implementation: `e059d6d4707d6bbf4944a54c6f46b001102ed809` (Phase 2A3).
Implementation: `390608762d5502cdfe7b64c66370b7c8ea6632b5`.
[Exact-source Quality CI](https://github.com/gurppt/tack/actions/runs/37983915478)
passed all three Linux/Windows/dependency jobs.
Compact measurements/hashes: [phase2a4.json](measurements/phase2a4.json).
Independent review: [phase2a4_shared_ui.md](reviews/phase2a4_shared_ui.md).

## Delivered behavior

File → Share Board → Share from this computer → Start Sharing proposes a
`-shared.tack`, snapshots an independent document incarnation and opens a hosted
window. The original local window/file stays independent. Tack explicitly starts
and owns the existing sibling server; users do not launch a server command.
Copy Invite and File → Join Shared Board → Paste invite use existing canonical
URI parsing, publication, clipboard and joined-canvas authority. Primary panels
and ordinary shared titles avoid address/port/Board ID/revision jargon; Advanced
keeps technical details. No presence count is fabricated.

The bounded `.tack.sharing.json` companion binds a stable Board ID to the shared
copy. Reopen offers read-only offline inspection or Put Online; the same hosting
profile reuses committed server authority and the old invitation. Stop/host close
checkpoints accepted authority and originals through existing checked streaming
code, reaps the server and leaves an inspectable offline snapshot. Offline Save,
edits, Undo/Redo and recovery cannot become collaborative authority. Keep both
the companion and hosting profile/authority; filename alone does not identify
a hosted board. Details: [desktop lifecycle](design/shared_desktop_lifecycle.md).

One toolbar projects up to 32 existing semantic actions. View provides Toggle
Toolbar, Edit Toolbar and Toggle Status Bar; all are remappable. Six placements,
floating grip, catalog Add/Remove/Up/Down/Reset and persisted action IDs use direct
rectangles and stack geometry. Pan now uses existing camera pan on left drag.
There is no widget/docking framework, watcher, SVG runtime or new dependency.
Unknown action IDs normalize away. Static online/connecting/offline blocks use
green/yellow/red rectangles without blink timers. Hover reads the active keymap;
a native screenshot verifies Arrow `[F11]` after remapping.

Sixteen original hard-alpha PNG16 symbols live in `gfx/icons/`. Editing a PNG and
restarting the real build changes it without a source rebuild. Missing, corrupt,
wrong-sized or soft-alpha files get a deterministic placeholder and diagnostic.
The 128×32 RGBA atlas loads once; nearest expansion is verified pixel-for-pixel
at 1× and 2×. [Format and implementation costs](design/pixel_toolbar.md).

## Validation

All expensive work ran serially on owned X11 `:99`, RTX2060, without the user's
desktop or concurrent compiler/benchmark. Sources were frozen before the final
receipts; `source_manifest.json` hashes 337 relevant tracked/owned source files.
Its SHA-256 is `4de3f397a2521647e1627595ab3b6cf8eb29ce4380ee96395ab5cfaf1e027817`.
Raw data remains under ignored `benchmark-results/phase2a4/`; compact receipt
hashes are published in the measurements file.

| Check | Result |
| --- | --- |
| Exact-source Quality CI | Linux + Windows Rust and dependency jobs PASS |
| Full `tools/check.sh` | fmt/check/Clippy/docs, 340 Rust + 27 Python tests, dependency checks PASS |
| Explicit hardware GPU suites | 14 PASS: smoke, selection, product, LOD convergence and long churn |
| Independent contracts | 5 PASS within the gate: geometry/bounds, migration, fork/read-only, companion, icons |
| GUI sharing | 12 PASS: named fork, original preservation, Copy Invite, partner join/edit/camera, Stop, offline rejection, same-ID restart/old link, clean close |
| Managed server EOF | 6 PASS: accepted edit, changed checked snapshot, stable ID, persistent authority, reaped exits; direct EOF simulation, not physical machine crash |
| Toolbar native | 18 PASS: six placements/resize, reconfigure/reset, floating restart, Rectangle/Undo/Pan, releases across chrome, PNG edit/nearest/fallback |
| 2A3 daily / About | 20 / 41 PASS |
| Shared authority regression | 32 PASS, including CAS, refusal, undo, reconnect and remote active-gesture cancellation |
| Huge JPEG / potato / mixed | 33 camera poses PASS; visited cache reuse/reopen; no fixture corpus regeneration |
| LOD churn | 2,520 wheel + 32 pan + 5 filtering transitions, settled/reopen PASS |

Concrete pilot fixes: relayout on resize; canvas-owned releases passing through
chrome (including held pan); managed-server and launched-window child reaping;
bounded host-identity reads; consume Put Online once; readiness requiring a real
connected snapshot; initialize new modal hit testing from the actual pointer.
An isolated-LAN interface fixture replaces a default-gateway assumption. Pilot
profile/clipboard timing mistakes were harness issues, not claimed product bugs.

## Performance and disk cost

Same release/strip mode: client **19,671,488 bytes**, versus 2A3 **19,450,176**:
**+221,312 bytes (+1.14%)**. Current server is 1,854,888 bytes. The human package
is stripped while symbol-rich builds stay in the shared Rust target directory.
`bin/tack` SHA-256: `57adcef9023e39046ffe22cb0d1418dccfecace99520edfb67a7f93592a7171c`.
Decoder/About assets and Cargo dependency declarations/lockfile are unchanged.
The server dependency tree still contains no UI, renderer or image codecs.

Two paired warm/unspecified-cache runs, native 800×600, same hardware/display:

| Local board | 2A3 startup median | 2A4 visible median | RSS median delta |
| --- | ---: | ---: | ---: |
| Empty | 460.0 ms | 451.8 ms | +406 KiB |
| 1,000 objects / 64 image sources | 455.1 ms | 440.1 ms | +228 KiB |

Twelve observed local runs include visible, hidden and hidden-with-status-off.
All have **41 threads, zero IP sockets, zero measured idle I/O and zero recurring
idle redraws**. Process CPU sampling includes existing GPU/driver work: 0–7 ticks
over ~3 seconds, with comparable baseline ticks; no CPU-zero claim. Two pairs
do not establish a speedup or a statistical startup bound. RSS includes the modern
driver (~334–360 MiB); this is not measured P3/128-MiB hardware acceptance.

Atlas pixels: **16,384 bytes**; total atlas + fixed GPU vertices: **20,224 bytes**.
Startup decoded pixels are released after upload. Hidden submits zero icon quads
but retains those tiny GPU resources. Default toolbar JSON is **268 bytes**
(serialized state, not an allocator/RSS estimate); hidden 271 bytes. Static UI
uses stack geometry; status text allocates on hover/state changes, not idle frames.
Explicit hosting/child-window waiters exist only after those user actions and
must not be confused with ordinary local-startup worker cost.

Generated phase evidence is below the 512-MiB budget (about 54 MiB before closeout).
Only superseded owned pilot directories were removed (about 1.5 MiB); final raw
receipts/profiles/captures, reused corpus/baseline and all owner files remain.
About 21 GiB remained free, above the 10-GiB reserve. No broad cache wipe or full
Cargo clean was performed.

## Product answers and remaining acceptance

1. **Terminal-free LAN share/join:** implemented and exercised through actual
   GUI controls; physical partner acceptance is still pending.
2. **Close/reopen and old invitation:** verified on the same host/profile with
   a stable reachable IP/port. DHCP/adapter/firewall changes are separate LAN work.
3. **Ordinary local collaboration cost:** no additional collaboration socket or
   worker at startup; measured local thread count matches baseline and idle stays
   free of toolbar/status redraw/I/O.
4. **Primitive UI:** fixed action/rect arrays, bitmap text, one tiny atlas and
   existing dispatcher; no modern UI framework underneath.
5. **Edit PNG/restart:** verified in the shipped binary with exact pixel sampling
   and safe fallback. Artwork/ergonomics remain owner decisions.
6. **Later company server:** high-level Share/Join and existing transport boundary
   are retained; numeric remote-server alternative exists. DNS/TLS/auth/public WAN
   deployment are not implemented or accepted by this phase.

One desktop-hosted board owns default port 7337. Original shared cache admission
and silent TCP interruption behavior retain existing limits. Borderless mode,
cache-management UI, menu/keymap icon decoration, presence and general offline
merge are deferred. These are not hidden implementation claims.

Ready build: `./bin/tack`, sibling `./bin/tack-server`/decoder/assets.
Follow [HUMAN_TEST_2A4_LAN.md](HUMAN_TEST_2A4_LAN.md) for the required two-computer
acceptance. Only subjective artwork/wording comfort is added to
[HUMAN_REVIEW_PENDING.md](HUMAN_REVIEW_PENDING.md). Stop here; no WAN or next phase
was started.
