# Phase 1L independent review

2026-10-08. Independent source and receipt review of the existing Phase 1L,
based on Phase 1K commit ef7a5e3085d495429c8e4ee6c38a3223aec3bee4.

**Verdict: technical PASS for the delivered local fixes and bounded opt-in PNG
prototype.** No unresolved corruption,
unbounded huge-frame allocation or material source blocker was found. All
previous source findings are resolved. This review reads source and serialized
integrator-produced evidence; it does not run a competing build or benchmark.

Source manifest SHA256:
78c2203260f94e12f605e1df810fe98452c87e3cab0d4e29a21fd3800f97cc72.
All 226 entries match current files. Final executable SHA256:
a9a44778aa66ff53ba57b42d10dc4cc6aa1d73519fa9360b8c0fae4693f3a1de.
The executable, final performance cases, native checks and ordinary LOD run
agree on this identity. The refreshed final gate exits 0 and its log SHA256
91b3752e999bf2a44f0d6dcf05a164d432f54a7486cf9f5317f06b96b8ecc58f
matches the compact measurement receipt. Final commit/report assembly belongs
to the integrator; the reviewed technical checkpoint is green.

The acceptance scope matters: static noninterlaced huge PNG supply is supported;
regional tiles remain experimental and opt-in. Larger JPEGs are deliberately
refused. This verdict does not assert Windows native behavior, a directly
measured decoder-scratch peak, 50k-square usability or 128 MiB feasibility.

## Resolved source findings

- A detected changed/missing/unavailable linked source could previously reuse
  a cached tile. `product::load` now rejects tile tags unless source state is
  Available or Embedded before touching the derived cache; `supports_tiles`
  also checks that state. The warm-cache test covers Changed, Missing and
  Unavailable while retaining the last-known overview. The capability test
  covers Foreign and explicit revision removal. `check.log` records all these
  tests passing, plus stale tile publication and cancelled huge tile readmission.
- The Normal PNG memory estimate previously omitted retained 16-bit buffers and
  overlapping RGBA conversion. `huge_image::streamed_layout` now includes source
  precision in its routing estimate. `derive_normal` normalizes with STRIP_16,
  checks its output buffer against 16 MiB, ignores text/ICC chunks, destroys the
  PNG reader before thumbnailing and destroys the original before converting
  the thumbnail to RGBA8. The live pixel buffers stay within the 32 MiB candidate
  ceiling. The passing precision-routing test checks 2048-square RGBA16 enters
  generator 3 and RGB8 stays in generator 4. Parser, row/unfiltering scratch and
  allocator overhead remain separate costs; whole-worker RSS is not capped at
  32 MiB.
- Disposable cache generators 3 and 4 identify the streamed and bounded Normal
  PNG algorithms. `PreparedOverview.generator` maps these to 2 using `min(2)`.
  Both `tack-storage::reader` and its save validation accept generator 1 or 2;
  the persisted payload is still a bounded RGBA8 PNG overview. No tile is added
  to persisted preview authority because preparation requires Thumbnail/128.
- Cached tile decoding previously applied a 2048-pixel image limit and checked
  tile dimensions after allocation. `decode_cached_png` now sets width/height to
  the requested raster edge and allocation allowance to edge²×8 before decode.
  The passing compressible-1024-square test rejects this payload for a 256 tile.
  Writer-produced cache PNGs are RGBA8. A corrupted 2048-square higher-precision
  cache can still retain up to 32 MiB decoded pixels plus 16 MiB conversion;
  the source Normal route's 32 MiB pixel ceiling should not be claimed as a
  universal cache/worker/RSS ceiling. The cached case remains finitely bounded.
- Keymap export originally appended `.tackey` after picker confirmation and
  could replace a different existing destination. `preferences::export_keymap`
  now locks and refuses an existing suffix-adjusted target. An already suffixed
  deliberate target still uses the normal replacing writer.
- Tile admission originally spent all slots on the first eligible draw. The
  planner now counts eligible draws, assigns a remaining per-draw quota, and
  selects a coarser mip to fit. This fixes the several-source starvation case
  within the slot count. More eligible images than slots remain an explicit
  admission limit. The several-huge native receipt has three visible sources
  and frames with all 32 admitted tiles resident; potato has frames with all
  eight resident. This proves converged admission across this finite scenario,
  not fairness for every arrangement or more visible objects than available slots.
