# Phase 1K independent LOD review

2026-10-07. Scope: implementation `e85d0e4b84a72968a3d3b0500c2c7355beb2a5b1`
covering LOD selection, residency protection,
opt-in diagnostics, flat backgrounds and Preferences/Keymap implementation.
This review includes follow-up fixes, final receipts and exact implementation
CI after the initial checkpoint. It does not replace owner acceptance.

**Independent technical verdict: PASS for this implementation and its measured
scope.** No unresolved correctness issue remains in the reviewed code or
evidence. Owner acceptance of the original problematic board remains pending;
stop for human review before starting the next phase.

## Findings and evidence

The old selector excluded every resident tier above the desired tier. With
Thumbnail128 and Detail2048 resident and Medium delayed, a monotone dezoom
through 513, 512, 129, 128 projected pixels displayed Detail, Thumbnail,
Thumbnail, Thumbnail: adequate, inadequate, inadequate, adequate. The retained
`benchmark-results/phase1k-prefs/selection-before.log` reproduces the failed
assertion at 512 pixels. The new `displayed_lod` bridges the gap with Detail
until Medium is available, then chooses the smallest adequate resident.

The old accounting also preceded later uploads. Three Detail textures and four
Medium textures can occupy 52 MiB in the 64 MiB refinement partition. With LRU
order M1, M2, M3, D1, D2, D3, M4, uploading D4 evicts D1 after it was counted as
resolved. The real-GPU regression in `lod_convergence.rs` recreates this eviction
and confirms protection of current exact-demand residents. Its second GPU test
covers lower-tier insertion and repeated threshold crossings. Both passed in
`benchmark-results/phase1k-prefs/lod-gpu.log`; the reviewer read the log and test
implementation, and did not independently run GPU work during another owner run.

Post-upload selection now recomputes coverage, adequate quality and outstanding
exact demand together. Protection touches exact current demands, not optional
larger fallbacks. The admission plan reserves no more than the partition budget
for unique refinement source keys; actual RGBA costs cannot exceed its square
tier estimates. Thus obsolete entries precede protected demands in the LRU,
and the admitted final set fits without increasing caps. Shared-source aliases
do not add refinement residency. A separate read-only model checked 120 initial
LRU permutations with this property.

No unresolved production LOD correctness issue was found in this diff. Request
caps, source/revision identities, stale-result rejection and worker count remain
unchanged. Diagnostics add no timer, worker or ordinary heap allocation. Flat
background conversion removes the second color and interpolation; the matching
uniform shrinks from 80 to 64 bytes. No remaining gradient shader/state was
found in the implementation paths searched.

## Follow-up corrections verified

- The native zoom harness now uses `Session.started` and checks every frame in
  the observation interval with a conservative 100 ms margin. The misleading
  last-frame-only/first-second omission is removed. Its retained 800x600 receipt
  records final adequate quality for the one visible image and zero idle I/O;
  it is a diagnostic run, not an instrumentation-equivalent CPU comparison.
- Diagnostics report acceptance after successful CPU cache insertion, and
  retain at most 16 completed publication records while explicitly enabled.
- `delayed_zoom_result_cannot_suppress_the_new_tier_or_return_to_detail` uses a
  real ProductAssets worker, withholds publication across a demand change,
  confirms stale Detail rejection, and confirms subsequent Medium and Detail
  admission. It passes in `check-final.log`. This supplements the GPU inventory
  tests; it is not a Windows runtime observation.
- Current README and feature inventory now describe flat backgrounds.
  Historical Phase 1J reports retain their historical gradient descriptions.

## Preferences and Keymap review

The selected action is changed transactionally: conflicts preserve the active
map and profile. Capture uses the existing logical-key normalization. Escape
and the explicit Cancel button leave capture without applying a binding.
Global/category resets require a separate confirmation; Cancel preserves the
map. Handle Size remains bounded to 3..21 and Hit Radius to 5..32, with disabled
endpoint buttons and no wraparound. UI Scale and Background use direct choices.

Import validates through `preferences::read` before replacing mappings; export
uses the existing canonical profile writer. The panel adds no serialization
path, permanent widgets, thread, timer or background index. Mouse row selection
does not itself enter capture. Search scans action names, categories and active
shortcut labels using the existing catalog. Menu shortcut formatting continues
to use the active keymap.

The initial readability finding is addressed by compact canonical mouse/wheel
labels and selected-action footer details including trigger semantics. Reviewed
800x600 at 2x screenshots show readable Pointer bindings and an immediate Undo
conflict stating the requested Ctrl+V, existing Paste action, refusal and Cancel.
Essential Change/Unassign/Reset/Import/Export controls fit. Long unselected row
labels can still truncate; selecting the row exposes its details. Reverse Tab
now reaches the last control when invoked at the first.

