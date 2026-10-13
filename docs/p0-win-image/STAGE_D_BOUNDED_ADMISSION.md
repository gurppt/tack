# Stage D — bounded JPEG/PNG admission and real large-source proof

**Scope:** meaningful >64 MiB baseline JPEG/noninterlaced PNG now enter the
existing local supply/storage pipeline. This is not universal giant Photoshop
support, a new container format, PNG native-tile promotion or Windows acceptance.
Physical B1 dialogs are confirmed; remote Linux guest reachability remains separate.

## Resource plan instead of a raised global constant

One metadata-only `image_import_plan` is shared by GUI import, CLI create,
Linked/Embedded transition and relink. It returns dimensions, format, whether
streaming is required and an independently derived original-byte allowance.
Ordinary sources retain **64 MiB** whole-input allowance and existing adapter.
Larger encoded JPEG uses the existing validated 8-bit single-scan baseline
scanline adapter even if pixel dimensions fall within the ordinary thresholds;
it never allocates the whole compressed original for NativeThumbnail.
Larger PNG forces the bounded static row path, including interlace/animation
eligibility checks, independently from pixel dimensions.

Existing JPEG/PNG cumulative decoder read budgets stay **256 MiB**. Admission
reserves its observed metadata/header read plus **16 KiB** for the renderer's
layout probe/rewind buffers. It therefore does not promise the entire read budget
as usable encoded-original bytes. Example allowances: 268,418,742 B JPEG,
268,410,872 B PNG for the exact corpus below; header-heavy files can have a lower
allowance. This is a conservative resource plan, not a reason to raise every cap.

Unchanged independent guards: JPEG header 1 MiB, streamed scratch 16 MiB,
codec axis 65,500, helper output 16 MiB and 30 s scoped timeout; ordinary JPEG
coefficient scratch 192 MiB/output 32 MiB; PNG internal parser 8 MiB, row 2 MiB,
output 16 MiB; supply CPU 64 MiB and existing GPU/disk budgets; embedded batch
2 GiB/session 4 GiB, with a 128 KiB streaming copy and unchanged CRC/fsync.
Online 512 MiB original/cache policies are independent and unchanged.

The production supply receives already-known fingerprint/blob-range lengths;
no new End seek/stat is required for ordinary source dispatch. Generic legacy
readers keep their bounded route and Start-seek contract. A failing parser-work
witness initially revealed this compatibility issue; it was corrected without
weakening assertions. No new dependency, renderer, resident worker or idle timer.

## Genuine fixtures and end-to-end results

Main private receipts: `benchmark-results/p0-win-image/stage-d/`.
Valid encoded files, not padding/sparse stand-ins:

| Fixture | Layout | Encoded bytes | SHA-256 |
|---|---|---:|---|
| 8000×9000 JPEG | L8 baseline, quality 100, seed 261013 | 113,652,624 | a6e8fff32c7951bd64083ca30fdbcc21d732883ec454e6d36d0f6d487fecc7fd |
| 8000×9000 PNG | L8, noninterlaced, seed 261013 | 72,034,446 | 1729abceb3c99cd40319d0713f96dba27a8a9e7b377215a783f078df82072512 |
| 6000×4500 JPEG | RGB8 baseline, quality 100, 4:4:4, seed 261014 | 110,997,089 | 51f8e3c08838e82c70847dbb80288496bd88222976a67a82670240eef22a3e90 |

All three succeed Linked **and** Embedded through actual ImportRequest →
ProductAssets 128/512/2048 representations → durable save → reopen, exact document
comparison and original CRC/SHA-256. The ordinary-dimensions RGB case proves
encoded size alone can trigger the scanline adapter. Complete original-byte
hashing happens only in this explicit acceptance probe, not ordinary startup.

