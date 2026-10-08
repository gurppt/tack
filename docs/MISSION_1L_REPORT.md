# Phase 1L — local polish and bounded huge PNG prototype

2026-10-08. Finalization of the existing 1L implementation after interruption;
Phase 1K baseline: `ef7a5e3085d495429c8e4ee6c38a3223aec3bee4`.

**Checkpoint: local technical PASS for the delivered scope.** Local feature
fixes and the bounded static PNG path pass the final gate, native/GPU checks and
33 final-binary scenarios. Regional tiles remain explicitly experimental and
opt-in (`--huge-tiles`). Remote CI status is recorded separately below.
No LAN, media playback or Phase 2A implementation was started.

## Delivered local behavior

- Board creation/Save As appends `.tack` to arbitrary dotted names and accepts
  an existing case-insensitive suffix; atomic ownership/save rules remain.
- Keymap export defaults to `.tackey`; import accepts this and compatible legacy
  JSON. Invalid imports preserve the active profile. Export refuses an existing
  suffix-adjusted destination that the picker did not explicitly confirm.
- Selection highlights derive from authoritative membership, including all
  visible selected members rather than the former small UI overlay limit.
  A lazy instanced outline pass supports the document limit of 10,000; no
  selection allocation/texture is retained when the selection is empty.
- Theme/Scale choices remain in their submenu and apply immediately. Escape
  returns to Preferences, then closes it. Nested Keymap returns to Preferences;
  directly opened Keymap closes directly and releases capture/search state.
- Open reuses only the initial, empty, clean, unrecovered Untitled slot with no
  pending mutations or history. The worker acquires and validates the new board
  before installing document state. Failed/cancelled Open preserves the seed;
  meaningful/dirty work uses the existing separate-window behavior.
- Successful Open/Save As remembers a bounded native board-folder descriptor;
  cancellation does not change it. Directory validation is on the picker worker,
  with a safe fallback for deleted/inaccessible directories.
- Normal Note corner drag changes the wrapping box without changing the font.
  Shift drag scales box, font and stroke from a captured initial basis. Each
  drag/release creates one undo transaction; redo is exact.
- Save Original As copies linked or extracts embedded encoded bytes with a
  128 KiB buffer on the existing local worker, temporary sibling publication,
  cancellation and embedded CRC checks. It performs no decode or transformation.
  Linked files modified during copying are not immutable snapshots.
- Default Arrow stroke/head increase from 3/20 to 4/24. Curves and attachments
  are deferred under the brief's feasibility allowance: new serialized controls
  and attachment relations would distract from the supply/authority validation.

The updated [feature inventory](FEATURE_INVENTORY.md) and
[owner checklist](HUMAN_TEST_1L.md) describe the resulting product behavior.

## Huge images: delivered capability and limits

[Design note](design/huge_images.md) contains the routing, geometry, codec and
cache policy. Source decoding of large **static noninterlaced PNG** now samples
bounded rows into overview/detail products instead of retaining a whole source
frame. Tiles extend the same source/revision keys and bounded product caches;
no full pyramid, sparse texture/page table or permanent service was added.

The Normal PNG path has a 32 MiB ceiling for live pixel buffers, with source
channel/16-bit routing accounted for. RGBA estimates through 16 MiB remain Normal;
16–32 MiB are Large; greater than 32 MiB are HugeTiled. Both Large and Huge use
rows. Legacy wide/tall axes also route to rows, even at a small pixel count.
The source precision estimate can force rows earlier. This is independent of
compressed size and of the motivating 50,000-pixel example.

The 8 MiB parser allowance, bounded rows (2 MiB each), allocator/runtime and
native codec scratch are separate costs. Parser limits are best effort; exact
scratch peaks were **not directly measured**. Normalization/drop ordering and
precision regression tests establish the source pixel-buffer bound; RSS is not
used as its proof. Disposable cached PNGs have dimensions checked before decode,
with at most a 2048 edge, or 256 for tiles. Corrupt high-precision cache entries
can require an additional bounded conversion buffer; the Normal-source 32 MiB
claim is not an all-cache or whole-process memory claim.