- Wide small PNG metadata previously admitted Adam7/APNG that the derivation
  route could not handle. The shared `requires_streaming` size predicate now
  covers both metadata eligibility and derivation, including the legacy axes.

The initial concern that admitted resident tile textures were not refreshed
before insertion was withdrawn. Tiles enter the common demand list before the
`product_window.rs:368` residency-touch loop; that loop preserves coded tile
edges and runs before ordinary and tile uploads. Final quality accounting runs
after all uploads.

## Inspected behavior and limits

Fresh-window replacement checks the initial seed identity, zero generation,
empty object/asset/source collections, clean history, recovery state and pending
mutations. The worker opens and leases the new board before publication. The
installer prepares fallible supply/profile/input state before replacing editor
authority, then resets document input, selection, history, source supply, GPU
products, original handles, camera and recovery state. Cancellation drops an
already published open result; failed reads leave the existing document owned.
The native receipt observes the same process and single window before/after
fresh Open. Its separate failed/cancelled paths preserve the initial document.

Board suffix handling preserves dotted and non-Unicode native filenames and
accepts case-insensitive `.tack`. Last-board-directory validation runs on the
picker worker and falls back for missing/inaccessible directories. Successful
board installation and Save As update the bounded profile descriptor.

Source export captures an original descriptor or pinned stored range, copies
with a fixed 128 KiB buffer on the existing local worker, validates embedded
stored CRC, writes a private sibling temporary and atomically publishes only
after completion. It performs no decoder, transform or re-encoding operation.
The native linked-source export check is green, and focused local tests cover
embedded originals and corrupt CRC rejection. Concurrently modified linked files
are not immutable snapshots.

Selection outlines derive from authoritative membership and gesture previews.
The new optional instanced GPU path supports up to the document's 10,000 visible
members independently of the small UI overlay budget. Its storage is absent
without visible selection. Notes use explicit text-box dimensions and font/style
values; Shift corner scaling derives every preview from captured initial values
and commits one batch. The native note history checks and the explicitly run
selection GPU test passed in the inspected receipts.

The codec prototype is static noninterlaced PNG. It uses bounded scanlines,
checked output layout, normalized 8-bit channels, cancellation, capped cumulative
reads, and tail CRC/IEND validation. It never constructs a full-resolution frame,
full-width stripe or complete disk pyramid. Tile derivation still scans the PNG
from the beginning, including its tail, for each requested representation; bounded
memory does not establish acceptable latency. The tiled renderer is opt-in.

Huge JPEG is not proven by this stack: turbojpeg 1.5.1 exposes reduced scaling
but no safe regional decompression/scratch cap, and jpeg-decoder's progressive
coefficient allocation precedes its output-size limit. Conservative JPEG guards
remain necessary. The 32 MiB PNG pixel policy is conservative and supported by crossover
fixtures; it is not a universal optimal threshold. The existing worker/request/cache budgets
and source/revision keys remain authoritative; derived tiles are not originals
or document authority.

## Final receipts and accepted limits

The reviewed check-final.log records 224 successful Rust tests, 11 GPU tests
ignored by ordinary cargo test, 26 successful Python tests, and cargo-deny
advisories/bans/licenses/sources OK. gpu-checkpoint.log explicitly executes nine
renderer tests; gpu-lod-checkpoint.log executes both LOD convergence tests.
The final tooling-gate refresh has also completed successfully with exit 0;
its hash is recorded above and agrees with the measurement receipt.

The two huge_authority integration tests create a streamed 8192×1025 PNG whose
RGBA estimate crosses the Huge boundary. Linked and embedded variants derive
overview/partial boundary tile, save/reopen identical document authority, reuse
a disposable tile, then delete and reconstruct identical tile pixels. Embedded
original bytes survive external-source deletion. Recovery preserves original
bytes/revision and compatible generator-2 overview, marks recovered work dirty,
and leaves the normal board save unchanged. No tile becomes container authority.
These are actual huge-class storage/supply tests, not a native crash simulation.

