# Mission 1B report

2026-10-03. Starting `main`/origin commit `c03d24b` (Phase 1A A—PASS).
The requested local persistence/image vertical slice is implemented. No Phase 1C
interaction, future backlog UI or rejected storage experiment enters runtime.
Required normative briefs, reports and context were read; no conflict found.
The unrelated user-owned `gfx/Untitled.png` remains untouched/untracked.

## Starting state and storage decision

The existing private `Document`, typed 128-bit identities, object→asset→source
relationships, validated geometry/crop/opacity/filtering, reversible commands,
bounded history, semantic actions/keymap and immutable `DocumentQuery` remain
the source of truth. Renderer benchmark identities remain separate.

[The pre-implementation decision](design/tack_storage_decision.md) compared SQLite
metadata+incremental chunked BLOB storage with an indexed snapshot. A small warm,
no-fsync 1k dummy-payload spike measured write/open 219.3/2.56 ms versus 40.7/0.109 ms.
The metadata workloads were not equivalent; this is feasibility evidence, not
a production superiority claim. SQLite large BLOB limits would require chunking
and native deployment. The selected explicit indexed snapshot provides bounded
document-first reads and independent original/preview ranges without extraction.
Its cost is whole-file replacement and space for a complete temporary generation.
The spike stays ignored evidence; no SQLite/ZIP/runtime framework was added.

`tack-storage` promotes already locked getrandom 0.3.4 and crc32fast 1.5.2 as direct
dependencies, with no new transitive package. OS entropy supplies nonzero stable
128-bit IDs independent of paths, slots or network. Finite sample tests check
semantics/roundtrip, not mathematical collision impossibility. Core remains
zero-dependency. Licenses/advisories are checked by the configured cargo-deny gate.

## Format and source semantics

[Format v1](design/tack_file_format_v1.md) documents the wire layout: 80-byte header,
separate container/schema versions, bounded authoritative metadata, independently
checksummed derived directory, uncompressed addressed payloads. Encoding is
explicit little-endian with versioned records and binary64 geometry, not Rust
struct serialization. IDs, order, image state/pixel metadata, descriptors,
revision/fingerprint and original/overview references round-trip exactly.

Unknown/newer authoritative schema, capabilities, records, kinds, fields or enums
refuse editable open. There is no opaque preservation/migration yet; refusal
prevents old-reader save from losing newer work. Unknown derived formats are
reproducible and disposable. CLI `create` refuses existing outputs/symlinks before
preparation and again before save; it cannot silently replace newer user work.

Embedded originals live in the single file and stream on demand through positional
reads of its opened generation. Linked paths retain lossless Unix native bytes
or Windows UTF-16LE plus platform/absolute metadata; foreign/unsupported paths
are preserved unresolved. Relative paths use the board parent. CLI import creates
canonical absolute links; cross-parent repair refuses relative descriptors rather
than silently changing their binding. Save APIs leave base/relink decisions to
composition. A reversible `SetSource` uses a fresh increasing revision; editor
high-water survives undo/divergence. Cheap size/mtime detects observed changes,
not same-size/same-time content substitution. There is no watcher or full hash on
open. Missing/changed/foreign/unavailable state is explicit for requested sources;
valid stored previews may display as last-known imagery.

Overview role, encoding, generator, size and source revision are directory data,
not image-object semantics. The measured native 128 JPEG tier is reused. Embedded
repair uses a ranged streaming adapter into the existing scaled JPEG/thumbnail
operations (generator 2), without materializing a whole original. PNG is similarly
bounded. Damaged/missing/stale previews repair independently if source is usable.
The product supply wrapper is necessary because the benchmark loader uses u32
manifest/file keys; it does not introduce another document model or redesign LODs.

Bounds before allocation: authority 64 MiB,100k per table/directory, source records
8192 bytes, paths 4096 bytes, overview 1 MiB encoded/512×512, file≤8 TiB, checked ranges
and sorted O(n log n) overlap checks. Workers cap encoded parsing at 64 MiB plus
sniff/sentinel, source dimensions 6000×4500, PNG allocation hint 192 MiB and overview
4 MiB. Codec scratch/RSS is separate. Two workers admit ≤16 jobs/results; CPU RGBA
64 MiB and worker-only repair SSD 512 MiB. Stable asset/revision/generator cache files
are reused after CPU eviction and quota maintenance reuses existing DiskCache.