Every uncached tile rescans and validates the PNG through its tail. Sampling is
nearest and there are no tile gutters. This limits memory but amplifies I/O and
can alias fine detail/reveal smooth-filter seams. The measured detail latency
supports keeping tiles opt-in. Whole streamed PNG supply is enabled normally.
Huge Adam7/APNG and inputs beyond checked axis/pixel/row/256 MiB read limits
fail explicitly. No 50k-square or gigapixel runtime claim is made.

JPEG keeps reduced DCT supply and conservative 6000×4500/64 MiB encoded/32 MiB
native output guards. The safe decoder wrapper lacks a proven regional/scratch
bound for larger sources. The 8192–50000 JPEG cases measure bounded refusal,
not successful detail or JPEG tiles. A quiet failed source is never counted as
successful convergence.

## Measurement method and provenance

All native runs are serialized on this Linux host, NVIDIA RTX 2060/Vulkan and
an isolated owned X11 display at 800×600. Builds/corpus generation do not overlap
CPU performance runs. The old baseline executable is preserved once; the final
binary is built with pinned Rust 1.95.0. OS page cache was not flushed; opening
uses a fresh application/derived-cache process, not guaranteed physical cold I/O.
Warm return means the same process; reopening rebuilds its temporary derived cache.

Raw reports, process samples, logs and screenshots remain under ignored
`benchmark-results/phase1l/`. The checked-in
[compact measurements](measurements/phase1l.json) bind board/binary/report hashes,
stage coverage, p50/p95/p99 CPU/callback timings, GPU samples, queue peaks, upload
bytes/counts, payloads, source/container I/O and settled process observations.
Callback timing includes FIFO/presentation waits; `cpu_ms` is the separate frame
work measurement. GPU timing comes from timestamp queries, not CPU submission.
First recognizable/useful clocks are CPU submission before presentation.

The external quiet detector waits for 1.5 seconds of low process activity; it is
not a coverage assertion. Stage rows independently inspect actual visible,
quality-resolved and requested/resident tile counts. The optional detail latency
is the first complete frame matching the final stage camera/zoom after gesture
settling starts; rapid zoom can transiently request other mips. Disk values are
settled PNG-file snapshots under the owned temporary directory, not a directly
sampled all-time scratch/cache peak. I/O counters include process work in the
observed interval; logical source reads differ from physical OS reads.

### Ordinary workload comparison

| Fixture | HWM MiB old → final | 80% useful ms old → final | CPU frame p95 ms old → final | Coverage at detector end |
| --- | ---: | ---: | ---: | ---: |
| empty | 230.87 → 231.55 | — → — | 1.132 → 1.010 | 0/0 |
| realish | 265.82 → 266.28 | 436 → 394 | 1.086 → 1.075 | 1/1 |
| stress1000 | 267.37 → 267.64 | 10314 → 10727 | 3.734 → 3.354 | 1000/1000 |
| unique250 | 250.39 → 250.53 | 2582 → 2448 | 1.313 → 1.154 | 250/250 |
| mixed1000 | 270.93 → 271.28 | 10809 → 10765 | 3.354 → 3.343 | 1000/1000 |
| mixed1000-potato | 254.08 → 254.50 | 20075 → 20468 | 3.207 → 3.221 | 998/1000 |

The potato25-second row records998/1000 at the detector deadline; full coverage
follows before the actual idle sample. The40-second repeat verifies1000/1000
and an unambiguous quiet interval, as detailed below.

The 1000-path fixture requests 1000 source identities even though its physical
payloads are hardlinked. The unique-250 fixture is a distinct workload. Startup
is progressive: a first useful reference in roughly half a second does not mean
1000 references finish immediately. Measurements are observations, not repeated
statistical estimates or a Pentium III/128 MiB feasibility claim. Driver/runtime
fixed RAM already exceeds that aspirational target on this host.