JPEG first preview ~505 ms; 512/2048 tiers ~516–696 ms each in measured runs.
PNG Embedded first preview ~73 ms, 512 ~96 ms, 2048 ~171 ms. These are independent
tier times, warm/uncontrolled OS cache, sampler adds overhead. Embedded save
~209–369 ms for these sizes; Linked save ~8 ms. No multi-GB extrapolation.
Pure supply sampled PSS (parent + children, 20 ms samples) ~30–53 MiB for the
grayscale corpus, CPU cached payload peak 15,899,648 B; no full 288 MB source RGBA
allocation. RGB has separate successful receipts, not the same PSS measurement.

Six real GUI opens also display on **private Xvfb/software Vulkan**, 800×600:
overview then Actual Size. All zero errors, JPEG six ready native tiles, PNG zero
native tiles by current default (2048 representation remains). CPU/GPU payload
accounting peaks ~16.69 MiB in the gray JPEG case, below unchanged budgets. Worst
software callback observed ~35–73 ms during updates; no decoder-on-UI claim.
Each settles to zero CPU ticks and IO over 3 seconds. Captures visually inspected;
this is not physical NVIDIA/Windows VRAM or driver performance acceptance.
The error-details UI has a separately corrected/inspected native clipboard-import
receipt after an initial harness `new` invocation error; no failure was hidden.

## Ordinary control and object-count stress

Same 20 unique RGB 600×400 sources, 10 JPEG/10 PNG, isolated llvmpipe, three
fresh processes per side. Control exact a9260b9; current C/D source snapshot.
Neither global cache eviction nor physical GPU/Windows measurements claimed.

| Median | Control | C/D |
|---|---:|---:|
| Startup → mapped | 125.36 ms | 120.86 ms |
| Settled PSS | 76.49 MiB | 76.77 MiB |
| Callback p50 / p95 / p99 | 11.58 / 12.04 / 20.17 ms | 11.56 / 12.16 / 18.69 ms |
| Observed Ctrl+S ack | 206.47 ms | 205.40 ms |
| Accounted GPU payload | 3,403,776 B | 3,403,776 B |
| Threads / settled ticks / IO | 11 / 0 / 0 | 11 / 0 / 0 |

No meaningful regression shown within the short/noisy samples. Callback includes
submission/present wait; PSS includes driver mappings, not just application heap.

1k/4k/10k placed-object checks run sparse **and explicitly dense** viewports,
same control/current and navigation/selection/Undo. **One source, one asset**:
these measure repeated-reference rendering, not 10k unique codecs. Dense max
visible equals object count, zero errors. Single-sample worst callbacks control →
current: 25.56→34.34 ms (1k), 61.00→64.52 ms (4k), 129.39→120.92 ms (10k).
No speedup/regression conclusion from these maxima; existing dense selection
cost is visible and should be profiled separately, not blamed on new codecs.

## Tests, safety and remaining gates

Decoder/supply/authority suite: **101 passed, 0 failed, 2 existing ignores**.
Admission/worker/library targeted set: **168 passed, 0 failed, 5 existing ignores**.
Two declared-length policy witnesses are explicitly unit-level; valid >64 MiB
admission/render proof comes from the real corpus above, not simulated lengths.
Final Linux lint/fmt pass; full workspace, Windows build/lint, private package and
fault/resource coverage are finalized after the remaining study stages.

No storage/protocol migration, durability relaxation or installed bin replacement.
Temporary corpus exceeded the preferred 512 MiB phase-data budget (not documented
up front); all compact measurements/hashes/captures retained, completed duplicate
spools retired (~296.7 MB), no 10 GiB reserve breach in D. Generated boards/sources
will be removed after final acceptance receipts, retaining reproducible seeds.
Existing user sources, editable icons, caches/worktrees and listener preserved.

**Unresolved:** actual Photoshop files and 1.33/1.478 GB sources are unavailable;
encoded read allowance still rejects them. Large progressive/multiscan requires
coefficient-storage design; CMYK/unsupported precision stays explicit. PNG native
tile default and physical Win10 high-zoom remain gated. Windows/SMB cancellation
and driver behavior cannot be proved by Linux/Wine. E/F/G produce independent
experiments/design notes; no premature public Windows+HD release is authorized.
