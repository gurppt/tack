# Mission 1A — document kernel and action foundations

Date: 2026-10-03. Document/action foundation completed, tested and independently reviewed.

## Pre-implementation plan

1. Keep the four crates, Camera/WorldRect, Lod/ByteCache, measured loader and
   GPU renderer. Git starts at `00e893a` plus staged, completed Mission 0.7;
   retain that work and the unrelated untracked `gfx/Untitled.png`.
2. Add zero-dependency core modules for typed IDs, validated durable geometry,
   source/asset/image-object metadata, private document state and explicit
   reversible commands. No source reads, hashing, decoded pixels or schema codec.
3. Let a DocumentEditor own the document and a capped inverse-command history;
   expose immutable document queries, never mutable table references. Bound
   document metadata and linked-source descriptors explicitly.
4. Put canonical semantic actions, tool interaction and physical bindings in
   tack-app. Characterize existing navigation before replacing its hard-coded
   button tests with bindings. Preserve late Alt changes, modifier combinations,
   wheel scaling, focus-loss cleanup and scripted trace isolation. Add no final
   keyboard preset. Verify retained navigation against official PureRef docs.
5. Keep benchmark Board/asset keys explicitly benchmark-specific; their u32 IDs
   and trusted manifest hashes are not product identities. DocumentQuery yields
   immutable ordered image render metadata without I/O, caching or locks; test
   consumption at the renderer crate boundary. Actual transformed/cropped GPU
   drawing is a later vertical slice, not a renderer rewrite here.
6. Test identity separation, references, validation, every command and inverse,
   sequence restoration/divergence/bounded history, query ordering/culling,
   bindings/holds/modifiers/focus loss and tool restoration. No new dependency.
   Run all project gates, GPU readback and controlled native Phase 0 traces.
7. Document compatibility-sensitive ID/geometry/default semantics and Phase 1B
   requirements without selecting a container. Independent review, fix issues,
   final report, STOP before Phase 1B. Metadata query remains linear initially;
   no new frame I/O, threads, worker queues or retained media payloads.


## Repository starting state

`main` and `origin/main` started at `00e893a` (Mission 0.6). Completed Mission
0.7 changes were already staged but uncommitted; its frozen final executable is
our comparison baseline. Earlier relevant history is `691d084` (renderer) and
`a4d8f6d` (bounded streaming). The initial four-crate direction remains intact:
core geometry/cache policy; assets manifest/worker/cache; GPU rendering; desktop
composition, input and reproducible benchmark telemetry. Core had no dependencies
and no durable document/command/history model. The manifest's object ID was also
its u32 display-cache asset key, alongside a canonicalized path and trusted hash.
That is useful benchmark input, not production identity or file format.

The linear static-board cull, Camera, Lod, ByteCache and all native decoder/cache
ownership remain. No proven spatial index existed to discard or duplicate.
The pending 0.7 changes and unrelated untracked image were preserved.

## Document kernel and identities

Core now owns a `Document` with private ordered object, asset and source tables.
Stable nonzero 128-bit DocumentId/ObjectId/AssetId/SourceId newtypes cannot be
mixed; compile-fail tests prove those boundaries. Callers supply identities;
there is no synchronous generator/hash, and duplicate IDs are rejected. IDs
must persist through save/reopen, not be reused for different entities or derived
from slots, paths or GPU resources. Uniqueness generation remains an explicit
Phase 1B composition concern. No GroupId/FrameId or speculative subsystem is added.

`DocumentObject` owns ID/Transform and an explicit ObjectKind enum containing
only Image. Its ImageObject references ImageAsset; the asset references Source
and retains image pixel dimensions as metadata. Multiple objects can share an
asset. Source is Embedded (future payload lookup by SourceId) or Linked (bounded
opaque PathBuf). Source construction neither opens nor canonicalizes nor hashes
files. No embedding, linked reads, hot reload or source revision code is added.
Removing a referenced asset/source is rejected, never cascades silently. Decoded
pixels, cache entries and renderer handles are absent from commands/documents.

