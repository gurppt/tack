# Giant originals — first blockers and a bounded follow-up

Base: `dev 7f19e8c`. Stage D validates real >64 MiB baseline JPEG/PNG through
import, render, save and reopen. It deliberately retains cumulative streaming
read budgets of 256 MiB, including header/layout/rewind overhead. Approximate
1.33/1.478 decimal GB inputs remain rejected at encoded admission; not a solved
Windows LOD issue. Actual Photoshop sample headers/bytes have not been supplied.

## Progressive JPEG feasibility witness

Existing scanline path requires 8-bit, single-scan baseline RGB/gray. Progressive
scans require coefficient storage; requesting a 1/8 output does not eliminate
that need. An ordinary bounded progressive path remains available within its
existing 192 MiB coefficient budget; giant streamed admission is different.

An owned valid progressive RGB JPEG, 2560×2048, deterministic seed 26101321,
quality 95, was generated and decoded with the pinned bundled libjpeg-turbo 3.2.0
helper. Private properties/SHA are in
`benchmark-results/p0-win-image/stage-d/progressive-experiment/results.json`.
Helper SHA: `4fcef9b973c80de96ecf2d5babe86ef87b1be2459f22559243c8e85da3ebe587`.

`djpeg -scale 1/8 -maxmemory 8192 -outfile /dev/null` rejects immediately with
`Memory limit exceeded`, ~2068 KiB process peak RSS. At `-maxmemory 65536`, the
same image succeeds in ~0.16 s, ~17580 KiB peak RSS. These are one-sample native
feasibility witnesses, not 1.5 GB support or private/GPU memory measurements.
Local pinned `src/jmemnobs.c` implements backing-store open as an error; current
build is not a transparently disk-backed progressive decoder. Simply changing a
memory argument is not an implementation of bounded coefficient paging.

## Follow-up: GIANT-LOCAL-SUPPLY-01

First collect representative original Photoshop files privately, recording exact
bytes, JPEG marker/scans/precision/components, PNG bit depth/color/interlace and
dimensions. Keep original ownership and separate format-specific failure stages.
One file alone may fit today's Embedded 2 GiB batch, while 1.33 +1.478 GB together
does not; independent limits must remain explicit. Online 512 MiB cache and upload
protocol are not part of the local solution.

For baseline JPEG and static noninterlaced PNG, measure work/time as well as
memory before reviewing larger streaming read allowances. Repeated sequential
PNG passes for 128/512/2048 demand are potentially O(source bytes) each. Consider
one bounded derived overview or demand-aligned tile spool, with source snapshot,
revision, cancellation, disk quota and eviction integrated into current workers.
Do not claim compressed arbitrary ranges provide random access. Do not eagerly
decode or generate a pyramid for every image on board open. No unconditional cap
increase is authorized by this note.

Progressive alternatives to compare in a separate native-codec experiment:

1. A minimal audited disk-backed coefficient manager in the helper, with small
   resident row windows, quota/time/read/write caps, private per-job ownership,
   deterministic cleanup and cancellation. This likely requires a pinned codec
   build variation and additional native review, not a Rust constant change.
2. A maintained decoder providing a demonstrably bounded progressive strategy;
   compare exact binary/dependency cost, malformed-input safety and coefficient
   storage. Do not introduce an eager generic media library.
3. Continue an accurate unsupported response for giant progressive inputs while
   the owner supplies a baseline JPEG/PNG equivalent. Conversion must be explicit
   and must never silently replace the original.

Prototype on small progressive witnesses first, then 8000/20000-axis corpus and
the actual ≥1 GB encoded sources. Measure first preview, detail/tiles, repeated
zoom, peak PSS/private commit, scratch, logical/physical I/O, maximum callback
stall, cancelled jobs, source mutation, disk-full and settled idle. Compare 20
ordinary sources at 800×600 and normal desktop size on the same driver; preserve
ordinary dispatch without new processes/reservations/timers. Physical Win10 JPEG
and PNG acceptance, static SIMD packaging and storage-generation strategy from
Stage F are separate release gates. No `.tack` migration, codec fork, new daemon
or network architecture is implemented by this report.
