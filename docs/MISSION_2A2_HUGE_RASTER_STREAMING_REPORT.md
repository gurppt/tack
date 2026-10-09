# Phase 2A2 — bounded huge-raster streaming

2026-10-09. Status: **TECHNICAL PASS** — local gate, GPU, independent review
and exact-source Linux/Windows CI green.
Baseline: Phase 2A1 implementation `5c2c0bb`, closeout `e62b233`.
Normative brief: `briefs/CODEX_PHASE_2A2_HUGE_RASTER_STREAMING.md`.
Durable results, source/binary hashes and raw-receipt hashes:
[measurements](measurements/phase2a2.json). Native artifacts stay under
`benchmark-results/phase2a2/`; generated rasters and caches are not Git payloads.

The measured answer is yes for visited regions: a real 50,000 × 50,000 JPEG
works with the existing small potato budgets, and a restart reuses its local
detail from SSD without running the codec. Cold JPEG regions still require
sequential entropy traversal. A distant uncached corner takes about two seconds
to refine with potato budgets on this modern host; the camera and overview
continue to work. This does not establish Pentium-class CPU performance.

## Implementation and decisions

The existing generic Product/source/revision supply now accepts bounded worker
batches. Neighboring JPEG requests at one mip form a filled rectangle of up to
15 guttered tiles, with both aggregate output and the native crop footprint
charged at four bytes/pixel limited to 4 MiB. Scanlines scatter directly into
the outputs: there is no rectangular RGBA intermediate, full decoded raster,
coefficient bitmap, eager pyramid, hidden ring, second renderer or new codec.
The first pending tile anchors the greedy rectangle; arbitrary request order
does not promise globally optimal packing. Production demand is row ordered.
Coarse mips whose native stride exceeds the regional cap retain the bounded
single-tile route. PNG retains its generic single-region provider and opt-in
regional mode; it gains the same persistent tile reuse.

The scheduler still admits at most 16/default or 4/potato keys, including active
keys, with two/one workers and one bounded result slot per worker. A batch keeps
one cancellation token. Overlapping pans can retain useful products; removing
every batch demand cancels the helper. Each completion must still match the
current source, revision, asset and demand before publication. Existing native
header checks, strict decoding, 30-second watchdog and process reaping remain.
The actual JPEG header must agree with declared source dimensions before the
regional helper starts. Source, cache and codec I/O remain on workers.

A lazy raw cache lives at the local profile's
`raster-cache-v1/tile-detail-v1.raw`. Its quota is subtracted from the existing
combined derived-disk allowance: 64 + 448 MiB/default, 4 + 4 MiB/potato.
One file contains a checked 16-byte global prefix and fixed 266,384-byte slots
(128-byte checked identity/header plus at most 258² RGBA bytes). File length,
including padding, counts against quota. It grows only for visited detail.
Maximum startup work is 251 slot headers, 32,144 bytes including the prefix;
there is no directory/pyramid scan, database, mmap or background cache service.
A compact bounded index and persisted slot stamps provide LRU replacement.

The 80-byte key includes the document/canonical resolved-path namespace
(existing SHA-256 truncated to 128 bits), source/revision, actual size/mtime,
embedded original CRC when applicable, declared dimensions, mip/tile and
generator. Linked fingerprint and path identity are checked before reuse and
again before publication/writes. This follows the existing linked-source
fingerprint contract; it is not a whole-original content hash or a guarantee
against an external editor preserving both size and mtime.

Slots validate dimensions, length and CRC. Writes invalidate the previous
header before changing payload and commit the checked header last. Existing
files must have the cache-only global prefix before any truncation/mutation;
Unix additionally rejects multiple hardlinks and changed inode/link ownership.
Cache deletion, corruption, lease conflicts or write errors leave originals,
canonical metadata, saves and recovery independent and usable. An invalid global
prefix disables reuse until that disposable file is removed. One profile lease
has one cache owner; another concurrent window falls back to source supply.
The optional library route without a configured persistent cache preserves its
old single-product cache; coalesced products on that route only reuse RAM.

No package identity was added. Existing `crc32fast` became a normal assets
dependency; existing `sha2` supplies only the small path namespace. Licenses,
advisories and sources pass. The server executable is byte-identical to Phase
2A1, so this work adds no rendering/decoder baggage to `tack-server`.

## Real fixture and provider comparison

`tools/phase2a2_square.py` streams grayscale procedural PGM rows into the pinned
libjpeg-turbo 3.2.0 encoder: baseline quality 70, exactly 50,000 × 50,000.
The row is 50,000 bytes and the largest feed block 1,600,000 bytes; no full PGM
or bitmap file is written. Encoded size is **33,425,392 bytes**, SHA-256
`38081b27d2070876f89f496d81d9056da58696661f6d5a5cb223787040f01701`.
Generation takes 4.76 s; verified generator/encoder VmHWM is 18,728/16,988 KiB.
The initial generator receipt inherited a misleading launcher RSS maximum;
the independent regeneration receipt supplies actual `/proc` VmHWM and the
same source hash. A full RGBA source would require 10,000,000,000 bytes.