Transform uses center, positive world size, clockwise radians in y-down world
space and flip flags, with validated cached rotated bounds. Crop is a positive
normalized UV rectangle selecting pixels into the same world box, not an implicit
box resize. Opacity stays in [0,1]. Nonfinite values, invalid size/crop/opacity and
bounds beyond existing world limits are rejected. Private fields prevent durable
invalid values from being manufactured through normal public APIs; the older
public WorldRect remains a copied query/camera value, not durable mutable geometry.

Default filtering delegates to local preferences; Smooth/Nearest are durable
object overrides. Their eventual GPU sampling implementation is not part of this
mission. No product settings or UI is claimed.

## Commands and history

Eleven explicit Command variants add/remove sources, assets and objects; set
transform/crop/opacity/filtering; and reorder objects. They contain semantic
metadata only. Application is deterministic on the same prior document, checks
all preconditions before mutation, returns structured errors and creates an
inverse for actual changes. AddObject index permits insertion through len;
SetZOrder index is the final back-to-front slot in 0..len. Ordering positions
are not identities. RemoveObject inverses retain complete metadata and exact
prior position. There is no filesystem, renderer or collaboration dependency.
Indices are suitable for ordered local operations, not a concurrent-edit/CRDT
claim; any future replication must define causal/revision semantics explicitly.

DocumentEditor exclusively owns document and undo/redo; only immutable document
access escapes it. New successful edits clear redo; failed commands and no-ops
preserve it. Undo applies an inverse and retains the inverse of that application
for redo. Runtime never snapshots the entire document. History capacity is
supplied explicitly by its owner (zero disables recording); oldest undo entries
are evicted, and undo+redo retained entries share that cap. Each entry contains
at most one fixed record or ≤4096 native encoded bytes of linked path. Container
capacity/allocator overhead is additional bounded metadata, not image residency.
History may be cleared explicitly or discarded by consuming the editor; it is
not serialized. Direct Document::apply supports command-only construction or
non-history ownership, while an active editor exposes no mutable escape hatch.

Document metadata counts are separately bounded by caller-selected limits
(default 100k objects/assets/sources); linked paths reject empty, NUL-containing
and over-4096-byte values. These are metadata admission policies, not proof of
100k rendering, file-reader allocation safety or supported decode dimensions.
Source size is not working-set size.

## Actions, bindings and state scopes

App's library is the testable action/input foundation. Action and its enumerable
catalog are the canonical source for SelectTool, TemporaryTool, Undo/Redo,
PanView/ZoomView; labels live with that catalog. Future menus, palettes and
bindings can use those same values. A document action dispatch test demonstrates
that a binding and a direct menu-like Undo invocation reach the same editor.
No registry of separate menu/shortcut/tool commands was introduced.

winit events normalize to physical keyboard codes (including unidentified native
codes), mouse/extra pointer buttons, wheel axes and modifiers, then user Keymap
bindings resolve to ActionEvents. Exact/Contains/Any modifier predicates are
explicit, overlapping bindings return a conflict referring to the prior binding.
The map supports press, release, holds, wheel pulses, multiple controls per action
and fully unassigned actions. It holds at most 256 bindings. InputState retains
at most 32 assigned held controls and emits directly via callback, not an event
queue. Unassigned presses retain no state; newly assigned controls start only
with their next press. Excess assigned holds return a structured admission error
without damaging retained gestures. The live view adapter drops invalid/excess
input and rejected camera geometry without closing the window.

Temporary tools capture their action and opaque HoldToken at initial press;
repeat/duplicate press does not stack. Last pressed hold wins, out-of-order release
restores the remaining hold/base, and release/focus loss works after modifiers or
keymap changes. Choosing a base tool during a hold changes the later restored
base deliberately. R/Space tests prove rotation/pan holds, but are synthetic
bindings, not shipped defaults or an implemented rotation gesture. PanView holds
instead inspect current modifiers so late Alt press/release retains prototype
behavior. Held temporary-action queries use captured gestures, not changed maps.

Ownership remains explicit: Document/Editor durable metadata+history; Interaction
transient tools; Camera local view; Keymap local preferences; loader/GPU disposable
renderer/cache state. No combined production global state or new thread/runtime
is added. The existing Session is explicitly still a benchmark composition root.

## Renderer boundary and narrow refactors

