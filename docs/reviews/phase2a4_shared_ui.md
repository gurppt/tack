# Phase 2A4 independent shared UI review

2026-10-09. Baseline implementation `e059d6d`. Scope is the current bounded
Share/Join lifecycle, shared-copy companion, managed server checkpoint,
toolbar/icon/status primitives and their event/worker integration. The reviewer
read the mission, relevant existing authority design and contributing rules,
and directly inspected the supplied Deluxe Paint reference. No historical
whole-repository scan, Cargo, native/GPU test or subprocess benchmark was run.

Status: **LOCAL TECHNICAL VERIFICATION PASS — exact-source CI pending**. No
unresolved concrete defect remains in the reviewed paths and final local
receipts. Physical two-computer LAN acceptance is **PENDING**: it is the mission's
primary objective acceptance gate, distinct from subjective icon/geometry
comfort. One-host automated GUI results do not complete that physical gate.

## Concrete findings

1. **Resize did not relayout toolbar geometry.** Dock Right/Bottom kept old
   screen coordinates; a max-entry toolbar would not wrap after resizing a
   larger window to 800×600. The integrator added `chrome_layout()` after
   `camera.resize` in `WindowEvent::Resized`. Source correction verified;
   final native toolbar receipt covers all six placements and resize/persistence.
2. **Chrome swallowed canvas-owned release events.** A canvas image/scribble
   drag released over the toolbar never received its release, leaving the
   gesture captured. An `input.active()` guard closes that part, but initially
   missed held PanView/CenterPointer: those are held in InputState and are not
   included by `active()`. Middle/Alt-left pan released above the toolbar could
   continue moving after the button was up. Chrome now forwards every mouse
   release except its own floating-grip drag completion; canvas holds receive
   their release even when `active()` is false. Source correction verified;
   native pan, image and scribble cross-toolbar releases all pass.
3. **Cancelled ready host could remain unreaped.** `LocalWorker::poll` discards
   a cancelled `Hosted(Ok(...))`; `Hosted::Drop` closed stdin but did not reap
   Child. The server correctly observes EOF and checkpoints/exits, but dropping
   std Child does not wait/reap it. On Unix a cancelled result can therefore
   leave a zombie until the still-live parent exits. The integrator now transfers
   Child into a reaper created before the server is spawned. Creating that
   waiter must succeed before any server exists; it blocks on a private channel
   during hosting. Drop closes stdin and sends its sole Child without spawning
   a thread or waiting in the callback. The waiter permits checkpoint completion
   for 60 seconds, then kill/wait. Normal Stop/abort reaps on the operation worker
   and closing the unused sender terminates the waiter. Source correction
   verified; final GUI Stop/host-close checks reap the server, and direct EOF
   checks export accepted authority and exit. EOF simulation is not a physical
   machine crash or forced allocation/thread failure claim.
4. **Host ownership identity read was unbounded.** `sharing::owns` read the
   entire profile `hosting-identity` whereas its creation/validation path limits
   reads to 65 bytes. Corrupted oversized metadata could allocate proportionally
   during Put Online. Comparison now reads at most 65 bytes and validates WireId.
   Source correction verified; oversized identity refusal added to the companion
   contract test, passing in the final gate.

Two integrator-found fixes were also reviewed: `--put-online` is consumed once
when installing the offline snapshot, so Stop Sharing/reload cannot silently
restart hosting; join readiness requires an actual connected server snapshot,
not the pre-existing offline editor. Final lifecycle checks pass.

A native toolbar pilot also exposed a stale modal pointer: after reopening its
editor, a click at the unchanged Position coordinates used a new LocalUi cursor
of [0,0] because X had no reason to emit CursorMoved. App now tracks pointer before
modal routing and seeds `LocalUi::set_pointer` before every handle call. This
restores unchanged-coordinate hit tests without querying the OS or creating a
timer. Source correction reviewed; final repeated-click/placement sequence passes.

A later native pilot found a distinct zombie for a successfully launched native
window, after its server had already been correctly reaped. `owned_window::adopt`
now owns successful `join_launch` and ordinary `Operation::Launch` children.
It admits at most 16 active windows per parent, blocks on Child::wait only in an
explicit-child waiter thread, and releases the slot after completion. Admission
or waiter-spawn failure kills/reaps the child on the operation worker; spawn
failure retains child ownership through the temporary Arc/Mutex. No waiter is
constructed at local startup and no polling timer is added. This source fix is
reviewed; final native closure waits for removal of the owned host PID. Earlier
pilots are diagnostic findings, not substitute final PASS receipts.

