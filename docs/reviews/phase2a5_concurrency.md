# Phase 2A5 independent concurrency review

Date: 2026-10-10. Scope: the authoritative conflict/receipt path, transient
manipulation leases, native shared-event isolation, and the data-preserving
Share / Save to Local / Close Board transitions requested by the corrective.

**Source review: PASS for the inspected paths. Local gate and inspected Linux
native/GPU evidence: PASS. Windows/SIMD and exact-source CI acceptance: pending
final receipts. Physical two-computer acceptance: PENDING.** This is not a
whole-repository audit or a claim that all Phase 2A5 product criteria pass.

## Review method

The independent reviewer read the brief, the baseline authority/client paths,
and the corrective sources and tests. Concrete failure scenarios were sent to
the implementer while the changes were being written, then the corresponding
fixes were inspected. No Cargo compilation, native window, GPU benchmark, or
second test execution was launched by this reviewer. Implementation-worker
reports of passing commands are not substituted for final evidence receipts.

The later review read `/tmp/tack-2a5-gate.log` and
`/tmp/tack-2a5-last-{clippy,tests}.log`. The complete local gate contains no
failed Rust result, the 27 Python tests pass, and cargo-deny reports
`advisories ok, bans ok, licenses ok, sources ok`. The subsequent Clippy and
focused application tests also finish successfully, including all five
`phase2a5_corrective` tests. These logs establish local execution, not a
Windows desktop or physical LAN result.

Inspected source boundaries:

- `tack-server`: `authority.rs`, `object_clock.rs`, `service.rs`,
  `service/leases.rs`, and the focused authority/LAN tests.
- `tack-shared`: semantic `scope.rs`, protocol/framing, the control reader and
  sender, and streaming snapshot/download helpers.
- `tack-core`: the shared semantic request queue and captured request bases.
- `tack-app`: image gesture capture/commit, shared event and lease state,
  hosting transitions, local installation/updates, and independent-copy worker.

## Authority and conflict invariants

Global revisions remain the durable total order. Object clocks use the last
accepted global revision affecting each object; these are compact conflict
stamps, not an independent document or distributed merge model. Stamps,
bounded deletion tombstones, inverse history, and exact accepted receipts are
published together with canonical document state.

Commands retain their creation-time base. Transform gestures retain the base
captured when manipulation began. Submission does not replace it with the
latest unrelated revision. Existing sender/operation receipts are recognized
before conflict validation, preserving duplicate acknowledgement without
applying the same accepted edit twice.

Tombstones are capped at 4,096. Eviction raises the durable conflict floor
beyond the evicted deletion stamp. Thus an old command cannot recreate an ID
whose deletion history has been forgotten. Legacy authorities conservatively
seed their clock/floor at their current revision. Future bases and bases below
the retained floor are refused and require reconciliation.

Source/asset replacement includes its currently dependent image objects.
Group membership includes affected members. Descriptor/group dependencies
have a conservative barrier; index-based z-order edits have an order barrier.
Unrelated transforms and fresh-object creation do not acquire a global edit
lock. Domain validation and the server's durable total order remain decisive.

Undo validates its actual inverse targets against both the submitted base and
the history entry's accepted revision. History is copied before attempted
mutation, so refusal does not consume an undo. Chained undo rearming remains
conservative: it requires an exact adjacent global history boundary and equal
target sets. Interleaved history can therefore refuse an otherwise potentially
safe older undo; this preserves another client's accepted state.

## Transient leases and local state

Leases are owned by the server connection, board, and object rather than by a
claimed client ID alone. Acquisition is atomic across the requested targets.
Limits are 256 targets per connection and 1,024 leases per board; lifetime is
five seconds. Renewals occur only while a reservation is active. Leases are
not serialized into the document or authority file.

Incoming activity lazily retires expired reservations. EOF, replacement of a
duplicate-client connection, and bounded outgoing-queue failure release the
removed connection's reservations. Commands and undo/redo inverses are checked
against foreign live leases, so delete or undo cannot bypass manipulation
arbitration. Accepted edits retain the existing durable-before-broadcast order.