### PNG crossover and axis sweep

| PNG dimensions / class | HWM MiB old → final | First recognizable ms old → final | Final errors |
| --- | ---: | ---: | ---: |
| 1024², Normal | 278.96 → 279.46 | 431 → 379 | 0 |
| 2048², Normal boundary | 302.41 → 284.98 | 438 → 461 | 0 |
| 3072², HugeTiled | 317.75 → 280.19 | 473 → 385 | 0 |
| 4096², HugeTiled | 307.86 → 280.36 | 465 → 445 | 0 |
| 6000×3000, HugeTiled | 297.18 → 257.18 | 463 → 458 | 0 |
| 50000×4096, HugeTiled | refused → 236.84 | — → 654 | 0 |

The sweep probes below, at and above the 4,194,304-pixel Normal boundary, then
axes 4096/6000/8192/12000/16000/24000/32000/50000 at the generated bounded heights.
It supports a conservative fixed decoded-risk budget and shows avoided large
bitmap allocations. It does not prove a universally optimal 32-versus-64 MiB
performance cutoff. The 32 MiB pixel policy is justified by explicit allocation
accounting plus these observations, rather than a 50k threshold or host RAM.

### Regional tiles, fairness and return

| Scenario | HWM MiB | Peak CPU/GPU payload MiB | Deep tiles ready/requested | Deep completion s / warm return ms | Logical source reads MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| huge-tiled | 244.83 | 6.09/6.09 | 20/20 | 5.08 / 14 | 714.18 |
| huge-tiled-potato | 238.75 | 2.28/2.28 | 6/6 | 1.60 / 12 | 399.27 |
| huge-multiple | 249.11 | 10.25/10.25 | 32/32 | 6.12 / 14 | 884.68 |
| huge-multiple-potato | 240.76 | 4.04/4.04 | 8/8 | 1.36 / 11 | 468.88 |
| mixed-visible | 306.22 | 41.88/41.88 | 20/20 | 5.43 / 15 | 700.06 |
| mixed-visible-potato | 242.90 | 5.07/5.07 | 6/6 | 1.53 / 18 | 395.80 |
| huge-reopen | 244.50 | 6.09/6.09 | 20/20 | 5.08 / 13 | 713.03 |

Each supported tiled stage must show full logical coverage and all admitted
resident tiles; rejected JPEG is listed as unsupported. Ordinary-source fairness
is checked with a genuinely visible huge+four-reference layout, replacing the
older huge+100 board whose chosen view showed only the huge image. Concurrent
huge sources receive per-object quotas and a coarser mip where needed. More
eligible objects than available slots remain an explicit coarse-fallback limit.

Default budgets remain CPU 64 MiB/GPU128 MiB/disk512 MiB, two workers/16 requests;
potato remains CPU8 MiB/GPU16 MiB/disk8 MiB, one worker/four requests. Upload
ceilings are 16 MiB/eight per frame, potato1 MiB/two; at most three GPU submissions.
Tiles use at most 64/eight full slots before ordinary reservations reduce them.
Cache payload accounting excludes scratch, queues' bookkeeping and driver RAM.

Observed final peaks:16 pending/14 queued by default,4/3 in potato;
16 MiB/two uploads per frame by default and786,432 bytes/one in potato;
one reported GPU submission in flight. Across final runs CPU/GPU payloads peak
at43,910,144 bytes default and5,313,024 potato. Maximum settled derived PNG sizes
are39,871,854 bytes default and4,423,670 potato; these are snapshots within the
hard disk ceilings, not exact all-time peaks.

| Final scenario | CPU frame p50/p99 ms | Callback p50/p99 ms | GPU pass p95 ms |
| --- | ---: | ---: | ---: |
| stress1000 | 3.012/4.088 | 25.800/30.683 | 1.378 |
| single huge tiles | 0.164/0.722 | 21.947/27.509 | 0.099 |
| huge+ordinary potato | 0.137/1.023 | 21.176/27.297 | 0.358 |