## Save/recovery and dirty state

Save is a worker/CLI operation. It validates metadata/bindings, streams 128 KiB
chunks into an exclusive sibling temp, verifies stored-payload CRC, checks input
length/size/mtime, writes directories/header, syncs the file, then replaces the
target without first deleting it. Unix parent sync follows publication.
`PublishedButNotDirectorySynced` explicitly means a new valid target is already
published. Earlier failures leave the previous generation unchanged. Owned temps
are removed on recoverable failures; process kills can leave orphan temps, which
are never auto-promoted or broadly deleted. New Unix files/temps use 0600, private
work dirs 700 and repair PNG 600; replacement preserves target permission bits.
Symlink/nonregular save targets are refused.

Four real SIGKILL stages (temporary creation, payload copy, metadata write, file
sync) and injected failures preserve the previous target byte-for-byte. This is
Linux process consistency, not simulated power loss or arbitrary filesystem
atomicity. Metadata open does not scan original CRC. Partial decoding also does
not prove full original integrity; an equal-length valid substituted embedded
image can decode before save-copy rejects its CRC. CRC32 is accidental integrity,
never authentication. No encryption/ACL preservation claim is made.

Effective edit/undo/redo sets conservative dirty state; successful exact-current
save clears it. A return to saved content via undo may remain conservatively dirty.
History, camera, caches, workers and GPU state are not persisted.

## Human reproduction and renderer integration

Prepare the existing pinned native decoder once, build, then use real JPEG/PNG:

```bash
python3 tools/prepare_turbojpeg.py
cargo build --release --locked -p tack-app
target/release/tack-app create /tmp/my-reference.tack --embedded /absolute/path/image.jpg
target/release/tack-app inspect /tmp/my-reference.tack
target/release/tack-app open /tmp/my-reference.tack
```

Close the window, remove/move the external original if desired, and reopen the
embedded file. Copying the file needs no external source/cache/account. `--linked`
keeps an external absolute descriptor. Middle drag or Alt+left drag pans, wheel
zooms. `repair INPUT.tack OUTPUT.tack REPORT.json` repairs previews while retaining
authority; input compatibility is checked first. Paths support native non-Unicode
arguments. Repair may replace its validated canonical input or write a new file;
a distinct existing destination is refused before preparation and rechecked before
publication. Diagnostic reports require a new file and use atomic exclusive
creation, including protection from hardlink/symlink aliases. Existing Linux libXi workaround/build instructions remain in README.

Product `DocumentQuery` now reaches the proven common GPU path using typed
AssetId+revision texture keys, without source/storage concepts or GPU file I/O.
Quads honor center, size, rotation, flips, normalized crop, opacity and sampling.
Smooth/Default use linear and Nearest uses nearest sampler bindings sharing one
texture. GPU readback checks those behaviors and revision-separated textures.
Residence/coverage is recomputed after all uploads so later eviction cannot create
false recognizable coverage. The native window loads metadata asynchronously,
sleeps when idle/occluded, and drains existing asset/GPU work outside navigation
before report export. No selection, gestures, higher-LOD product refinement,
polished import UI or autosave is implemented.

## Measurement method and provenance

Final runtime binary, source ZIP, harness hash, validated 1k source SHA inventory,
native static-library/build identity, environment, commands and raw-report hashes
are bound in [product evidence](../benchmarks/phase1b-product.json) and
[regression evidence](../benchmarks/phase1b-regression.json). Ignored raw files,
binaries and logs remain in `benchmark-results/phase1b-product-export-safe`,
`phase1b-regression-baseline`, `phase1b-regression-export-safe` and named repeat
directories. Runtime src/Cargo files and tests match the frozen final tree;
subsequent changes only update documentation/evidence.

Ryzen 2700X, RTX 2060/Vulkan/NVIDIA 580.173.02, Linux/X11, Rust 1.95, two workers.
The corpus has 1000 IDs over 32 hardlinked synthetic JPEG contents. Input digest
validation warms pages outside the app measurement; kernel page cache is not
flushed. Each native reopen has fresh process RAM/VRAM. These are hardware planning
trials, not noisy CI timing thresholds or artist-media quality evidence.