Native previews wait for acquisition. A new image gesture cannot reuse the
previous gesture's reservation while its commit awaits acknowledgement.
Foreign ownership or expiry affecting the exact owned targets cancels that
gesture. Remote expiry uses a one-shot deadline to clear the red outline;
there is no lease timer after all reservations disappear.

An unrelated accepted command no longer clears every queued edit, gesture,
menu, note draft, or tool. Target-intersecting commands cancel only affected
gestures/text/queued edits. Selection is pruned against live objects, and menu
actions recheck the current context before execution. Reconnect snapshots
still reconcile authority explicitly; they are not ordinary unrelated edits.

## Concrete findings resolved

1. **Undo could erase a different client's intervening edit.** The proposed
   relaxed history rearming allowed A edit, B edit, A edit, A undo, A undo to
   restore the pre-B pose. Exact-boundary rearming was restored. The focused
   `undo_does_not_rearm_across_another_clients_same_object_edit` test asserts
   refusal of the second undo and preservation of B's pose.
2. **A second gesture could borrow an earlier reservation.** The native owner
   remained granted while the previous commit was in flight. Admission now
   blocks a new image gesture until that reservation completes.
3. **Lease ownership loss could leave a stale preview.** The native state now
   marks an intersecting foreign grant/expiry as lost and cancels only that
   manipulation.
4. **Fallible board installation could detach the current backend too early.**
   Save to Local and Empty Board now install successfully before detaching
   shared/host ownership. Share snapshot preparation retains the original local
   board lease until the replacement is ready; transition failure aborts back
   to the original board.
5. **Local copies lacked a safe native linked-source path and directory sync.**
   The worker resolves native linked paths relative to the board, checks the
   fingerprint before/after a streamed copy, verifies embedded CRC or shared
   hash/length as appropriate, and synchronizes the companion directory before
   publishing the board that references it. The fork has a fresh document ID
   and no sharing sidecar. Failed unpublished copies clean only their own
   newly created companion directory; a published copy is retained on a later
   durability error.

## Acceptance boundaries

The follow-up source review also inspected these later corrections:

- Shared Duplicate retains the existing selection until its fresh IDs exist
  in a sender-owned authoritative acknowledgement. Refusal clears the pending
  selection; unaccepted objects are not treated as live authority.
- Keymap Normal/Hold presentation reflects actual binding triggers. Changing
  a tool to Hold binds `TemporaryTool` with a Hold trigger; Normal restores
  `SelectTool` with Press. Actions with fixed manipulation semantics do not
  pretend to offer an arbitrary mode. Capture resolves collision in a copied
  keymap and persists only a successfully validated result.
- Reusing a window clears a pending camera-slot assignment during successful
  installation, preventing a later key from assigning the previous board's
  captured view to its replacement. A later native reproduction also found
  that capture swallowed the release of the assignment key B. The corrected
  capture path forwards release to InputState, so B can start a subsequent
  assignment instead of remaining logically held.
- The offline sharing panel translates its bitmap quads to a centered integer
  position, and hit tests subtract the same panel offset. No animation timer
  is added by this layout change.

Native attempt `benchmark-results/phase2a5/native-three-3/checks.json` records
successful image-move/scribble coexistence and separate-object transforms,
then a failed red-outline pixel assertion. The corresponding screenshot was
visually inspected and shows a red outer frame around the rotated object.
The implementer identified an RGB threshold that excluded the framebuffer's
sRGB red `(248, 134, 153)` and corrected the harness threshold. The original
attempt remains failed; `native-three-9` subsequently provides the complete
succeeding receipt described below.

The three-connection server test covers distinct targets at the same base,
all-or-none multi-target acquisition, same-target refusal, lazy expiry, and
socket disconnect cleanup; its execution is present in the passing local gate.

Final Linux receipts were independently read and their assertions checked:

- `benchmarks/phase2a5/native-three.json`: **16/16 PASS**, matching the raw
  `native-three-9/receipt.json` byte for byte. Three native clients use three
  owned X11 displays on one physical Linux host. Evidence covers unrelated
  move/scribble coexistence, distinct transforms, same-object busy reservation
  (623 red pixels), menu survival, exact-target deletion, shared duplication
  and undo, B/Num7/Num8, crashed-owner reclamation, automatic disconnect status,
  and zero idle native redraws.
- `benchmarks/phase2a5/desktop-ui.json`: **13/13 PASS**. Its harness checks the
  same process transitioning on Share, unchanged original bytes, canonical
  clipboard invite, Stop checkpoint/reap before continuing, a fresh editable
  local fork without sharing companion, Close Board retaining the process,
  toolbar placements/capture, and Note newline/completion semantics.
- `benchmarks/phase2a5/local-idle.json`: two serial baseline/current pairs,
  all quiet with **41 threads, zero Tack TCP/UDP sockets, zero data I/O, and
  zero idle redraws**. Mean RSS is 334.6914 MiB baseline versus 334.5313 MiB
  current. Native startup medians are 453.76 ms and 474.51 ms respectively;
  two repetitions do not establish a statistically significant startup trend.
- `/tmp/tack-2a5-gpu.log`: **11/11 PASS** across texture upload, selection,
  and product bitmap/grid/annotation/sampling tests. The reviewer read this
  log and did not rerun a GPU job.

Both native receipts identify client SHA-256
`a44797c860d0ca2ca7a45a7fbe87bc3d61efdb77a338687c2936b16cfbce8a85`,
which matches the inspected `bin/tack`; their recorded harness hashes match
the corresponding checked-in scripts.

The three-client receipt measures server RSS at 2,850,816 bytes before joins,
4,300,800 with three clients, and the same 4,300,800 with one active lease.
This demonstrates a small single-reservation increment at allocator/RSS
resolution, not a maximum-capacity lease-table measurement. Settled server
threads accumulate zero CPU ticks; clients accumulate 0/1/1 ticks over three
seconds and zero data bytes. One client records a single read syscall without
read bytes, so process-wide syscall counts are not all zero. Driver work is
visible and is not described as zero client CPU.

Six measured native input-to-receipt durations are approximately 419–425 ms.
They include deliberate injected-input sleeps and receipt polling, and cannot
be interpreted as pure transport latency. The current binary/dependency delta,
Windows packaging/SIMD decoder evidence, and exact-source CI still require
their final receipts at this review checkpoint.

The physical two-computer LAN regression in brief section 23 remains a separate
human gate. Multiple processes on one Linux computer, a Windows compilation,
or an automated CI run cannot certify that physical acceptance. No CRDT,
offline merge, authentication, WAN transport, presence service, or recurring
idle network maintenance was introduced in the inspected concurrency paths.

## Integrator evidence appended after the independent source review

This section is supplied by the integrating agent, not a second test execution
or retrospective certification by the independent reviewer. The final local
gate passes 350 Rust tests, 28 Python tests, Clippy/documentation and dependency
checks; explicit NVIDIA Vulkan GPU suites pass 11 tests.

Tracked receipts now contain [16 native concurrency checks](../../benchmarks/phase2a5/native-three.json),
[13 desktop workflow checks](../../benchmarks/phase2a5/desktop-ui.json),
[paired local idle measurements](../../benchmarks/phase2a5/local-idle.json), and
[full lease-capacity/expiry measurements](../../benchmarks/phase2a5/lease-capacity.json).
All recorded checks pass. Native source binaries are identified by their SHA-256;
the earlier failed automation attempts remain separate.

The GNU Windows package contains SIMD SSE2/AVX2 objects and no external MinGW
runtime dependency. CLI/JPEG execution evidence is separately tracked in
[windows-execution.json](../../benchmarks/phase2a5/windows-execution.json).
The routine chosen during automatic SIMD dispatch is not instrumented.
Physical Windows desktop and two-computer artist acceptance remain pending.