In the final mixed cold view, four ordinary references resolve before the PNG
finishes its whole-image products. The raw frames show4/5 recognizable at about
440ms, with huge work still pending, then5/5 recognizable. Deep mixed views show
three logical images (huge plus two ordinary), pan shows four, and overview five;
each settled supported stage resolves every visible image and admitted tile.
This is visible-source fairness evidence, not a guarantee for arbitrary banks.


Settled windows are checked separately for redraws and I/O over two seconds.
A few process CPU ticks can belong to driver threads; no claim of zero OS thread
activity is made. The 25-second quiet watchdog can expire on the potato1000
cold-open workload while final quality still reaches1000/1000; that timeout is
reported distinctly. In the final25-second run its conservative100ms idle
margin includes the last completion frame preceding the actual idle sample;
exact-sample I/O is zero. A separate run with40-second maximum settling detects
quiet at full1000/1000 coverage and proves0 idle frames/0 I/O, without ambiguity.

## Correctness and validation

- `PATH=/tmp/tack-tools/bin:$PATH bash tools/check.sh`, Rust1.95.0,
  cargo-deny0.20.2, exit0: fmt, all-target/all-feature locked check,
  Clippy with warnings denied, tests, docs, Python and dependency audit.
  **224 Rust tests pass; 11 GPU tests are ignored by this default gate;
  26 Python tests pass.**
- Explicit RTX2060 execution: gpu_smoke1, product_gpu7, selection_gpu1 and
  lod_convergence2: **11 GPU tests pass**. A Linux software-Vulkan CI step also
  includes the new selection test.
- Source cache invalidation tests reject tile detail after Changed/Missing/
  Unavailable/Foreign detection, preserve existing last-known whole overviews,
  reject old revisions and discard/re-admit cancelled huge work. An oversized
  highly compressed PNG cache is rejected by decoder dimension limits before
  allocation of its claimed bitmap.
- `huge_authority` tests exercise actual HugeTiled8192×1025 linked and embedded
  PNGs. They compare original bytes across save/reopen, ensure generator3
  disposable products become compatible generator2 persisted overviews, reuse
  and delete/regenerate a partial tile, and reopen dirty recovery through a
  fresh BoardLease. No tile is serialized into original/document authority;
  an embedded original still works after the external source is deleted.
  This is a focused storage/supply test, not a simulated native GPU crash.
- Linked and embedded original export tests compare bytes/hashes and cover
  cancellation, corruption, non-regular paths and destination/source guards.
- Native workflow and ordinary LOD receipts: final counts/provenance below.

The final native receipt (`local-picker-fixed`) has **21/21 successful checks**,
including same-process fresh Open, native picker-directory argument, cancelled/
failed Open, dirty separate-window Open, unchanged note geometry/history/redo,
menus, Note gestures and byte-identical linked export. Binary SHA-256:
`a9a44778aa66ff53ba57b42d10dc4cc6aa1d73519fa9360b8c0fae4693f3a1de`.
Harness SHA: `71dcd3867a867d39fef237674d6aa9cfea98f50f93e27738ebe6e2f76c18521c`.
The [source manifest](measurements/phase1l-source.json) hashes 226 relevant
source/build/test files; aggregate `78c2203260f94e12f605e1df810fe98452c87e3cab0d4e29a21fd3800f97cc72`.

Ordinary native LOD zoom oscillation passes all nine checked stages, including
HD → low → HD and rapid/progressive crossings:104 traced frames,17 codec jobs,
25,690,112 decoded bytes,23,592,960 peak CPU/GPU payload bytes,a maximum of five
pending requests. Final visible coverage1/1, two-second idle0 redraws/0 I/O
(one process CPU tick). The board contains eight generated mixed/grouped/rotated
sources; the final zoom shows one, rather than eight simultaneously.

A small native800×600 screenshot set is retained in
[measurements/phase1l](measurements/phase1l): Theme, dirty-note Open picker,
and procedural huge detail. The screenshots support inspection; they are not
independent timing or subjective-quality acceptance.

