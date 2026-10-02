# Mission 0 architecture

The repository originally contained README, local briefs and `gfx/tak.jpg`,
with one initial Git commit and no application, tests or configuration.

Four crates own distinct responsibilities:

- `tack-core`: f64 world geometry/camera, viewport intersection, projected-size
  LOD choice and byte-accounted LRU policy. No GPU/window/storage dependencies.
- `tack-assets`: bounded manifest validation, immutable object snapshot, worker
  scheduling, JPEG/PNG decode, persistent display cache and decoded CPU cache.
- `tack-render`: texture ownership, vertex generation, submission and completion
  polling. It receives rectangles/asset representations, without a local/shared
  document distinction. No filesystem access or decoder invocation.
- `tack-app`: native window/input, composition and scripted benchmark telemetry.

The static benchmark geometry uses a linear visibility scan. At 1,000 objects
this is a simple measurable baseline, not a claim of sufficient scaling to
arbitrary boards. Objects outside the camera rectangle never emit quads. The
same renderer is used by the window and a 1280×720 offscreen target.

Display levels have longest edges 128, 512 and 2048 pixels. Originals are decoded
only by workers and dropped after downsampling. An absent image first requests a
thumbnail, then the projected-size LOD. Existing lower LOD stays visible during
refinement; otherwise a quiet placeholder is used. Detail beyond 2048 pixels,
tiling, EXIF/color management, prefetch and production import are not implemented.

The frame path polls channels and GPU completion without waiting. Each worker
has one bounded job channel, one bounded result channel and at most one active
job. Stable ID sharding keeps cache paths reusable across process restarts.
Current visible demand is reconsidered every frame; obsolete active jobs receive
an atomic cancellation flag. The codec itself cannot be interrupted, but flags
are checked before/after decode, LOD generation and cache writing. Obsolete
results are discarded before cache insertion. Failed visible keys are suppressed
until demand leaves the viewport, preventing error retry loops.

| Resource | Initial policy |
| --- | --- |
| Immutable manifest | <= 4 MiB, <= 10,000 objects |
| Decoders | 2; API permits 1–4 |
| CPU display cache | 64 MiB; pressure scenario 16 MiB |
| GPU display cache estimate | 128 MiB; pressure scenario 24 MiB |
| Persistent display cache | 512 MiB total, stable per-worker quotas |
| Encoded input | Capped read of 64 MiB + sentinel; growth beyond cap rejected |
| Decoded dimensions | <= 6000×4500 original; <= requested LOD in cache |
| Decoder allocation hint | 192 MiB per decoder; image library's best-effort limit |
| GPU uploads | <= 2 textures and <= 16 MiB RGBA per submitted frame |
| GPU submissions | <= 3 outstanding; skip rendering when saturated |
| Retained telemetry | <= 7,200 frames; benchmark duration <= 120 s |
| GPU timestamps, when supported | 3 fixed resolve/readback slots; <= 7,200 samples |

Cache counters are payload budgets, not total RAM/VRAM. CPU memory also includes
two capped encoded buffers (with Vec capacity growth), transient JPEG decoder
copies, original pixels, resize/PNG scratch, one result per worker, bookkeeping
and telemetry. Limits on file bytes, dimensions and concurrency bound this work,
but max_alloc is not an OS memory sandbox. Peak process RSS is measured separately.
GPU accounting excludes the fixed vertex buffer (~0.96 MB), timestamp buffers,
placeholder,
target/swapchain, driver overhead, aligned allocations and staging. Outstanding
submissions may retain evicted textures; at most three submissions and a bounded
upload volume prevent an arbitrarily growing submission backlog. Actual driver
memory needs hardware profiling before production budget claims.

Worker-owned disk cache evicts oldest-written entries (FIFO on disk, LRU in RAM),
uses temporary files followed by rename, recovers corrupt entries and disables
itself if unavailable. Cache failure never hides a valid original. This is
disposable cache persistence, not the final document/save format. The benchmark
manifest supplies hash identities; originals are not rehashed during loading.

The GPU baseline uses one draw/bind group per visible image, one reused vertex
buffer, and explicit display textures without mip chains. No elaborate batching
framework is introduced before measurement. Input redraw is coalesced; when no
input/loading work remains, GPU callbacks are polled briefly without submitting
new frames and the event loop then sleeps. Hidden/zero-size windows skip surface
acquisition and redraw until restoration. Scripted runs disable manual camera
input and resizing so the prescribed demand stays reproducible.

Optional GPU timestamps surround the canvas render pass, excluding uploads and
presentation. Three readback slots are mapped asynchronously; busy slots drop
a timing sample instead of waiting. Only shutdown drains outstanding work, for
at most five seconds. Native telemetry separately measures acquisition, present
and the entire redraw callback, and counts failed surface acquisitions. Attempt
intervals remain scheduling cadence, not monitor presentation timing.

Tests cover cursor-anchor stability, invalid geometry, LOD thresholds, LRU byte
eviction, corrupt/unavailable caches, warm reuse, stale results, encoded input
limits, and a GPU readback proving uploaded red pixels reach the target. Linux
and Windows CI are configured; running locally does not establish a remote CI
result. No production persistence, collaboration, media player or product UI
has been added.