The reviewer read the final `settings-counter-final/receipt.json`: all six
theme/scale cases and 62 checks pass, including US/FR logical remapping and
canonical in-panel export/import. Its 18 Preferences, Keymap and closed-panel
intervals have zero redraw, zero I/O, zero RSS delta and no new threads. These
are owner-run receipts, not new GPU runs by the reviewer. `check-final.log`
records 176 passing Rust tests, 10 intentionally ignored GPU tests, 13 Python
tests, and successful advisories/bans/licenses/sources checks. Separate GPU
receipts remain necessary for the ignored tests.

## Final code and measurement checkpoint

The final diagnostic counters count successful job dispatches and all returned
RGBA bytes, including discarded outcomes, rather than hiding stale work. A real
worker test now also rejects delayed revision-1 pixels and successfully admits
revision 2. Both delayed-result tests pass in the final local gate.

The counter-only baseline patch was inspected: it adds dispatch/returned-byte
counters and bounded wheel timing to the previous implementation without
changing LOD selection or scheduling. The comparable `counts-*` receipts use
the same harness and board hashes, with diagnostics disabled on both binaries:

| Window | Fixture | Requests before/after | Decoded bytes before/after |
| --- | --- | --- | --- |
| 800x600 | 5k | 32 / 32 | 29,573,120 / 29,573,120 |
| 800x600 | sparse | 21 / 21 | 16,220,160 / 16,220,160 |
| 1024x768 | 5k | 52 / 52 | 34,078,720 / 34,078,720 |
| 1024x768 | sparse | 29 / 29 | 19,005,440 / 19,005,440 |

Payload peaks and pending/queue bounds also match in these four pairs. Median
supply time is measured in tens of microseconds: 800x600 sparse increases from
11.22 to 17.82 microseconds, while 1024x768 5k decreases from 38.59 to 29.20.
Earlier AB/BA runs show variable direction; these short, warmed, isolated-X11
samples do not establish a general speedup. No material regression is evident
within this measured scope.

The final 1024x768 zoom pair matches 21 requests and 61,341,696 decoded bytes.
The 800x600 pair has a real increase: 17 to 18 requests and 25,690,112 to
42,467,328 returned bytes. Raw frames locate the additional 16 MiB Detail result
during rapid camera changes: dispatch near 7489.5 ms, completion near 7693.7 ms,
and discarded count increases from 2 to 3. Uploads remain 15 / 23,592,960 bytes
and payload peaks match. The separately recorded diagnostic current run has
17 requests, matching the baseline. This is consistent with asynchronous
admission/completion timing variance, not incorrect publication or retained
growth. It is still additional work and must not be described as a necessary
correctness refinement or hidden by averaging. The reverse-order 800x600 pair
now records current 17 requests / 25,690,112 bytes and baseline 18 requests /
42,467,328 bytes, reversing which binary incurs the discarded Detail job.
Both retain identical uploads and payload peaks. Across the two orders both
binaries therefore span 17..18 requests, without evidence of a systematic
increase from the fix.

The inspected binary audit records 17,954,152 to 18,028,136 stripped bytes:
+73,984 bytes / +0.4121%. All 276 dependency packages remain the same; tracing
was already transitive. Stripped and unstripped current `.text` hashes match.
The final current stripped SHA256 is
`20c8f368f51898e2a676a9e9be3192b0e2f0144c8bc1a46d0919ba7bee9ff45f`;
the counter-only comparison baseline is
`36a51481f3bd1d0406490d9e72bf35fb50a7e2c0e8f27690b398fac72e8c638f`.
The 92-action filter receipt records approximately 3.4 microseconds for an
empty search and 25..27.5 microseconds for tested nonempty searches.

## Final verdict and acceptance boundary

No unresolved production correctness issue was found in the reviewed changes.
The code and available comparable measurements are acceptable with the explicit
asynchronous-work variance above. Diagnostics remain excluded from comparable
CPU runs because they rebuild and sort a plan per frame.

The final native log records all 12 renderer/LOD suite cases passing, including
the 10 explicit GPU opt-ins and the three-fill readback test. The additional
heavy, menu and two-board idle harness was reviewed; its mistaken nested quality
field was reported and corrected before execution. `idle-final/receipt.json`
records 14 two-second observations, seven per binary, covering empty boards,
open/closed menus, the 5k board and both simultaneous windows. Every observation
has zero redraw, zero I/O, zero RSS delta and zero thread-count change. Both
binaries have eight threads under that test environment; process aggregate CPU
accounting is 0..2 ticks, so this is not a claim of zero driver CPU activity.
Final heavy-view convergence is explicitly checked. The final zoom harness
also checks adequate quality at every settled scripted stage, not just at exit.

The reviewer inspected these receipts and harness code without launching native
windows, compiling or overlapping GPU ownership. Local technical verification
has no unresolved correctness finding. The exact implementation SHA passes
[Quality run 37594215458](https://github.com/gurppt/tack/actions/runs/37594215458):
Linux, Windows and dependency jobs all completed successfully, as recorded in
`ci-implementation.json`. This establishes the Windows compile/test gate, not
Windows runtime behavior. Technical verification is PASS; owner testing of the
original board remains pending. No collaboration, server or media phase is
authorized by this review.