DocumentQuery returns Copy ImageRenderData and ordered objects_in_view without
source descriptors, IO, locks, decode, hashing or worker waits. Single ID lookup
is O(log n); visibility is an allocation-free O(n log n) ordered scan through
BTreeMap IDs with conservative rotated AABB culling. Source paths can be missing
and dimensions enormous without query reads or source-sized allocations. A
renderer-crate consumer test proves the source-free contract and stable ordering.
Actual GPU use of transformed/cropped product objects is deferred to the image
vertical slice; this is a query boundary, not a claim of a rendered product board.
No new production spatial index or renderer fork was introduced.

Before refactoring native navigation, its middle/Alt-left/focus/wheel contract was
extracted mechanically and its two characterization tests passed. Those tests
survive binding migration, strengthened with native event normalization, scripted
camera isolation, malformed input and 1000 unassigned-control cases. The old
middle_down/left_down/alt interpretation in window was removed, replaced by
NavigationInput → Keymap → Action, and Camera methods retained. The manifest's
order/culling/identity/path-containment characterization passed before renaming
Board/ImageObject to BenchmarkBoard/BenchmarkImage across all consumers. No old
aliases or competing production document copies remain. AssetKey's u32 prototype
identity is documented explicitly; no measured scheduler/cache/LOD implementation
changed beyond these type names.

