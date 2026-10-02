# Mission 0.5: asset pipeline experiments

Measured on 2026-10-02, Ryzen 2700X / RTX 2060. The application and isolated
codec experiments are separate evidence. See [the report](../MISSION_0_5_REPORT.md)
and [compact experimental records](../../benchmarks/mission0_5-experiments.json).

## Profile before changing the decoder

The instrumented Mission 0 loader, two workers, 8-second adjacent normal pan,
spent 9,414 ms decoding and 4,823 ms resizing across observed completed jobs.
Reading took 73 ms, headers 65 ms, PNG encoding 464 ms and writes 37 ms.
Decode p50/p99 was 226/249 ms; resize 65/223 ms. This justified reducing
original resolution before changing disk format or renderer submission.
The 20 distant jumps/s test cancelled 62 jobs after decode and supplied no images.
Completed-job totals exclude work still executing at exit.

## Scaled JPEG decoding

An isolated release Rust experiment used eight 6000×4500 corpus JPEGs,
pre-read into memory, alternating the old image/zune full decode and
jpeg-decoder 0.3.2 scaled decode. Times below are medians in milliseconds;
headers are included in this microbenchmark's decode time. It is distinct
from application stage profiling. No nested Rayon threads were enabled.

| Requested edge | Old full decode + resize | Scaled decode + resize | Scaled RGB output |
| --- | ---: | ---: | ---: |
| 64 | 227 + 39 | 126 + 1.1 | 1,266,750 B |
| 96 | 227 + 52 | 127 + 1.4 | 1,266,750 B |
| 128 | 227 + 60 | 129 + 1.7 | 1,266,750 B |
| 512 | 226 + 64 | 127 + 7 | 1,266,750 B |
| 2048 | 227 + 201 | 137 + 121 | 20,250,000 B |

The old RGB output was 81,000,000 B at every edge. JPEG DCT scaling has a
minimum 1/8 resolution: reducing the final thumbnail from 128 to 64 does
not halve source decode work. The selected runtime uses scale(edge, edge),
then image's thumbnail resize and RGBA conversion. The corpus aspect ratio
makes this equivalent to the experiment's scale(edge, 3×edge/4).