## Source boundaries reviewed

- Share snapshots a document fork with a new DocumentId, retaining source/object
  identities and bounded durable metadata; it does not mutate the local original
  or create collaborative offline merge. Relative linked paths are resolved for
  the shared incarnation. Existing source publication and server authority are
  reused. New destination lease and companion publication refuse overwrite.
- A checked <=4-KiB versioned companion binds URI Board ID to snapshot DocumentId
  and records hosting identity separately from filename. Reopen uses the same
  stable identity and server root. Existing server authority is reused instead
  of republishing over it. Sidecar is local lifecycle metadata, not server edit
  authority. Losing the companion is not a general offline merge mechanism.
- Offline snapshot uses an unwritable shared DocumentEditor, suppresses local
  save/recovery authority, and rejects Duplicate/bookmark edits. The existing
  backend alone supplies accepted online state. Original local board remains
  independently editable in its original window.
- Managed server starts only on explicit Share/Put Online, receives a parent-owned
  stdin pipe and exits on EOF. Its checkpoint shuts subscribers and holds the
  existing authority mutex while streaming the durable authority into the
  offline snapshot. Stop work belongs to the temporary operation worker; no
  service/daemon/autostart or local-board server construction was added.
- Toolbar is one <=32-action list projected through the existing semantic
  dispatcher. Placement uses fixed stack geometry and direct hit rectangles;
  configuration stores action IDs/placement, not duplicated tool logic. Unknown,
  held-only and duplicate entries normalize away. Temporary customization panel
  uses the existing immediate panel primitives.
  The previously inactive existing Pan tool now projects ImagePointer hold into
  the existing Camera::pan path, cancelling transient image preview and leaving
  document authority untouched. Release still uses the existing InputState path;
  no parallel tool/controller was introduced. Grab cursor advertises that mode.
- Sixteen external hard-alpha 16² PNGs load once at startup, each encoded file
  <=16 KiB, through the existing bounded UI PNG decoder. Fixed 128×32 RGBA atlas
  is 16,384 bytes plus a bounded 32-quad vertex buffer, sampled nearest. Missing,
  corrupt, wrong-sized or soft-alpha input gets a deterministic placeholder.
  No SVG runtime, watcher, icon decode on redraw, new renderer framework or
  docking tree was introduced.
- Static toolbar drawing uses stack arrays. Hover text changes on hover/state
  transitions and reads current semantic keymap shortcuts. Stable green/red
  status uses rectangles with no blink/redraw timer. The shared backend's
  existing explicit-session networking is distinct from ordinary local idle.
- Normal connecting/shared title paths now show artist-facing shared state.
  Raw address, Board ID, revision and reconnect debug text remain available only
  with explicit report output or `TACK_NATIVE_DIAGNOSTICS`; Advanced continues
  to expose the technical connection details deliberately.
  Current chrome/status and both title paths classify Connecting/Reconnecting
  consistently as transitional Connecting/yellow rather than Offline/red.

The original route-to-192.0.2.1 selector could fail on an isolated LAN with no
gateway. It has been replaced by an explicit bounded interface query (`ip -j -4`
on Linux, PowerShell IPv4 query on Windows), using the existing cancellable
capture helper with a 64-KiB/5-second limit. A no-gateway fixture covers selection.
This is interface selection, not mDNS or ongoing discovery. Multi-interface
address suitability still requires physical LAN acceptance. WAN/TLS/auth are
outside this mission; the existing explicit server-address boundary remains.

## Independent contracts and remaining evidence

The reviewer added only `crates/tack-app/tests/phase2a4_contracts.rs`: five tests
for max toolbar geometry/hit semantics at 800×600 and scales 1/2/4; old preferences
and action-ID migration; fork identity/offline editor rejection; companion
identity/bounds/create-new publication; and valid/edited/missing/corrupt/wrong-size
and soft-alpha PNG handling. Fixtures are tiny owned generated PNGs and metadata,
with temporary cleanup, no subprocess/GPU/network/dependency additions. Rustfmt
was applied. All five contracts, including oversized identity refusal, pass in
the final coherent gate. The reviewer did not run Cargo or native work.