The release comparison requests the same twelve bottom-right mip-zero tiles.
Complete RGBA equals the single-tile reference; twelve independent coordinates
match the known source pattern within JPEG gray tolerance ±2, with alpha 255
and twelve distinct CRCs. The comparison reference's 3,195,072 RGBA bytes are
test storage, separately accounted from production outputs.

| Shape | Helpers | Elapsed | Logical source reads | Aggregate output peak |
| --- | ---: | ---: | ---: | ---: |
| Twelve singles | 12 | 7.623 s | 395,579,232 B | 266,256 B |
| One 4 × 3 rectangle | 1 | 0.629 s | 33,161,544 B | 3,195,072 B |
| Three bands | 3 | 1.875 s | 98,894,808 B | 1,065,024 B |
| Rectangle + adjacent column | 1 | 0.737 s | 33,161,544 B | 3,993,840 B |

The rectangle cuts elapsed time about 12.1× and logical reads about 11.9×.
A full ring would require thirty tiles and exceeds the cap; the adjacent column
is comparison-only. Production prefetch is zero. The rectangular native crop
has a 3,354,120-byte charged RGBA footprint, without an allocated crop bitmap.
Sampled helper VmHWM is about 2.3 MiB, not an exact allocator/scratch measurement.
These are single serial provider samples with warm/unspecified OS page cache.
Logical reads are feeder/parser work, not physical SSD reads or random access.

## Raw versus encoded persistent products

64 alternating release reads per 256² tile, warm OS page cache; both paths
perform actual file I/O and validate resulting pixels/CRC.

| Content | PNG bytes | Raw padded slot | PNG read p50 | Raw read p50 | Raw reopen |
| --- | ---: | ---: | ---: | ---: | ---: |
| Flat | 1,915 | 266,384 | 0.247 ms | 0.110 ms | 0.087 ms |
| Noise | 262,488 | 266,384 | 0.334 ms | 0.104 ms | 0.042 ms |

Raw hits are 2.25–3.22× faster here and avoid image decode/encode. Flat PNG
storage is much smaller and its 0.173 ms encode beats the 0.242 ms raw write;
noisy PNG encoding takes 1.383 ms versus 0.068 ms raw write. The raw cache is
chosen for revisited detail under a strict quota, not universal compression or
write superiority. Packed slots bound file count and startup indexing; no
isolated packed-layout versus individual-raw-file speedup is claimed.

## Native scenarios A–H

Owned isolated X11 display `:99`, Linux RTX2060, 800 × 600; actual wheel and
middle-drag input, no test camera or cache bypass. Camera poses are independently
tracked and asserted. Times below are first fully resolved matching native
frames after input; external quiet waiting adds roughly 1.5 s and is excluded.

| Scenario | Default | Potato | Result |
| --- | ---: | ---: | --- |
| A recognizable preview | 575 ms | 563 ms | Fit has no tile demand; complete admitted overview about 1.1 s. |
| B deep zoom | 374 ms | 1,046 ms | Six tiles; source pixel occupies 1.92 screen pixels. |
| C horizontal new detail | 337 ms | 316 ms | Overlap reused; bounded newly exposed products. |
| C vertical new detail | 362 ms | 338 ms | Same bounded behavior. |
| E exact same-session return | 16 ms | 19 ms | Zero added source reads or region jobs. |
| F sharp zoom-out | 12 ms | 13 ms | Zero HD tile demand. |
| D first extreme | 68 ms | 135 ms | Overview remains available while detail arrives. |
| D opposite extreme | 767 ms | 1,997 ms | About 46,075 source pixels away; bounded cold refinement. |
| E hot restart detail | 30 ms | 73 ms | Six raw hits, zero detail source reads/region jobs. |

Restart first reconstructs the overview (about 1.05–1.13 s); that source work is
separate from the hot-detail measurement. Default deep demand coalesces into
one helper. Potato's four-key admission serves six visible tiles progressively
in three bounded jobs; its cold opposite corner costs about three entropy
traversals. Fitting the source does not require a complete source pyramid.

| Resource | Default observed peak | Potato observed peak | Scope |
| --- | ---: | ---: | --- |
| CPU pixel cache | 6,971,744 B | 6,172,976 B | Owned display pixels; budgets 64/8 MiB. |
| GPU texture payload | 6,971,744 B | 6,971,744 B | Cache payload, not physical VRAM; budgets 128/16 MiB. |
| Persistent allocated file | 5,860,464 B | 3,995,776 B | Includes prefix, headers and padding. |
| Whole-process VmHWM | 366,809,088 B | 362,577,920 B | Includes runtime, other scratch and GPU driver. |
| Sampled helper VmHWM | 2,342,912 B | 2,457,600 B | External 50 ms sampling can miss brief peaks. |
| Upload bytes/frame | 1,597,536 B | 1,048,576 B | Potato maximum two uploads/frame. |
| Admitted pending keys | 6 | 4 | Global limits remain 16/4. |