[jpeg-decoder's API](https://docs.rs/jpeg-decoder/0.3.2/jpeg_decoder/struct.Decoder.html)
provides 1/8, 1/4, 1/2 and full scales. The project is explicitly in
[maintenance mode](https://github.com/image-rs/jpeg-decoder), with MIT/Apache-2.0
licensing. This is a documented experiment-stage dependency exception, justified
by measured scale support, not a claim of active feature development. Version
0.3.2 is pinned; optional Rayon is disabled and adds no runtime dependencies.
The full workspace cargo-deny check passed without advisory exceptions.
No Tack-owned unsafe or new native runtime boundary was introduced. The image
JPEG feature remains only for integration-test fixture encoding, not application
loading. Color/EXIF handling and arbitrary photographic formats remain outside
this prototype.

A separate Pillow 10.2 / libjpeg-turbo DCT reference completed tiny decode +
resize in approximately 70 ms. This motivates the next **bounded, isolated
native-decoder experiment**, but is not a measurement of a Rust binding or
proof of its memory/security properties. The [libjpeg-turbo Project's official
API documentation](https://libjpeg-turbo.org/Documentation/Documentation) is the
starting point for that boundary; its native allocations and error handling
require dedicated review before runtime integration.

## Retained overview size

Pillow microbenchmarks, eight sources; median encoded sizes. RGB PNG bytes
here differ from the application's RGBA encoder. RAM estimates are exact
RGBA payload for 1,000 images with 4:3 aspect ratio, excluding allocation and
texture alignment.

| Tiny edge | Board RGBA | PNG per image | JPEG quality 90 per image | Native scaled decode + resize |
| --- | ---: | ---: | ---: | ---: |
| 64 | 11.72 MiB | 7,886 B | 2,083 B | ~70 ms |
| 96 | 26.37 MiB | 18,817 B | 4,662 B | ~70 ms |
| 128 | 46.88 MiB | 34,816 B | 8,604 B | ~70 ms |

Keep 128 for this experiment: the normal 64 MiB GPU overview partition can
hold the entire board's payload; the 32 MiB CPU partition retains roughly
682 thumbnails. Disk eviction prefers keeping thumbnails. Smaller sizes save
residency/transfer bytes but do not solve the measured source latency.
Synthetic PSNR versus full-resolution BOX downsampling was approximately
42.9/37.5/34.7 dB for 64/96/128 respectively; this compares downsamplers at
each size and does **not** rank artist usefulness across sizes. Preview PNGs
are retained under ignored benchmark-results/mission0_5-codecs. No artist
assessment of recognizable content was possible while the user was absent.
Application coverage means any actually submitted image, with tiny/medium/detail
reported separately; it is only a proxy for recognizable references.

## Display format

Isolated Pillow medians; buffer encode/decode, not filesystem or Rust timing:

| Edge | Format | Encode ms | Decode ms | Encoded size |
| --- | --- | ---: | ---: | ---: |
| 512 | PNG | 20.9 | 6.24 | 515 KB |
| 512 | JPEG 90 | 1.22 | 1.61 | 141 KB |
| 512 | raw RGBA | 0.32 | 0.06 | 786 KB |
| 2048 | PNG | 479.7 | 76.3 | 5.49 MB |
| 2048 | JPEG 90 | 13.85 | 16.78 | 1.02 MB |
| 2048 | raw RGBA | 7.72 | 3.52 | 12.58 MB |

Retain PNG. The application encoder is much faster than this Pillow encoder
(e.g. old application detail encode around 35 ms p99). Its measured bottleneck
was decode/resize, then cache-directory traversal and handoff for warm thumbnails.
A lossy display format or raw buffer would change quality/disk pressure without
addressing the first bottleneck. Do not transfer Pillow's 480 ms into an
application performance claim.

## Highest-LOD tiling

Cached encoded PNG tile decode: median 1.76 ms for 256² versus 6.58 ms for
512². RGBA payloads are 256 KiB and 1 MiB. At 2048×1536, the file counts
are 48 versus 12 per image, or 48,000 versus 12,000 for this board. Tiling
6000×4500 originals would create 432 versus 108 files/image: up to 432,000
files. This favors highest-LOD-only consideration, not tiling all originals.

At zoom 0.6 the 1280×720 viewport maps to roughly 729×410 pixels in the
2048 representation. Depending on alignment, intersecting 256²/512² tiles
would transfer approximately 1.5–4 MiB instead of the full 12 MiB RGBA LOD.
Those are geometry estimates, not measured GPU uploads. The timings explicitly
exclude first source decode, tile generation, manifest/index and directory cost;
there is no measured source-region decoder here. Runtime tiling was rejected
for this mission because it would not supply missing overview thumbnails.

## Prefetch, queues and workers

Two workers, 8 seconds, identical traces at an intermediate scaled checkpoint:

| Policy | Normal pan image coverage | Combined scan | Distant jumps |
| --- | ---: | ---: | ---: |
| none | 74.4% | 20.7% | 0% |
| symmetric | 72.9% | 21.5% | 0% |
| directional | 77.2% | 21.8% | 0% |

Both prefetch policies request only tiny offscreen images, at most 64 near
candidates, with visible tiny/medium/detail ahead of them. Direction uses camera
motion sign and a half-viewport lead, not an adaptive prediction. Keep none by
default: the small differences in one trial do not establish a durable benefit.
The bounded comparison switches remain useful experiment controls.

One worker supplied normal pan reasonably but aggressive pan collapsed (8.7%
in the final 12-second worker comparison). Two workers supplied 68.1% aggressive
coverage; four reached 76.7%. Four normal workers raised observed RSS from
554 to 652 MiB and CPU p99 from 2.89 to 6.11 ms for a ~9 percentage-point
coverage gain. Keep two as the conservative default; more threads do not solve
cold-board overview starvation. These are single sequential trials, not
statistically powered rankings. The 1/2/4 comparisons all use the same
archived binary/source pair as the principal suite.

Warm loading initially paid ~2,020 ms in repeated directory scans versus ~262 ms
PNG decode on the board tour. Prepare quotas once at startup and scan on writes,
not every hit. Thumbnail publication and upload batches now allow eight small
results/uploads, while large jobs remain one at a time per worker and uploads
stay below 16 MiB/frame. Large RGBA buffers retain a worker-owned Arc so CPU
cache eviction cannot free their pages on the event thread. CPU-insertion p99
fell from milliseconds to microseconds; these fixes target measured ownership
and handoff costs, not speculative renderer replacement.
