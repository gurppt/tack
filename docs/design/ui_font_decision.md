# Bitmap font and pixel presentation (Phase 1F)

Tack uses the existing **Spleen 8×16**, with the existing Unifont bitmap data as
fallback. The same glyph lookup serves UI labels, frame names and notes. There
is no custom font, runtime discovery, rasterizer, SDF atlas or font service.

| Candidate | Fit and coverage | Distribution decision |
|---|---|---|
| Supplied Espy Sans 9 / Bold 10 YAFF | Compact classic Mac appearance; MacRoman descriptors, limited script coverage | Rights are not established by the supplied files; evaluated locally, not copied into shipping assets |
| Amiga Workbench / Geneva families | Appropriate period appearance; encoding and size depend on the particular source | No verified distributable source/license selected; would require a separately pinned rights review |
| Spleen 8×16 | Existing workstation bitmap, 1001 Unicode glyphs, including Latin accents and selected Greek/Cyrillic/symbols | Selected; explicit BSD-2-Clause license and pinned upstream |
| GNU Unifont | Existing broad Unicode fallback, 8/16×16 cells; visually wider for many scripts | Retain existing OFL-1.1 derivative, renamed Tack Label Bitmap |

Spleen is pinned to upstream commit
`57f9219328c9f5873085320fe8bc8f7dd34b8791`, version 2.2.0.
[Official upstream](https://github.com/fcambus/spleen) and its BSD notice are
recorded in `assets/ui-font/provenance.json` and `LICENSE`. The BDF is 154114 B;
its SHA-256 is `b38b32a66920068965a3101f98071d310c5c74659fe86e55d346140770f8f6e8`.
`tools/build_ui_font.py` deterministically packs 1001 fixed 37-byte records:
**37037 B**, SHA-256
`6c083bb6db7086dbb167e50bafb3b93f5eca3077e8384af6f0b7a20602f3f837`.
The upstream 8×16 pixels are preserved; no glyph outline has been designed.

The existing [Unifont provenance](../../assets/pixel-font/README.md) pins
18.0.01 and its original sources/attribution. Its 115053-record table is
4256961 B. Both tables are immutable binary sections: binary search touches
requested records; there is no eager heap copy, complete glyph texture or growing
Unicode cache. Unknown scalars draw U+FFFD without changing durable UTF-8.
Primary cells advance eight pixels and fallback cells eight or sixteen. Tabs,
scalar wrapping and box clipping reuse the existing note layout.

At automatic DPI, presentation rounds to a whole logical pixel scale (1..8);
the preference offers system/1/2/3/4. All sixteen glyph rows survive fractional
DPI. UI origins are snapped, and common annotation/overlay shaders evaluate
binary coverage at integer logical pixel centers. Rotated rectangles, segments,
arrows, scribbles and bitmap glyphs have hard edges. Requested opacity still
blends normally; it is independent of edge antialiasing. User images retain
Smooth/Nearest sampling. Geometry/picking remain in the existing world model.

Font VRAM is **zero**: requested bitmap rows travel in bounded primitive/overlay
packets. The removed note atlas had a 1,867,776 B texture and 416042 B compressed
payload; these assets, its direct renderer image dependency and build script are
removed. Read-only tables add no allocator-backed idle glyph inventory.
Annotation instances remain capped at 32768×128 B; the active pixel-grid uniform
adds sixteen bytes per pipeline. Actual binary/idle measurements are in the
[mission report](../MISSION_1F_REPORT.md).

Complex shaping, RTL/bidi, ligatures, color emoji and full localization are not
claimed. A future shaping boundary can produce glyph placements before the
common packet builder, without requiring every idle instance to pay for a font
engine. The current font licenses are distributable with their notices; unused
Mac/Amiga candidates remain outside the repository assets. Native captures
verify raster behavior and readability at tested sizes, not artist preference.
