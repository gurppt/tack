# Phase 1K — About addendum

2026-10-07. Scope: `CODEX_PHASE_1K_ADDENDUM_ABOUT.md`, not a claim that other
Phase 1K work has shipped. Implementation: `785cbf79ebdedc3716c999f91b48ec49c1873425`.
[Compact evidence](../benchmarks/phase1k-about/receipt.json),
[design and packaging contract](design/about_modal.md),
[owner checklist](HUMAN_TEST_1K_ABOUT.md).

About Tack is reachable through the shared right-click Tack section and F10.
Both invoke the same semantic action and produce pixel-identical modals. The
left column reads immutable metadata generated from `gfx/about.toml`; the right
column presents the supplied artwork with nearest sampling. Escape, Enter and
Close dismiss it. The panel uses flat existing bitmap UI, with no new gradient,
font, animation, timer, scrolling, permanent button or browser helper.

The author is **Captain Cool - Suspicious Sausage Records** and website is
**https://github.com/gurppt/tack**. Contact remains unset, license “Undecided”,
and copyright is omitted. Cargo's version **0.0.1** is authoritative; an explicit
metadata version must match it. Invalid schema, controls, excessive field length
or text beyond the narrow layout fail preparation explicitly.

## Package and cost

| Item | Result |
| --- | --- |
| Editable source PNG | 587×635, 146,187 bytes |
| Prepared lossless PNG | 207×224, 38,426 bytes |
| Decoded RGBA while requested | 185,472 bytes, transient worker result |
| GPU image + six vertices while open | 185,592 payload bytes, excluding driver/staging |
| GPU payload after dismissal | 0 |
| New Rust dependencies | 0; reuse existing PNG decoder |
| Stripped executable increase | 22,464 bytes, 0.125% |
| Release executable increase with reduced debug | 134,072 bytes |
| Total stripped package increase including PNG | 60,890 bytes |

Baseline stripped executable: 17,931,688 bytes. Current: 17,954,152 bytes.
[Binary audit](../benchmarks/phase1k-about/binary-audit.json) verifies `.text`
identity between the tested stripped executable and ordinary release build.
Ordinary release SHA256:
`6fa6623ce3a7977323e692442894c84f57220c8c42852b2b97b4a3735d3c665e`.
The full source and compact PNG are absent from that executable.

The embedded prototype was rejected after measuring 45,056 resident bytes in
its asset-intersecting pages before About opened. The shipped small sidecar
`tack-about.png` is read only on explicit opening. A syscall trace observed
**0 / 1 / 1** asset opens before opening / while open / after dismissal, and no
full-source access. No About image decode, texture or worker is requested at ordinary
startup. Fixed menu/metadata strings, a request counter and empty image-option
checks remain; “free while unused” does not mean zero additional code bytes.

Cargo prepares the package automatically using Python/Pillow at build time.
The repository-local build helper copies both executable and compact PNG and
records both hashes. Runtime has no Python, Pillow or TOML requirement. Keep
`tack-about.png` beside an executable when moving it. Missing artwork is a
recoverable information-panel fallback, not an application launch failure.

An explicit release decoder probe measured first decode **0.598 ms**, warm p50
**0.405 ms**, p99 **0.467 ms**, 50 warm samples. This is CPU PNG decode only;
it excludes file access, worker scheduling and GPU upload. Bounded encoded reads,
256×256 dimension limits and a 1 MiB decoder scratch limit are checked.

## Native and automated verification

Local formatting, all-target/all-feature check, warnings-denied Clippy,
workspace tests, docs and cargo-deny passed: **168 Rust tests**, **13 Python
checks**, plus **9 explicit GPU tests** (including seven otherwise ignored).
The new GPU test verifies nearest red/blue texels above an overlay at both
scales, 20 upload/clear cycles and zero owned payload after clearing.

**41 native About assertions** pass on an owned X11 display with RTX 2060/Vulkan:
all three themes at 800×600 with 1×/2× scale; both menu routes; Escape, Enter and
pointer Close; 40 additional open/close cycles; zero final texture payload and
no resident local worker. Readable text stays inside the viewport, including
all author words and the whole URL. The image preserves aspect within rounding.