Regional outputs can additionally own up to 4 MiB per active job outside CPU
LRU, while whole-source overview outputs retain their separate 16 MiB cap.
Scanline/feeder/parser/native scratch and in-flight GPU resources are separate.
No total-RSS or exact physical-VRAM cap is inferred from display-cache budgets.
Wheel callbacks p95 are below 0.001 ms, maxima 0.0054/0.0105 ms. Inclusive frame
callbacks p95 are 26.9/23.5 ms and include acquire/present; they are not input or
codec CPU times. The first opposite-corner camera frames arrive in 14/9 ms
with the overview recognizable and zero ready detail tiles: refinement does not
hold the camera until decoding completes. Settled two-second intervals have zero redraws and zero logical
or physical I/O, with one/zero process CPU ticks default/potato.

G combines the actual square, ordinary JPEG, ordinary PNG and a 50k-axis PNG.
All four visible references become recognizable and resolve admitted quality
at open, deep zoom, return and overview in both profiles. Huge PNG opt-in
tile pan/return and existing color 50k-axis JPEG are separately exercised, with
complete settled detail, no errors, no idle redraws or I/O. The PNG provider
still rescans sequential rows; this phase does not add random PNG access.

## Regression, review and provenance

- Full post-review Rust/Clippy/tests/docs/Python/dependency gate passes
  (`gate-03.log`, 65 assets tests plus two explicitly ignored measurement tests).
  The explicit GPU suite passes all 14 tests: smoke, selection, nine product
  tests, two convergence witnesses and the long churn test.
- Worker churn covers 1,200 real cache-eviction/readmission turns and unchanged
  saved authority; the explicit GPU long-churn test covers 12,000 uploads.
- Native LOD witness: 2,520 wheel transitions, 32 pans, five filtering changes,
  save/reopen, 229 trace frames and zero best-resident quality valleys. These
  events are not falsely counted as thousands of completed codec transitions.
- Twelve paired serial local runs cover ordinary PNG/JPEG, the existing huge
  panorama with ordinary references, and a 1,000-image mixed board. Startup
  remains in the measured half-second range and RSS remains about 351–356 MB;
  no consistent material ordinary-image regression is established. Every idle
  interval has zero redraws/physical I/O and no Tack TCP/UDP/shared worker.
- All 32 inherited native shared-image/CAS/server checks pass, including
  authority convergence, local board isolation and conflicting gesture handling.
- Cache tests cover quota reduction, corruption, deletion, eviction, exclusive
  lease conflicts, hardlinked originals, changed fingerprints/paths, revision
  changes and stale completions. Linked/embedded save/reopen preserve authority.
- [Independent review](reviews/phase2a2_huge_raster.md) found two concrete
  issues: copied relative-path cache namespace collisions and destructive open
  of a cache hardlinked to authority. Both are corrected and regression tested;
  no unresolved source blocker was found. Local and exact-source CI gates pass.

Final stripped client: **19,329,864 bytes**, SHA-256
`861bbeb67c39572372585eba53250543ff4892c93fcb376e449c43d8f8c72664`,
192,192 bytes (+1.0%) above the equally stripped Phase 2A1 baseline.
Human binaries are `bin/tack` and `bin/tack-server`; `bin/BUILD.txt` records
the raw executable hashes and source checkpoint. Server SHA-256 remains
`3e43ef7addba4eae197761f87e6287c2b570bb6ab2121a5950c93c8cd0417108`.
About artwork and the pinned decoder also remain byte-identical.
Only test/harness files changed after this production build; their final hashes
and gate receipts distinguish that from executable changes.

Disk discipline: 11,774,385,362 bytes of obsolete generated Rust incremental
cache were removed, retaining current-phase cache/native tools and all owner
files. Phase artifacts are bounded and existing axis/ordinary fixtures reused;
the duplicate generator raster was removed after hash verification. Historical
failed harness attempts are excluded from acceptance. At closeout, the reserve
is about 22.7 GiB free; retained phase artifacts occupy about 128 MB.
No full-source caches enter Git.

Implementation commit: `873ce97cb57dc525441feebb94156073b3ebe87b`.
[Exact-source Quality run 37898157574](https://github.com/gurppt/tack/actions/runs/37898157574)
passes Linux, Windows and dependency jobs, including Linux software-Vulkan GPU
tests. The closeout changes only documentation/evidence. No Windows desktop/GPU
runtime acceptance is inferred from CI. Remaining subjective review is recorded in
[the human queue](HUMAN_REVIEW_PENDING.md); the two-second cold refinement and
sequential codec limitations above are measured technical facts.