Retained navigation was rechecked 2026-10-03 against
[PureRef's official navigation handbook](https://www.pureref.com/handbook/navigation/)
and [2.1 default shortcuts](https://www.pureref.com/handbook/shortcuts/all-shortcuts/).
Middle drag, Alt+left and wheel remain; Z drag/window gestures are still deferred,
and no final keyboard preset is chosen. The existing
[mapping table](research/pureref_shortcuts.md) records those limits.

## Tests and quality checks actually executed

- Pre-refactor `cargo test -p tack-app navigation_input --locked --offline`:
  2 characterization tests passed before binding migration.
- Pre-rename `cargo test -p tack-assets --test benchmark_board --locked --offline`:
  manifest/culling characterization passed.
- `cargo test -p tack-core --locked --offline`, app targeted tests and workspace
  all-feature tests passed during implementation. New core tests cover each
  command class, exact creation/removal/ordering restoration, shared identities,
  invalid command atomicity, no-ops/divergence, metadata/history admission,
  validation, queries and a deterministic 400-command random sequence.
- `PATH=/tmp/tack-tools/bin:$PATH bash tools/check.sh`: final fmt, workspace
  all-target/all-feature locked check, Clippy with -D warnings, workspace
  all-feature tests (42 Rust + 3 compile-fail doctests), docs, 6 Python tests and
  cargo-deny advisories/bans/licenses/sources all passed. The configured advisory
  check is cargo-deny; no separate unexecuted cargo-audit result is claimed.
- `cargo test -p tack-render --test gpu_smoke --locked -- --ignored --nocapture`:
  one explicit readback test passed on NVIDIA RTX 2060 / Vulkan, discrete GPU.
- `cargo build --release --locked -p tack-app` and assets overview_prepare example
  build passed. `cargo tree -p tack-core --locked --offline` confirms zero
  dependencies. No dependency or lockfile change. `git diff --check` passed.

Quality log SHA256:
`c55e9d9b13dc48abba6e5223f193119700e8a3defd31077772c8f05cc4c07e19`.
GPU log SHA256:
`56fbcf314d88b0e40a725b69645b58bb54a9c9a71ad148ccb9d6ade2cfe30347`.
Full logs are retained in ignored raw results, not presented as independently
executed logs. Independent verifier commands/results are recorded below.

## Native performance/regression evidence

Existing harness executed 22 native 12-second traces: nine baseline + nine final,
then paired normal-pan/pressure repeats. Raw binaries/source ZIPs/manifests,
frame/worker/episode reports and environment remain under ignored
`benchmark-results/phase1a-{baseline,final}[-repeat]`; compact full summaries,
commands, provenance and quality hashes are in
[regression evidence](../benchmarks/phase1a-regression.json).

Commands actually executed:

```bash
python3 tools/run_benchmarks.py --binary benchmark-results/mission0_7-final-control/tack-app --source-snapshot benchmark-results/mission0_7-final-control/source-snapshot.zip --output benchmark-results/phase1a-baseline --scenarios cold warm pan-normal pan-fast zoom-traverse scan board-tour pressure pan --seconds 12
python3 tools/run_benchmarks.py --output benchmark-results/phase1a-final --scenarios cold warm pan-normal pan-fast zoom-traverse scan board-tour pressure pan --seconds 12
python3 tools/run_benchmarks.py --binary benchmark-results/phase1a-baseline/tack-app --source-snapshot benchmark-results/phase1a-baseline/source-snapshot.zip --output benchmark-results/phase1a-baseline-repeat --scenarios pan-normal pressure --seconds 12
python3 tools/run_benchmarks.py --binary benchmark-results/phase1a-final/tack-app --source-snapshot benchmark-results/phase1a-final/source-snapshot.zip --output benchmark-results/phase1a-final-repeat --scenarios pan-normal pressure --seconds 12
```

Baseline executable SHA256 is Mission 0.7's
`fd92de406510b5c32a5e39b8f2ce1f70709cee0dfcd4b4d434c2da4d5f7723c8`;
final is `0cc62b94086f3b6c7e32f249acc8634beb4570c823858c04e166d94c81d06ddc`.
Final ZIP crate files match the working tree byte-for-byte. Manifest SHA256 is
`8bd27d9631560213365df47b842e37720421703608b13e79ee54f8524ad44af6`.
Ryzen 2700X (boost disabled), RTX2060/Vulkan/NVIDIA580.173.02, native X11/Linux,
Rust1.95, two workers, prefetch/preparation off. Corpus is 1000 IDs over 32
hardlinked synthetic JPEG contents, not representative artist media; kernel
page cache is not flushed. Each trace has fresh CPU/GPU/derived cache, except
cold/warm deliberately sharing only their own suite's SSD cache.

First paired suite:

| Trace | CPU p99 baseline / final ms | Recognizable baseline / final % | Final GPU pass p99 ms | Final RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| cold | 0.482 / 0.511 | 98.77 / 98.91 | 1.581 | 492.2 |
| warm | 0.456 / 0.450 | 99.86 / 99.86 | 1.502 | 385.0 |
| pan-normal | 6.175 / 5.864 | 83.66 / 80.79 | 0.694 | 515.8 |
| pan-fast | 0.678 / 0.935 | 76.36 / 77.23 | 0.180 | 498.6 |
| zoom-traverse | 0.956 / 0.762 | 41.90 / 41.44 | 1.542 | 489.2 |
| scan | 1.793 / 1.756 | 36.74 / 36.33 | 0.559 | 504.6 |
| board-tour | 1.358 / 1.332 | 5.81 / 5.36 | 0.252 | 353.5 |
| pressure | 2.241 / 2.912 | 95.50 / 95.49 | 0.208 | 448.6 |
| pan (distant jumps) | 0.346 / 0.264 | 0.00 / 0.00 | 0.057 | 342.9 |

Repeat normal pan gives CPU p99 5.867 / 5.855 ms, recognizable 81.78 / 81.84%,
requested LOD 25.98 / 24.07%; first-pair requested was 27.14 / 23.55%. Thus useful
coverage variation is demonstrated, not an established gain/loss; requested
coverage is lower in both pairs and remains recorded. These are two uncontrolled
scheduler trials per selected trace, not a statistical non-regression proof.

Pressure CPU p99 increases in both pairs: 2.241→2.912 and 2.344→2.893 ms
(+0.55–0.67 ms). Do not dismiss that as an isolated outlier or claim identical
performance. Raw frame breakdown places these p99 frames in the unchanged GPU
upload code: upload CPU p99 is 2.058→2.711 and 2.098→2.686 ms. Every pressure run
uploads exactly 18 textures / 80,510,976 bytes. That identifies the measured stage,
not the cause of the difference; no profiler-derived causal claim is made. CPU
p95 upload is zero in all four traces. Pressure recognizable coverage is stable
at 95.49–95.50%; requested coverage is 72.01→71.37% then 71.97→71.65%.
This small absolute CPU tail increase is retained as a baseline risk for the next
vertical slice, while measured renderer responsiveness/resource budgets remain
within the Phase 0 envelope. No tradeoff relaxing those budgets is proposed.

All 15,628 recorded frames pass cache/pending/upload/in-flight bounds. Assets
fully drain; job/coverage profiles have zero drops. Final CPU worst is 8.344 ms,
GPU pass p99 max 1.581 ms / worst 1.778 ms, and GPU completion 59.08–59.33/s
(matches the scripted 60-FPS-class baseline cadence). Final RSS across repeats
peaks at 563.5 MiB versus baseline 545.8 MiB; payload budgets, not RSS/driver
memory, are the hard admission caps. No source-sized domain buffers were added.

Native acquisition/presentation must remain visible: final acquisition max
19.931 ms, present max 23.939 ms, callback max 31.595 ms, versus baseline present
24.300 ms / callback24.507 ms. Frame CPU/GPU timestamps exclude these platform
costs; this is not proof the window never jitters or an absence of all regression.
No new input/core code performs I/O or worker waits; renderer, shaders and asset
scheduling/decoding remain unchanged. New product DocumentQuery/undo performance
at large scale is not measured by this prototype and needs its own later gate.

## Compatibility design

[The compatibility contract](design/tack_document_compatibility.md) specifies
schema/app/generator/protocol version separation, stable ID encoding requirements,
known optional-field defaults, explicit migrations and older-reader degradation.
Unknown newer records/properties must be preserved safely in a future storage
envelope or force read-only/refusal; an older writer cannot silently overwrite
unsupported user work. Future export to an older version writes a separate copy
with explicit deterministic baking/rasterization/degradation disclosure.
Reader byte/count/offset/decompression budgets precede domain construction, and
missing/corrupt derived previews cannot damage authoritative state. Cross-platform
path representation, corruption/interruption and recovery requirements are explicit.
No SQLite/ZIP choice, serializer, save/import path or persistence scaffold is added.

## Remaining risks and stop boundary

Independent agent `mission0_verifier` reviewed responsibility boundaries, durable
versus view state, identity/reference integrity, commands/inverses/history,
input/action separation, hidden work, growth, clones, error paths and cleanup.
It found the native saturation error propagating to window exit. The fix ignores
unassigned controls and drops malformed/excess input at the live adapter;
regression tests prove subsequent navigation/focus cleanup stays usable.
No material defect remains.

Independent commands actually executed with Rust 1.95 were
`cargo test -p tack-core -p tack-app --locked --offline`,
`cargo test -p tack-render --test document_query -p tack-assets --test benchmark_board --locked --offline`
and a final `cargo test -p tack-app --locked --offline`: 24 distinct Rust tests
plus 3 compile-fail doctests passed. It also recomputed all 22 traces / 15,628
frames, checked binary/ZIP/harness/manifest/raw/compact hashes and commands,
actual cache inventories/quotas, coverage/censure/stages/concurrency/bounds,
quality logs and report tables. Independent outputs are session tool results,
not a claimed independent log file. The
[verification record](../benchmarks/phase1a-verification.json) preserves that scope.

Persistence format, portable path encoding, unique-ID generation and source
revision/change commands remain to implement/test in Phase 1B. New product query
integration and large-board spatial indexing are not benchmarked; the renderer
still consumes the proven manifest board. Production input device/platform/DPI
coverage and default keymap editor/preset are unfinished. No actual rotate-view
behavior, image manipulation/selection UI or sampling shader change is claimed.
Mission 0.7's background fairness, warm high-LOD contention, progressive native
scratch and synthetic-corpus/OS-cache limits still apply; Phase 1A does not solve
or disguise them. Windows native startup/CI was not run locally.

No `.pur` file or private media was opened/copied; no product import, persistence,
media, collaboration, server, annotation/grid or other backlog scope was started.
The current change has one explicit domain path plus separately named benchmark
fixtures, not old/new document implementations. No speculative entity/plugin,
network or storage framework was added. After independent review and report,
STOP before Phase 1B; the human reviews this foundation before the next mission.

## Recommendation

The resident kernel, deterministic reversible command/history ownership and
semantic binding/action foundation are coherent enough to author
**Phase 1B — Minimal .tack persistence + image vertical slice**. Required quality,
GPU and resource gates pass; the independently audited benchmark evidence keeps
the pressure tail increase, platform outliers and incomplete product integration
explicit. No persistence/container or roadmap implementation was needed to make
these boundaries concrete. User-authorized commits/push save this completed work.

**STOP after Mission 1A.** Wait for human review; this verdict does not authorize
Phase 1B implementation automatically.

**A — PASS**
