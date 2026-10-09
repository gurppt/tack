# Phase 2A2 independent huge-raster review

2026-10-09. Independent reviewer; decoder, raw-cache and native integration
authors are separate. Baseline: Phase 2A1 implementation `5c2c0bb`, closeout
`e62b233`. Scope: regional supply bounds, cancellation/publication, persistent
cache authority/quota, worker-only I/O and inherited local/server boundaries.

Status: **SOURCE AND LOCAL TECHNICAL REVIEW PASS — exact-source CI pending**.
No Cargo, decoder, native or GPU execution by this reviewer during the root's
serialized measurements. No concrete source blocker remains. Technical closeout
still awaits exact-source Linux/Windows CI receipts; those pending receipts are
not source defects.

## Initial source audit

- JPEG coalescing admits distinct same-mip tiles forming a filled rectangle.
  The shared pure planner caps outputs at 15 guttered tiles and 4 MiB aggregate
  RGBA; native crop footprint is also charged at four bytes/pixel and capped at
  4 MiB. Scanlines scatter directly into tile buffers; no crop bitmap exists.
  Coarser-stride single requests retain the old bounded scanline fallback.
- The scheduler groups only existing demand keys, never an implicit ring. PNG
  uses the same generic Product tile path with a single-region provider. Global
  request ceilings remain 16/default and 4/potato, with two/one workers and one
  bounded result channel per worker. A batch result owns at most 4 MiB; cache
  hits plus missing outputs remain within the same admitted product count.
- Actual JPEG header dimensions must equal declared source dimensions before
  spawning the regional helper. The provider and scheduler therefore use the
  same geometry; forged dimensions cannot expand an admitted rectangle's real
  output. Each returned tile is dimension checked again before publication.
- All products share the batch cancellation token. An overlapping pan can finish
  still-useful tiles; removing every batch demand cancels it. Publication checks
  current source/revision/demand separately for each returned key. The existing
  helper watchdog/parser/pinned header and safe scanline boundary are reused.
- Persistent cache opens lazily on workers, behind a worker-owned mutex/lease.
  Startup inspects at most 251 fixed 128-byte slot headers, not a source pyramid.
  A distinct 16-byte global prefix is checked before mutating existing files.
  Payloads are dimension/length/CRC checked on hits. Slot writes invalidate the
  old header before payload and commit checked identity last. Missing, corrupt,
  resized or lease-conflicted cache falls back to authoritative source decode.
- Allocated raw-file length is charged, including fixed-slot padding. Quotas
  split existing combined disk budgets: 64 MiB persistent + 448 MiB per-open by
  default, 4 + 4 MiB potato, including the global header and slot padding. No
  eager full quota or full-source cache allocation.
  The native installer passes only a profile path; no cache stat/open occurs on
  the rendering/event thread. Renderer consumes existing generic product pixels.

## Concrete findings

1. **Resolved-path collision in persistent linked identity.** The initial key
   includes board/source IDs, revision, dimensions, actual size/mtime, generator
   and tile, but not the resolved linked path. Copying a board to another
   directory preserves IDs and relative descriptor; different originals with
   identical size/mtime can then reuse the first directory's persistent pixels.
   Per-open isolated caches did not bridge that history. Add resolved-path
   identity or verified content identity, and a two-directory equal-fingerprint
   regression. **Source correction reviewed:** the first sixteen key bytes now
   hold SHA-256(document ID + canonical resolved path), truncated to 128 bits;
   namespace and actual fingerprint are checked again after reads/decode before
   publication/cache writes. The existing sha2 package is reused without a source
   preload. The added two-directory test equalizes actual size/mtime and proves
   different decoded pixels cannot hit the first directory's namespace. This
   regression passes in `assets-final.log` and `gate-02.log`. **Closed.**
2. **Hardlinked raw cache can truncate authority.** Symlinks are refused, but
   an existing private regular `tile-detail-v1.raw` with another hardlink passes
   initial checks. `open` immediately rounds/trims its length, so a hardlink to
   a small original or board truncates that authoritative file to zero. Refuse
   multiply linked files before mutation and test that the external original
   stays unchanged. The storage sidecar itself uses `truncate(false)`; the
   destructive operation is the raw cache's `set_len`. **Source correction
   reviewed:** Unix rejects nlink != 1 before mutation and on each reuse/write;
   portable open verifies the cache-only global prefix/slot-size/CRC on the
   actual handle before trimming existing bytes. Valid JPEG/PNG/Tack originals
   cannot satisfy that prefix, covering Windows without unstable link-count API
   or FFI. Invalid/empty/legacy opaque files remain untouched, with source decode
   fallback. Added tests cover a real JPEG hardlink, unchanged opaque/corrupt
   headers and Unix hardlinks added after open. These regressions pass in
   `assets-final.log` and `gate-02.log`. **Closed.**

## Provider comparison inspected

The actual baseline grayscale JPEG is 50,000×50,000, encoded 33,425,392 bytes,
SHA-256 `38081b27d2070876f89f496d81d9056da58696661f6d5a5cb223787040f01701`.
Its generator streams procedural rows (50,000-byte row; at most 1.6 MiB feed
block), with no full raster file. Native codec limits admit the real square.

`region-comparison-release.json` compares the same twelve near-bottom-right
mip-zero tiles with exact complete RGBA equality and procedural source-pixel
witnesses (JPEG quantization tolerance for the witness only):

