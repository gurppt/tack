# Bounded JPEG decoder boundary

Phase 2A1 uses the already pinned, unmodified libjpeg-turbo 3.2.0 `djpeg`
executable for baseline JPEG scanlines and cropped detail. The existing
`turbojpeg = 1.5.1` safe wrapper exposes reduced DCT, header metadata and a scan
limit, but not crop/skip/scanline output or a hard allocation control. Extending
that wrapper would require a new FFI surface. Tack adds no unsafe Rust, C shim,
decoder crate or service: an admitted image worker starts one short-lived
packaged helper when a bounded representation is absent.

The helper is shipped beside the client and resolved from `current_exe()`;
no PATH lookup, terminal, baked build path or system JPEG installation is used.
The explicit native preparation script verifies the upstream archive hash and
records the helper hash. Cargo only copies that prepared executable into build
and test directories. Missing packaging becomes an image-supply error.
The server does not depend on this image crate or invoke the helper.

## Header and scratch admission

The Rust parser captures at most 1 MiB of header bytes through the first SOS.
It validates dimensions, component IDs, quantization/scan-table indices,
sampling factors and checked arithmetic before starting the child. The exact
captured prefix is replayed into the child's stdin, followed by the remaining
source stream. A linked file changing its header between validation and native
use cannot introduce an unvalidated progressive frame through a second read.
Existing linked fingerprints and source revisions govern result/cache admission.

The large route requires SOF0, eight-bit precision, one or three components,
and a first baseline scan containing every component exactly once. Progressive
and separate-component sequential scans are refused explicitly. This last
check matters: libjpeg-turbo's `jdmaster.c` selects a whole-image coefficient
buffer for **multiple scans**, not only progressive JPEG. A reduced output or
`-maxmemory` alone would not prove bounded caller/native memory.

Sampling factors must be 1 through 4 and sum(horizontal × vertical) at most
10 blocks per MCU. The codec's own axis limit is 65,500. The conservative
native baseline scratch envelope is `width × 192 + 1 MiB`, checked against
16 MiB; it includes full-width component iMCU/context sample rows, upsampling,
PNM row and fixed tables. It does not depend on height. The accepted baseline
path uses the single-MCU coefficient branch in `jdcoefct.c`, not a full image
of coefficient blocks. This is a source-derived bound, not an allocation trace
or a claim that process RSS equals decoder scratch.

Ordinary progressive or multiscan images retain a separate explicit allowance:
`round_up(width, 96) × round_up(height, 96) × 6 + scanline_envelope ≤ 192 MiB`.
The 96-pixel rounding covers DCT block/sample alignments 8, 16, 24 and 32.
The ordinary safe wrapper independently caps its RGB output at 32 MiB and
encoded input at 64 MiB. Large progressive sources do not silently enter that
ordinary path.

## Representation, cancellation and reuse

An overview requests native reduced DCT at 1/8, 1/4, 1/2 or 1, then samples
streamed RGB8 scanlines into at most 2048 × 2048 RGBA pixels. There is no full
decoded RGB/RGBA image allocation. Input is streamed with a 64 KiB feeder;
large JPEG encoded work is capped at 256 MiB. Normal UI import/relink retains
its separate 64 MiB encoded-file admission; the worker cap does not expand that
UI limit. A native output row is at most
196,500 bytes. The pinned P6 header, dimensions, row count and EOF are checked.
The helper runs with `-strict -maxscans 1 -maxmemory 16384 -rgb -pnm`.
The memory parameter is additional protection for virtual arrays, not a hard
bound on all native allocations.

Deep detail reuses the existing 256-pixel tile/mip addresses. Crop/skip avoids
unnecessary IDCT/output rows, but every uncached region still traverses the
**sequential JPEG entropy stream from the beginning**. Restart markers can
affect skip cost; they do not turn this interface into arbitrary random access.
The native DCT scale stops at 1/8. Coarser mip addresses sample with an explicit
integer stride, including partial edge tiles; proportional resize is used only
for whole-source overviews.

