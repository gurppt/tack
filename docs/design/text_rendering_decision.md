# Phase 1E text rendering decision

Decision before implementation, to be checked by native reading and measurement:
use a compact precomputed signed-distance glyph atlas for simple plain notes,
with the existing Unicode bitmap table as fallback. No runtime font discovery,
network/font service, shaping framework or additional Rust dependency.

The existing 1D 8/16×16 bitmap table is deterministic and useful for short UI
labels. Scaling those pixels alone makes paragraphs coarse at large sizes. It
does not automatically become the note font. UI continues its integer bitmap
path; user note glyphs use smooth distance coverage, as do geometric annotations.

Evaluate a subset of OFL-1.1 Liberation Mono, renamed Tack Note Mono, covering
basic Latin, Latin accents, Greek and Cyrillic. Store the pinned original only
as build provenance; ship a compressed derivative atlas, lazily expanded/uploaded
on first visible note. Fixed metrics allow deterministic scalar wrapping without
a font runtime or glyph-cache growth. Other Unicode scalars preserve exact UTF-8
and use the existing Unifont-derived fallback. Complex shaping/bidi/ligatures and
color emoji are explicitly unsupported; this is a plain annotation font.

Alternatives considered: reusing bitmap alone has no binary cost but weaker long
note legibility; a lazy system font adds discovery/I/O and nondeterministic metrics;
Swash/Fontations offers stronger typography but adds runtime/transitive code and
raster/cache work that this small slice can avoid. A second complete Unicode font
would compound the 4.26 MB table cost. A small static subset is measurable and
removable if native reading proves poor. Distance fields have finite contour
resolution and do not claim desktop publishing quality at extreme zoom.

Primary sources reviewed 2026-10-04:
- [Liberation Fonts upstream and OFL license](https://github.com/liberationfonts/liberation-fonts).
- [Swash scaling documentation](https://docs.rs/swash/latest/swash/scale/).
- [PureRef notes](https://www.pureref.com/handbook/features/): editable transformed
  notes; rich text is deliberately outside Tack 1E.

Actual atlas/binary/RSS/VRAM/layout/draw measurements, reproducibility hashes and
native readability observations are required before the final mission verdict.

## Measured implementation and final decision

The evaluated atlas was retained. Tack Note Mono is a renamed OFL derivative of
Liberation Mono Regular 2.1.5, pinned with source/license/build provenance in
`assets/note-font`. It ships 1197 glyphs, a 416,042-byte PNG and 2,560-byte slot
table, embedded once each; the compressed original TTF is build provenance only.
The rejected RLE experiment is removed. No additional Rust package/version or
font parser was added; the renderer directly reuses the existing image decoder.

The 1024×1824 atlas is R8, lazily decoded and uploaded when a primary note glyph
first becomes visible: 1,867,776 GPU bytes, fixed thereafter. Layout uses a
.6015625-em monospace advance; tabs occupy four advances. Unsupported primary
scalars use the existing 16-pixel Unicode bitmap path. UTF-8 authority survives
exactly even where rendering has fallback or missing/complex-glyph limitations.
The retained old 1D table remains in the binary; this phase does not remove it.

Matched release binaries with `strip --strip-debug`: 1D 19,919,264 bytes; 1E
20,591,112 bytes; increase 671,848 bytes (0.641 MiB, 3.37%). Fully stripped copies:
16,827,336 to 17,460,688 bytes (+633,352). Copies were stripped for comparison;
the measured frozen binary was not modified.

On RTX 2060/Vulkan/X11, 100 notes use 1390 note glyph primitives (including bitmap fallback): warm layout
p99 .151 ms, whole CPU frame .462 ms, GPU canvas .063 ms. 1000 notes use 14,890
note glyph primitives (including bitmap fallback): layout p99 .719 ms, whole CPU frame 1.617 ms, GPU .109 ms. First CPU
frames were 18.293 and 14.256 ms, including pipeline/atlas decode/upload/setup;
these are not separately isolated decoder timings. 5000 notes were tested for
metadata/save/reopen/query/layout, with 1086 visible boxes and 16,298 glyphs after
culling; there is no claim that 5000 full notes were GPU-rendered simultaneously.
GPU instance buffers are 0.25 MiB / 2 MiB for 100 / 1000 notes, separately from
the fixed 1.781 MiB atlas. No incremental glyph-cache growth occurs.

Native X11 and DPI2 captures were inspected for smooth readable note content and
crisp bitmap UI, including edit/resize/rotation/zoom/save/reopen. The static font
is adequate for these bounded notes; artist readability/tablet feel and complex
scripts remain human evaluation work. At extreme zoom the finite SDF resolution
is an explicit quality limit, not a claim of full typography support.

The generator was rerun in an isolated build environment and reproduced exact
hashes: atlas `8cd3481367ec144ba03de3d038894ea705c2a27b384e6c780d4b8e760eea6577`,
slots `06b73543390a353a75daaabc51ca0aabf1c1038dfa52ad5b97fc417b4f1bf53b`.
[Asset instructions](../../assets/note-font/README.md) pin generator libraries and
FreeType; none is a runtime dependency. Controlled idle/RSS evidence and the
complete performance/provenance tables are in [Mission 1E](../MISSION_1E_REPORT.md).