App clocks start at product dispatch, not before OS exec: native startup ends at
window/GPU initialization; first-recognizable is CPU submission before present,
not photon latency. The ordinary usefulness marker is 80% of the current viewport,
not automatically all 1k objects. Product scripted duration includes startup;
existing regression traces retain their established 12s navigation clock. Shutdown
drain is reported separately and finishes with zero pending work here. Asset state
counts cover requested sources only. RSS uses 10 ms sampled /proc VmHWM; tiny CLI
process peaks may be missed. Percentiles use floor(p×(n−1)) for product and postprocessed regression frame-upload
CPU values. Native regression CPU/acquire/present/callback distributions retain
the existing ceiling convention. JSON records the upload convention and each raw
root explicitly; raw counts and maxima are retained.

### First creation and size

| Measure | Result |
| --- | ---: |
| Header/import metadata | 131.9 ms |
| Overview preparation | 35.396 s |
| Final snapshot save | 140.6 ms |
| App total / process wall | 35.684 / 35.749 s |
| Peak RSS | 75.1 MiB |
| Logical source bytes read | 7,110,972,177 |
| Worker-generated overview PNG bytes | 46,821,740 (44.653 MiB) |
| `.tack` bytes | 47,215,836 (45.029 MiB) |
| Header/metadata/directories overhead | 394,096 bytes (0.835%) |
| Generated / errors / peak pending | 1000 / 0 / 16 |

Progress records 0,100,250,500,750,900,1000 prepared images; this one-time cost is
charged explicitly, consistent with 0.7's 35.7 s planning evidence. Embedded bounded
PNG creation also succeeds, then its external original is deleted before reopening.

Byte counters are logical application reads/writes, not physical disk traffic.
Source bytes include header/derivation reads and external original copy for embed;
stat operations are excluded. Container counters cover metadata/worker validation
and embedded copy where reported, not every final-save copy. `derived_bytes_written`
means worker-generated temporary PNG bytes, excluding their recopy into `.tack`.
For this linked creation, another 46,821,740 bytes are read/copied into the final
file, and the same payload is written again there; total derived payload writes
are 93,643,480, plus metadata/seed writes. Repair validates all persisted
previews, writes 140,604 regenerated PNG bytes, and whole snapshot save still
copies every final overview. It is not a 3-blob in-place update. GPU/CPU payload
budgets are not physical VRAM/RSS limits.

### Prepared, missing and repaired reopen

| Scenario | Native startup ms | Metadata ms | First / viewport80% ms | `.tack` logical bytes | Reuse / regenerate | RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| reopen-prepared | 577.9 | 4.774 | 684.3 / 701.2 | 581,050 | 4 / 0 | 318.4 |
| reopen-warm | 557.8 | 4.318 | 688.7 / 688.7 | 581,050 | 4 / 0 | 317.6 |
| reopen-tour | 604.4 | 4.303 | 696.0 / 1650.9 | 47,215,836 | 1000 / 0 | 378.7 |
| reopen-missing | 547.6 | 3.774 | 627.2 / 643.4 | 581,050 | 4 / 0 | 318.5 |
| reopen-repaired | 588.3 | 5.280 | 686.8 / 686.8 | 581,050 | 4 / 0 | 317.7 |
| reopen-embedded-after-deletion | 567.8 | 0.062 | 653.7 / 653.7 | 957 | 1 / 0 | 316.0 |

All these opens read **zero external source bytes** before detail; this slice
issues no detail requests. Ordinary prepared view requests 4 images, reads metadata
plus those previews, then idles with only 2–3 recorded redraws: percentile estimates
there are not representative steady-state timing. Maxima are retained in JSON.
Prepared overview navigation is near-startup/subsecond for the ordinary viewport;
the full tour reaches 80% of its much larger current viewport in 1650.9 ms.
This additional supply/upload time is identified, not hidden as full-board readiness
at first content. There is no multi-second original re-preparation on reopen.

