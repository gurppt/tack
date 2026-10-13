# Stage E — GIF first-image boundary, measured experiment

**Decision: stop at the GIF adapter gate. No production GIF support or animation
is enabled in this series.** The Stage C unsupported-format response remains
tested. This is the measured-design alternative explicitly permitted by master
brief §8, not completed GIF product acceptance.

Base: `dev` after `7f19e8c`. Production workspace dependency/lockfile, renderer,
workers, timers and caches are unchanged. The independent experiment lives in
`tools/p0_image/gif_experiment/`; its own manifest/lock are outside the workspace.
Private receipts: `benchmark-results/p0-win-image/stage-e/`.

## Results and semantics

Pinned `image 0.25.10` with `gif 0.14.2` can decode **only the first image** via
`ImageDecoder::read_image`, without iterating animation frames. This fills a
transparent logical canvas, with the first rectangle at its declared offset,
palette and transparency. It is not a disposal/background-composited animated
poster. Subsequent frames, their delays, counts and total duration are unknown.
The [GIF89a specification](https://www.w3.org/Graphics/GIF/spec-gif89a.txt)
defines the screen, image rectangles and control metadata; this choice must be
visible in future product documentation.

Three exact RGBA witnesses pass against Pillow/explicit first-image semantics:
transparent 32×24, animated 32×24 with a differing second image, and a 10×8
rectangle at (5,7) within a 32×24 canvas. First two hash
`bc63e0cd1534f4608363dea350d723e55c8a9ee456fa344b891e42b6e144ab0e`;
partial rectangle hash
`60e44afb93e514a9d3add85f9ff3d610da2464d86fe1726c9b315d2c4171371d`.
Full encoded hashes/layouts are retained in the private fixtures and cost receipt.

| Standalone measurement, Linux, three warm/uncontrolled samples | Result |
|---|---:|
| Stripped PNG control / GIF experiment | 696,120 / 739,384 B |
| Difference in this tiny probe, **not a Tack binary delta** | +43,264 B |
| Small first-image latency | 0.048–0.103 ms |
| 2048×2048 canvas, RGBA 16 MiB | 15.27–28.71 ms; peak RSS 18,536–18,776 KiB |
| 50,001-image stream, 4,550,125 B | only 8,202 logical bytes read; 0.080–0.097 ms |
| 65,535×65,535 logical screen | early explicit rejection; ~2.2 MiB RSS |
| 21,054,092 B pre-image metadata, 32×24 canvas | 19.53–28.02 ms; ~22.3 MiB RSS |

Read counts include the separate 10-byte header and buffered re-read. RSS is
standalone process maximum from `/usr/bin/time`, not private PSS or GPU memory.
The many-image witness proves later frames need not decode, not a global GIF
CPU/time guarantee. No physical Windows evidence or Tack idle delta is claimed.

## Why the simple feature switch is insufficient

Local pinned sources show `GifDecoder::new` reads application metadata **before**
`ImageDecoder::set_limits`. Underlying GIF defaults admit 50,000,000 bytes per
metadata vector; ICC/XMP can be retained. The 20 MiB metadata witness passes even
though the later image limit is 16 MiB and the image is just 3,072 RGBA bytes.
The output limit is therefore not a whole-operation bound. PNG control already
exists; simply enabling `image/gif` also activates `gif`, `weezl`, `color_quant`
(the latter serves encoding). None are added to Tack's workspace in this pass.

Production would also need a distinct admission plan, cold representation
dispatch, stable derived generator/cache identity, accurate errors and picker
filters, original-byte preservation and cancellation/time-budget integration.
Routing GIF through the generic full-canvas image branch would not establish
these contracts. No helper process, player, plugin API or generic media layer is
justified by these measurements.

## Bounded follow-up mission: GIF-POSTER-01

Use the existing supply workers, entered only for a GIF signature. Compare a
direct `gif` decoder with minimal `std` features against a bounded wrapper;
configure decoder memory **before** `read_info`, count cumulative input and check
cancellation. Enforce separate logical canvas, first rectangle, metadata and
output budgets; a decoder per-vector limit alone is insufficient for aggregate
scratch. Initially limit the canvas to 2048×2048/16 MiB RGBA and input to the
existing small-image cap. Define transparent first-image semantics explicitly.

Decode one image only; no count/duration scan or animation timer. Reuse the
128/512/2048 representations and current cache budgets. Preserve Linked and
Embedded originals byte-for-byte; reject oversized/malformed sources truthfully.
Test local/global palette, transparency, interlacing, offsets, out-of-screen
rectangles, truncated LZW, excessive extensions and frame-count bombs. Verify
import → render → save → reopen and stale revision replacement on Linux and
physical Windows. Audit licenses and stripped Tack binary size, compare the
20-JPEG/PNG baseline, threads/wakeups and idle I/O before merging. Playback needs
its own later approval. Rollback here is just removal of the experiment/docs;
no runtime change or board migration exists.