The final local-picker-fixed receipt has 21 observed successful Linux/X11
checks at 800x600, bound to the final executable and harness SHA256
71dcd3867a867d39fef237674d6aa9cfea98f50f93e27738ebe6e2f76c18521c.
It covers fresh Open/failure/cancel, actual next-picker directory, dotted Save
As, Preferences hierarchy, note resize/scale/history and linked original export.
Dirty-note Open retains the original window, unsaved geometry and history while
launching the chosen board separately; redo remains exact. Earlier GTK completion
and child-window automation failures are superseded by the successful final
receipt, not treated as product successes.

The compact measurement receipt contains baseline and 33 final runs on the
same RTX 2060/Vulkan host and native 800x600 display. Final runs use the identified
executable; the saved stripped baseline SHA256 is
20c8f368f51898e2a676a9e9be3192b0e2f0144c8bc1a46d0919ba7bee9ff45f.
Ordinary final endpoints reach complete visible coverage with zero errors.
Single-run RSS stays close for ordinary banks; callback distributions vary and
are not statistical proof of equivalence. The root report must retain measured
differences rather than describe every metric as unchanged.

The corrected huge+ordinary fixture has five visible objects at opening and
overview, and three during deep zoom. It reaches 5/5 coverage, then 20/20 admitted
tiles and 3/3 visible quality; potato reaches 6/6 tiles and 3/3 quality. The earlier
one-visible fixture is superseded and does not prove ordinary-source fairness.
Several-huge deep zoom converges all three visible sources and 32/32 tiles.
This demonstrates fairness within the tested quota; it does not prove admission
for more eligible objects than available slots.

The PNG sweep reaches usable overview through a 50,000-pixel axis. Larger JPEG
axes end with two errors, no display payload and quality 0/1. Quiet unsupported
JPEG is explicitly a refused source, not successful convergence. Single huge
deep-zoom complete admission takes about 5.1 seconds and several huge about
6.1 seconds; warm returns are about 14 ms. These submission-clock measurements,
nearest mip sampling, absent tile gutters and sequential source rescans justify
keeping regional tiles opt-in.

New-process huge reopen repeats 1/1 visible coverage and 20/20 deep tiles with
zero errors. It rebuilds a temporary derived cache; it is not a persistent
tile-cache hit, and OS source cache was not flushed. The ordinary native LOD
run crosses tiers repeatedly, then settles after 104 trace frames, 17 codec
requests and 23,592,960-byte CPU/GPU payload peaks with adequate final 1/1
quality and zero measured idle I/O/redraws.

Default peak requests/queue are 16/14 and potato 4/3. Maximum observed upload
bytes remain 16 MiB default and 0.75 MiB potato; maximum observed uploads/frame
are two/one. Payload, queue and upload ceilings hold. Derived-disk values are
settled snapshots, not directly measured all-time peaks. Per-stage coverage and
callback p99 are retained with raw-report hashes.

The original potato-1000 cold run exceeded the 25-second quiet watchdog. Its
broad sample window counted one late frame before the actual idle interval;
final coverage reached 1000/1000. The separate
perf-final-mixed1000-potato-settled repeat uses a 40-second watchdog, detects
quiet after 26.8278 seconds, reaches full 1000/1000 coverage with zero pending
or queued requests, zero errors, zero idle frames and zero recorded idle I/O.
This resolves the ambiguity without presenting the earlier timeout as PASS.

The final stripped executable is 18,271,176 bytes, versus 18,028,136 baseline:
+243,040 bytes (+1.35%). png 0.18.1 was already transitive; adding its direct edge
introduces no new package identity. The passing full dependency gate covers
licenses/advisories. The source manifest, executable and measurement receipt
are mutually consistent; final commit identity belongs in the closeout report.

The design note explicitly separates the Normal-source 32 MiB pixel ceiling,
parser/rows, corrupt-cache conversion and process RSS. Decoder scratch peak
remains unknown; payload bytes and RSS are not substitutes. Sequential PNG I/O
amplification, sampled mip quality, larger-JPEG refusal and grayscale procedural
JPEG fixtures are documented accepted limitations. No Windows runtime or final
remote CI claim follows from the local evidence.

The director closeout addendum supersedes the old blanket human-review pause.
Subjective review may be deferred and does not itself block technical acceptance.
This review does not activate the older Phase 2A brief; the director owns that gate.