The 12 s full tour reuses all 1000 previews, has 95.49% weighted recognizable
coverage, CPU p99 2.684 ms, upload CPU p99 1.888 ms, GPU-pass p99 1.561 ms
and callback p99 8.127 ms (641 frames). GPU pass excludes uploads and
presentation. All frame upload≤8/16 MiB, in-flight≤3 and payload≤128 MiB bounds pass.
Three hidden sources leave the document usable; two are requested in the initial
viewport and explicitly marked missing with last-known previews (the third is
outside that requested set). Tests separately cover moved/foreign/changed links.

Three corrupt overview payloads repair with 997 reuse/3 regeneration/zero errors:
preparation 232.4 ms, snapshot save 135.1 ms, total 370.2 ms,
source reads 21,331,750 bytes. Authoritative metadata before/after is byte-identical,
confirmed by checksum and direct comparison. A subsequent open performs no repair.
The 96×64 embedded PNG remains drawable after external deletion: metadata-only
open is lazy and valid overview reuse reads no original bytes. Separate tests
remove its preview and regenerate from the original; JPEG embedded streaming is
also tested with the external original deleted.

### Product metadata/query sanity

| Objects | Metadata/file bytes | Load ms | Query p50 / p99 / max ms |
| --- | ---: | ---: | ---: |
| 1,000 | 135,186 | 1.248 | 0.085 / 0.111 / 0.128 |
| 5,000 | 675,186 | 3.716 | 0.390 / 0.442 / 0.588 |
| 10,000 | 1,350,186 | 7.447 | 0.907 / 1.443 / 1.520 |

Generated metadata uses one shared asset/source and 1000/5000/10000 objects; each
of 100 actual ordered queries returns 9 visible objects. This tests metadata/query,
not decoding/rendering 10000 different images. The allocation-free ordered BTreeMap
scan is O(n log n), not a spatial index. At this scale it does not justify an R-tree;
no 50k readiness claim is made.

### Existing renderer/asset regressions

Six paired 12 s traces use the frozen 1A executable and final 1B executable, with fresh
per-trace caches, followed by paired pressure/normal-pan repeats. Preparation and
prefetch remain off. The existing harness validates every frame's resource bounds.

| Trace | CPU p99 1A / 1B ms | Recognizable 1A / 1B % | 1B GPU pass p99 ms | 1B RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| pan-normal | 5.508 / 5.846 | 81.28 / 80.54 | 0.646 | 535.8 |
| pan-fast | 2.476 / 1.746 | 77.57 / 77.87 | 0.184 | 499.7 |
| zoom-traverse | 0.734 / 0.739 | 41.63 / 41.51 | 1.540 | 491.1 |
| board-tour | 2.061 / 1.283 | 5.64 / 5.85 | 0.290 | 356.1 |
| pressure | 2.364 / 2.491 | 95.50 / 95.50 | 0.379 | 448.9 |
| pan | 0.273 / 0.367 | 0.00 / 0.00 | 0.053 | 343.8 |

Pressure main pair is 2.364→2.491 ms CPU p99;
repeat is 2.345→2.297 ms. Upload CPU p99 is
2.071→2.166, then
2.104→2.069 ms. Uploaded
textures/bytes are 18/80,510,976 main,
18/80,510,976 repeat. This does not
erase 1A's earlier 2.241→2.912 and 2.344→2.893 ms increase: that history remains a
baseline risk. Current timings are measured samples rather than a causal
improvement or statistical equivalence claim. Pressure requested-LOD coverage is
71.87→70.32%,
then 71.55→70.65%:
changes remain recorded despite stable recognizable coverage. No profiler-based
explanation for sampling variation is claimed.

Normal-pan repeat gives 5.925→5.890 ms CPU p99,
recognizable 80.00→80.45%,
requested 22.63→22.21%;
main requested 23.49→21.30%.
Main normal-pan RSS is 550.2→535.8 MiB;
repeat 534.2→552.6 MiB. Payload
bounds pass; RSS includes the allocator, driver and process overhead.
Distant jumps still show 0% recognizable content in this backpressure diagnostic;
they are not useful navigation. Final traces remain about 59 completed submissions/s
with the established cadence. Resource bounds and broad frame envelope remain
retained, with explicitly visible tails. Background fairness/high-LOD contention
and requested coverage remain open work. Native present max is
11.150 ms for board-tour,
13.869 ms for pressure, and callback max is
11.952/19.683 ms.
Across all eight final regression traces, present max is 25.963 ms
and callback max 26.180 ms (both pan-normal).
GPU-pass/CPU p99 therefore do not guarantee a native presentation deadline.