## Final local evidence

The reviewer recomputed the 337 file hashes in `source_manifest.json`: all match
the current relevant tracked and explicitly owned new source files. Owner
untracked files are excluded. Manifest SHA-256:
`4de3f397a2521647e1627595ab3b6cf8eb29ce4380ee96395ab5cfaf1e027817`.
The manifest records raw build hashes and binds the four packaged artifacts.
Each artifact's recomputed hash matches both `final-build/manifest.json` and
the shipped `bin/` file. Final client SHA-256:
`57adcef9023e39046ffe22cb0d1418dccfecace99520edfb67a7f93592a7171c`;
server: `d61405b82b61b570145b13a9dd5b39ca2d3ae0ac82273347bdb059a0b3ce90b1`.
The integrator states the gate follows the final production pointer/Pan/reaper
changes; subsequent changes are report/harness/package work, no production edits.

- `full-gate.log`: 340 Rust tests, 27 Python tests, formatting/Clippy and
  cargo-deny checks pass. Five `gpu-*.log` receipts contain 14 passing tests.
- `shared-final/receipt.json`: 12/12 GUI Share→Copy Invite→Join→edit convergence
  →Stop→reopen→Put Online checks pass, preserving local original and old invite
  identity, independent camera and read-only offline snapshot. Server and
  closed host PID removal are observed. Scope is two GUI sessions on one Linux
  host using its real LAN address, not two physical computers.
- `managed-final/receipt.json`: 6/6 direct EOF checks pass: accepted edits stay
  server authority while online, EOF exports a changed checked snapshot with
  stable ID and exits/reaps, restart reuses that authority, second EOF cleans
  up. Only 63,837 fixture bytes are copied. This models lifetime-pipe closure,
  not a physical host crash.
- `toolbar-shipping/receipt.json`: 18/18 checks pass: six positions, persistence
  and native restart, customization/reset, floating drag, annotation semantics
  and Undo, Pan/image/scribble release above toolbar, existing Pan camera motion,
  exact edited PNG nearest pixels at 1×/2× and corrupt-icon fallback. The reviewer
  visually inspected `icon-1-edited-png.png`: compact 800×600 toolbar and status
  `Arrow [F11]` displaying the actual remapped shortcut. UI raster/style comfort
  remains an owner decision. Default configuration is 275 serialized bytes;
  GPU atlas plus bounded vertex buffer is 20,224 bytes, no document image cache.
- `daily-final` 20/20, `about-final` 41/41 and `authority-final` 32/32 pass.
  Native Shared/Managed/Toolbar/Daily/About harness hashes match their current
  scripts. About artwork/decoder artifact hashes are unchanged.
- Square normal/potato/mixed final receipts cover 33 settled poses, complete
  requested supply, same-session return with zero additional regional jobs or
  source bytes, and reopen hot with six raw-cache hits and no source work. CPU/GPU
  payload peaks remain 6,971,744 bytes normal, 6,172,976/6,971,744 potato and
  8,455,008 bytes mixed. Quiet I/O is zero. LOD final records 2,520 wheel, 32 pan
  and five filter actions; 222 churn plus 11 reopen trace frames have zero
  supplied-pixel valleys and zero idle redraw. Native LOD does not force
  evictions; separate GPU churn evidence covers that boundary.
- Idle paired/extra receipts contain 12 runs: all 41 threads, zero idle redraw,
  zero I/O delta and only Unix/X11 sockets, no IP socket. All 12 underlying report
  hashes match. CPU ticks are 0–7, with comparable baseline GPU/driver activity:
  no zero-CPU claim. Two-observation startup medians baseline/current visible are
  460.01/451.77 ms empty and 455.11/440.13 ms for 1k images; hidden/bare current
  medians are 450.99/468.71 ms. Warm/unspecified page cache and one host do not
  establish a statistical speedup or legacy-CPU performance.

Final stripped client is 19,671,488 bytes versus Phase 2A3's 19,450,176 bytes
(+221,312, about 1.14%). No new GUI/docking/decode dependency was introduced.
Exact-source Linux/Windows/dependency CI is pending commit. Physical two-machine
LAN testing remains an objective acceptance item outside the subjective review
queue; Windows desktop runtime, multi-interface suitability and artist comfort
also must not be inferred from these automated local receipts.