Each JPEG tile adds one mip-pixel gutter on all four sides, at most 258² RGBA
pixels. Native crop starts align to 96 pixels so libjpeg's MCU expansion cannot
silently move the left edge. Sampling accounts for that offset. Adjacent tile
gutters contain their neighbor's samples; only physical mip boundaries repeat
the outer pixel. The client draws the interior UV rectangle so Smooth filtering
can sample neighboring content across tile borders. PNG prototype tiles retain
their separate opt-in behavior.

Phase 2A2 additionally admits a rectangular batch of neighboring tile addresses
at one mip, in any request order. `admissible_tiles` shares the decoder's pure
planner with the worker scheduler. Empty batches, duplicate addresses, different
mips, gaps, out-of-source tiles and excessive crops are refused before helper
startup. One batch invokes the existing helper once and traverses the source
entropy stream once; each native RGB scanline is scattered directly into the
requested guttered RGBA tiles. It does not materialize the rectangular crop.

The batch ceiling is 15 tiles, aggregate guttered RGBA at most 4 MiB, and native
rectangular crop footprint at most 4 MiB charged at four bytes per pixel. Sixteen
full 258² outputs would exceed that aggregate ceiling. At mips coarser than the
native 1/8 DCT scale, integer sample strides increase the crop footprint and can
reduce the admissible batch further. The scheduler can retain existing single
requests when grouping is inadmissible. The single-tile and overview routes
preserve their existing bounds and share the parser, scanline consumer and child
lifecycle with the regional route. Neither route adds random entropy access.

The main worker consumes stdout while one scoped feeder writes stdin. One
scoped watcher kills the child on cancellation or a 30-second active-operation
deadline, including when stdout is blocked before any pixels arrive. Errors
kill/reap the child; scoped threads finish before the worker returns. There
are at most two existing image workers, hence at most two decoder children,
four support threads and two bounded job outputs. A regional job
owns at most 4 MiB of aggregate tiles; an overview retains its 16 MiB output cap. Settled boards have no helper,
watcher, JPEG timer or new background service.

Overview outputs retain the existing per-open derived PNG cache. Native tile
outputs additionally use a bounded profile-owned raw cache, with checked
source/revision/path/fingerprint/tile/generator identity. Cache hits bypass
the codec; unavailable or corrupt cache falls back to the source. See
[the huge-image design](huge_images.md) for quotas and authority checks.
Tiles are disposable presentation products and never become canonical board
content. Save/reopen preserves original JPEG bytes or linked descriptors. A
source change or relink invalidates detail capability and old revisions.

## Chosen scope

A complete sequential tile-pyramid preparation pass would remove repeated
entropy scans, but adds cache construction, write amplification, cancellation
and fairness responsibilities for data the current view may never use. The
cropped scanline route, bounded neighboring batches and a small visited-region
cache avoid that preparation. The measured real 50k × 50k baseline fixture is
33,425,392 encoded bytes: twelve separate bottom-right tiles take 7.623 s and
395,579,232 logical source bytes; one twelve-tile rectangle takes 0.629 s and
33,161,544 bytes. Three bands take 1.875 s and 98,894,808 bytes. These are
single release-provider observations on the measured host, with exact RGBA
comparison and independent source-coordinate witnesses; they are not SSD
cold-cache or universal throughput guarantees.

Actual native opposite-corner refinement takes 0.767 s/default and
1.997 s/potato on the constrained modern host. Reopening the last visited
six-tile region takes 30/73 ms, with six checked raw hits and no detail source
reads or codec invocation. Overview reconstruction is measured separately.
The overview remains drawable while cold detail arrives. This implementation
does not claim a whole-source or filtered mip pyramid, random entropy access,
or legacy-CPU equivalence. The profile cache survives normal close;
other per-open products are retired as before. Full receipts and limitations
are in the [Phase 2A2 report](../MISSION_2A2_HUGE_RASTER_STREAMING_REPORT.md).

The exact upstream source inspection is reproducible under
`target/native/source/libjpeg-turbo-3.2.0/src/`: `jdmaster.c`, `jdcoefct.c`,
`jdmainct.c`, `jmemmgr.c`, `jmemnobs.c`, `djpeg.c` and `wrppm.c`. Upstream
documentation and source are in the
[pinned release](https://github.com/libjpeg-turbo/libjpeg-turbo/tree/3.2.0).