Earlier gate/automation attempts are retained as diagnostics. In particular,
the new dirty-Open automation needed to avoid GTK inline-completion races;
failed automation is not presented as a product PASS. The report uses only the
completed final receipt for that assertion.

## Corpus, disk and reproducibility

The reusable owned corpus is `test_file/phase1l_generated`, **411.719 MiB physical**
after additions (1.88GB apparent including hardlinks). The four base sources are
25 hardlinked JPEG payloads for1000 paths (~24.9 MiB), 250 unique JPEGs (~248.3 MiB),
96 mixed payloads for1000 paths (~44.4 MiB), and16 axis JPEG/PNG inputs (~86.3 MiB).
Three crossover PNGs add~5.9 MiB; additional boards reuse source files.

Quality45 was chosen after a small JPEG quality trial to respect the512 MiB
reservation. An earlier attempt stopped safely at the budget guard; it was
removed only through the owned cleanup manifest before successful generation.
The final base generation took208.13 seconds. Larger nominal JPEG fixtures use
grayscale to limit Pillow working memory; this procedural bank is not a real
photographic production corpus. Metadata-only board creation avoids decoding
with the new algorithm before the baseline encounters each source.

The generator journals owned paths, refuses unsafe ancestry/symlinks/unowned
additions, and maintains a10 GiB free reserve. Its552-line module keeps one
cohesive reservation/generation/ownership path; tests check deterministic source
bytes, hardlinks, guarded failure and cleanup. Local raw evidence is retained;
no corpus or binary is committed. Superseded pre-final screenshots and JPEG
quality-trial payloads were inspected and removed (23,563,700 bytes); all raw
frames/samples/summaries and final screenshots remain. Corpus plus local evidence
and the small checked-in screenshot set occupy about477 MiB, below512 MiB,
with about35 GiB free. Shared Rust build artifacts are accounted separately. The owner's adjacent `test_file/test_file.tack`,
`gfx/` additions and director coordination files are preserved.

Commands and safe cleanup are in the [design note](design/huge_images.md).
Rebuilding the generator's boards reproduces their semantics/geometry; native
identity and absolute linked-path metadata can change board-file hashes.

## Dependency and executable weight

`png = 0.18.1` was already transitive at1K. Cargo.lock adds only the direct
`tack-assets` dependency edge, with no new package/version identities. Direct
access is needed for header/row reading, parser limits and metadata suppression.
Its manifest license is MIT OR Apache-2.0; the final deny gate passes advisories,
bans, licenses and sources. No framework or new image codec was added.

Like-for-like `strip` sizes: baseline18,028,136 bytes, final18,271,176 bytes,
**+243,040 bytes (+1.35%)**. The shipping release retains debug information
(117,515,520 bytes); it must not be compared to the stripped baseline as code
weight. The temporary stripped comparison file was removed after measuring.
The baseline ELF text/data/bss are17,585,862/438,680/8,072 bytes; final values are
17,828,754/438,928/10,768. There is no claim of zero cost from zero new crates.

## Checkpoint and publication

Implementation commit and remote CI receipt will be recorded at publication.
Earlier user authorization to commit/push at phase completion remains in force;
the closeout addendum itself supplies no additional publication authorization.
The human executable is prepared atomically with `tools/build-test-bin.sh` and
its About sidecar. `bin/BUILD.txt` identifies commit, SHA and checkpoint.
Unrelated untracked owner/director files make its whole-worktree dirty flag true;
the source manifest and final binary SHA identify the tested implementation.

## Closeout boundary

[Independent review](reviews/phase1l.md) assesses the final source and receipts.
Subjective review remains in the local director queue `docs/HUMAN_REVIEW_PENDING.md`
under the director's newer policy. Windows CI compilation/tests and Windows
native desktop behavior are different claims; desktop usability remains deferred.

After the technical closeout, this worker stops. The director audits this
report and activates an updated next mission or requests a bounded correction;
the old2A brief is not automatically active.