## Failures, checks and independent verification

[Verification evidence](../benchmarks/phase1b-verification.json) records commands,
exit outcomes and local log hashes. Successfully executed:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo doc --workspace --no-deps --locked
python3 -m unittest discover -s tools -p 'test_*.py'
cargo deny check
cargo test -p tack-render --test gpu_smoke --test product_gpu --locked -- --include-ignored
```

Full default suite: 67 passed including 3 compile-fail doctests,2 explicit GPU
tests ignored there and run successfully separately (3 GPU-test cases, one geometry
case overlaps default suite); 6 Python tests pass. Dependencies: advisories/bans/
licenses/sources all pass; duplicate-transitive warnings remain the existing policy.
No command is reported passed merely because CI is configured.

Persistence tests cover truncation, magic/schema/record/capability refusal,
count/length/range overflow, bad references/duplicate IDs, invalid geometry/crop/
opacity/filter/order, overlaps, 1000 deterministic CRC-repaired mutations,
stable generations/cursors, native non-Unicode/foreign descriptors, revision
undo/divergence and dirty state. Derived tests cover missing/corrupt/stale entries,
one failure among valid entries, unavailable/changed links, independent repair,
embedded PNG/JPEG deletion, CPU eviction/disk reuse, private files/directories,
input corruption rejected on save, failed replacement/temp ownership and 4 real
SIGKILL stages. The Unix sparse 20 GiB fixture opens by reading 433 metadata bytes
and independently reads 8 final bytes; this proves addressing/lazy container open,
not a real 20 GiB image decode or Windows sparse support. Hostile repeated JPEG ICC
markers exhaust the 64 MiB parser budget without a source-sized byte bank. CLI tests
prove cross-parent relative repair refusal, existing/newer create/repair target
protection, same-input repair, normalized report aliases and hardlink preservation.

Two required independent read-only contexts did not implement code. The main
verifier reviewed persistence bounds/versioning, architecture, frame ownership,
performance accounting/provenance and regression applicability. The additional
security context reviewed codec/ranges, save ownership/modes, relative bindings
and losslessness. Both contexts independently reran the final three CLI tests
and passed the report/repair target safeguards. Security review is PASS. The main
verifier independently reran 45 Rust cases plus 3 compile-fail doctests and
audited final raw measurements, hashes, percentile conventions and frame bounds.
Material findings were fixed: private temp/PNG modes; relative SaveAs refusal;
existing create/repair protection; exclusive report outputs; stable repair
cache/quota; final GPU residency; occluded-idle timer; post-trace drain; failed-read accounting; PNG metadata caps.
Deferred original CRC, camera and Windows boundaries stay explicit rather than
being presented as solved. Final commands and evidence are recorded in the linked
verification JSON; neither independent context implemented code.

Windows CI is configured for full checks/tests and pinned native build. Phase 1B
Windows execution is pending the first push; no Windows atomic-save/path/power-loss
validation is claimed at this point. CI evidence will be updated if the run completes.
Linux explicit GPU readback is included in the configured workflow.

## Remaining risks and stop gate

No polished import/preferences/save UI, product selection/manipulation, higher LOD,
autosave or relink dialog is built. Snapshot save copies all embedded bytes and
requires complete replacement space. Preparing a generated preview set beyond
512 MiB can evict earlier inputs and fail final save safely; 100k metadata bounds
are not a 100k import/scaling promise. Camera remains ±1e8 although farther valid
document coordinates are preserved (clamped initial view is reported). Partial
original CRC, same-size/mtime change ambiguity, single-writer assumptions
(including the check-to-rename concurrent-create window in create/repair), Windows
filesystem/ACL behavior and power-loss evidence remain limited. EXIF/ICC/color,
representative artist media, future schema migrations/opaque retention, spatial
scaling beyond 10k, fairness/high-LOD contention and requested coverage remain later
measured work. No silent feature stubs or storage candidate implementations remain.

The local persistence/image slice is coherent enough to author Phase 1C—Core image
interaction. It does not authorize executing that phase. Stop after 1B, commit/push
per the user's standing request, then wait for human report review.

A — PASS