[1× capture](../benchmarks/phase1k-about/about-800-1x.png) ·
[2× capture](../benchmarks/phase1k-about/about-800-2x.png) ·
[missing artwork](../benchmarks/phase1k-about/missing-artwork.png).
At 2×, the native artwork comparison matches the nearest reference at >99.5%
within one channel unit; boundary sampling ties account for remaining pixels.
Independent GPU color tests verify that interpolation is not introduced.

A disposable copy of the package was tested with missing, corrupt and oversized
PNG data. In every case, metadata and Close remained usable, and subsequent
canvas grid interaction worked. The source, ordinary package and board were
untouched. **27 native clipboard/menu/theme regression checks** also pass on the
final packaged build.

Forty extra cycles showed settled RSS 329,232,384 → 332,091,392 bytes across four
batches, a 2.73 MiB range, with no local worker and zero owned GPU payload at the
end. This checks bounded behavior and owned-resource release; it does not claim
that GPU-driver caches or process RSS return byte-for-byte to their initial state.

## Ordinary idle and paired performance

[Idle observations](../benchmarks/phase1k-about/idle.json) cover unused, open and
closed About with grid hidden/visible, plus the preceding executable. Every
five-second interval has **zero redraws, zero GPU submissions, zero read/write
I/O and stable RSS**. CPU ticks are 2–3 per interval (about 0.4–0.6% of one core,
including native-driver threads), comparable with the baseline's two ticks.
No ordinary application timer or polling loop was added. Whole-process RSS is
about 315–316 MiB on this driver; this is not a 128 MiB runtime claim.

24 serialized native paired runs use the same empty board, three viewport sizes,
grid hidden/visible and reversed run order. Startup frames are excluded from
render timing; every run has at least 30 frames and GPU timestamp samples.
Numbers below combine the two orders, p50/p99 in milliseconds.

| Viewport / grid | Baseline CPU | Current CPU | Baseline GPU pass | Current GPU pass |
| --- | --- | --- | --- | --- |
| 800×600 / hidden | 0.150 / 0.240 | 0.157 / 0.234 | 0.012 / 0.035 | 0.012 / 0.035 |
| 800×600 / visible | 0.176 / 0.239 | 0.163 / 0.242 | 0.012 / 0.013 | 0.012 / 0.013 |
| 1024×768 / hidden | 0.239 / 0.299 | 0.254 / 0.309 | 0.017 / 0.050 | 0.017 / 0.051 |
| 1024×768 / visible | 0.247 / 0.306 | 0.247 / 0.302 | 0.017 / 0.051 | 0.017 / 0.051 |
| 1600×900 / hidden | 0.153 / 0.193 | 0.154 / 0.196 | 0.029 / 0.081 | 0.028 / 0.079 |
| 1600×900 / visible | 0.153 / 0.219 | 0.151 / 0.210 | 0.029 / 0.081 | 0.029 / 0.092 |

First-frame medians are 431.1 ms baseline and 422.6 ms current, with overlapping
415.7–459.9 / 410.6–445.7 ms ranges. There is no measured material launch/render
regression or extra GPU pass; these results do not establish a speed improvement.
CPU work remains distinct from GPU timestamps and presentation.

## Evidence, limitations and disk

Full raw evidence stays in ignored `benchmark-results/phase1k/`, with hashes in
the published receipt. Generated evidence occupies roughly 44 MiB, below the
512 MiB phase budget. One stripped baseline and one current executable are
reused; fault-test copies are removed immediately. Shared Rust/native targets
are retained; disk has about 47 GiB free, above the 10 GiB reserve. User artwork,
fonts, unrequested SVGs and briefs are preserved.

A preliminary native pixel expectation incorrectly resampled twice at 2×;
the probe was corrected to direct physical nearest sampling, then all cases
reran successfully. One paired-run attempt hit an isolated Xvfb reset between
clients; the owned server restarted with `-noreset` and the complete 24-run
series passed. These were harness failures, with their logs retained.

Independent review and owner aesthetic/comfort acceptance remain pending.
Linux/X11 native validation does not substitute for a real Windows or Wayland
session. The exact implementation commit passed [Quality CI](https://github.com/gurppt/tack/actions/runs/37581135771):
Linux, Windows and dependency jobs. [CI receipt](../benchmarks/phase1k-about/ci-source.json)
records the full SHA. Windows unit/build coverage includes packaged-image worker
loads, but no interactive Windows desktop claim is made.