| Provider shape | Helpers | Time | Logical source reads | Aggregate output peak |
| --- | ---: | ---: | ---: | ---: |
| Twelve single tiles | 12 | 7.623 s | 395,579,232 B | 266,256 B |
| One 4×3 rectangle | 1 | 0.629 s | 33,161,544 B | 3,195,072 B |
| Three horizontal bands | 3 | 1.875 s | 98,894,808 B | 1,065,024 B |
| Rectangle plus adjacent column | 1 | 0.737 s | 33,161,544 B | 3,993,840 B |

These are serial release-provider, warm/unspecified OS-cache observations, one
sample per case; no GPU or persistence is involved. Test-only reference RGBA
adds 3,195,072 bytes separately from production output. A full one-ring would
need thirty tiles and exceed the chosen ceiling; it is explicitly not admitted.
The result materially supports bounded rectangle coalescing without claiming
JPEG random access. New uncached regions still traverse the entropy stream.

## Cache and native evidence inspected

`cache-comparison-release.log` alternates 64 hot raw/PNG reads including file
I/O and pixel validation. For flat/noise 256² RGBA, raw get p50 is
0.110/0.104 ms against PNG 0.247/0.334 ms; PNG encodes to 1,915/262,488 bytes
against a 266,384-byte fixed raw slot plus the file's 16-byte prefix. Raw trades
compression for predictable work under an explicit byte quota. These warm-cache
modern-host measurements do not establish cold-disk or legacy-CPU performance.

The final native client is SHA-256
`861bbeb67c39572372585eba53250543ff4892c93fcb376e449c43d8f8c72664`.
`square-final-default/summary.json` and `square-final-potato/summary.json`
cover fit, deep zoom, horizontal/vertical pan, both extreme corners, exact
return, sharp zoom-out and reopen. All settled visible tile demands complete.
Both profiles show zero additional region jobs/source reads for exact same-session
return; exact reopened detail has six persistent hits and zero additional jobs
or source reads. Reopen overview itself still reads the source; the zero-read
claim concerns the subsequent hot detail transition.

| Final square profile | CPU pixel peak | GPU pixel peak | Raw cache peak | Process HWM |
| --- | ---: | ---: | ---: | ---: |
| Default | 6,971,744 B | 6,971,744 B | 5,860,464 B | 366,809,088 B |
| Potato | 6,172,976 B | 6,971,744 B | 3,995,776 B | 362,577,920 B |

Potato raw length stays below 4 MiB including padding/header. Quiet windows have
zero measured I/O, with zero potato/one default detector tick. Sampled decoder
HWM is about 2.3–2.5 MB, distinct from whole-process memory and analytical scratch
bounds; 50 ms sampling can miss short helper peaks. Tests/source prove the
height-independent helper bound, row allocation and absence of a decoded full
square/crop bitmap. Native observations are Linux/RTX2060 at 800×600 with
constrained budgets, not proof on an old processor.

`mixed-final-v2`, `mixed-final-potato`, `png-axis-final` and `color-axis-final`
use this same client. Settled mixed stages resolve four visible objects, deep
visible tiles complete, and reported errors/idle I/O/idle frames are zero. PNG
still uses generic tile products and its existing single-region provider.
The earlier `mixed-final` pilot removed its manifest too early and is excluded;
earlier square pilots used a different reopened camera pose and are excluded
from exact hot-reopen acceptance.

`assets-final.log`: 64 tests pass, two explicit measurement tests ignored and
then executed separately in the release comparisons above. `gate-02.log` passes
workspace tests, Clippy, and dependency advisories/bans/licenses/sources. It
includes authority/revision, cancellation, cache absence/lease conflict/corrupt
payload repair, hot reopen and cache-hole band tests. Original/CAS/save/undo
authority remains in existing storage/core/sync layers; no server, core, wire,
renderer or authority source is changed by this patch. The final server binary
SHA-256 `3e43ef7addba4eae197761f87e6287c2b570bb6ab2121a5950c93c8cd0417108`
matches the preceding build; the decoder helper is unchanged too.

`lod-native-final/summary.json` repeats 2,520 wheel, 32 pan and five filter
events, then save/reopen: 218 + 11 trace frames contain zero history valleys.
At 99.18 projected pixels Nearest displays native Medium; Smooth requests
Thumbnail but keeps valid resident Medium. Idle redraws/I/O are zero. This
native scenario has zero CPU evictions, so it does not replace the independent
Rust eviction-pressure stress. `ordinary-paired/summary.json` contains twelve
serial runs (two observations per baseline/current and per corpus). GPU payload
is unchanged in each pair; idle redraws/I/O are zero. Useful startup differences
range from about -19 to +62 ms, within these sparse noisy observations; they
cannot support a statistical improvement or regression claim.

`shared-native-final/checks.json` has 32/32 observed assertions, with final
client/server hashes in `provenance.json`. Three native clients converge with
persisted authority through undo/conflict/restart/rejoin/relink/CAS and original
supply, while the independent local board stays unchanged and has no shared
backend. `gate-03.log` includes the added header/dimensions refusal test and all
workspace checks (65 assets pass, two measurement tests ignored), with the four
deny categories successful. Explicit GPU logs pass smoke 1, selection 1,
product 9, convergence 2 and eviction-pressure long churn 1 tests.

`final-build/source.json` matches current production source hashes. Subsequent
changes seen during review are a test-only declared/header mismatch regression
and native harness refinements, not a changed production binary.

## Closeout receipts outstanding

Exact-source Linux/Windows CI and any additional local receipt the integrator
includes in final closeout. Human visual judgment remains separate from deterministic
pixel witnesses and telemetry. No unexecuted future receipt is reported as a
code defect.
