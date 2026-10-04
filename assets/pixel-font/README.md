# Tack label bitmap data

Derived from GNU Unifont 18.0.01 (2026-09-16), BMP and upper-plane HEX files.
Authors: Roman Czyborra, Paul Hardy and the GNU Unifont contributors, including
individual glyph authors acknowledged in the upstream release.

These bitmap assets, including `tack-label-glyphs.bin`, are distributed under
the **SIL Open Font License 1.1**, reproduced in `OFL-1.1.txt`. Upstream also
offers GPL-2.0-or-later with a font embedding exception (`LICENSE.txt`). The
application code has its own license; these assets retain their font license.
The converted font is named **Tack Label Bitmap**, not GNU Unifont.

[Upstream release and license](https://unifoundry.com/unifont/index.html).
`provenance.json` records pinned source URLs, SHA-256 and conversion format.
Original gzipped HEX sources are retained for exact reproduction and attribution.
No runtime font engine, system font lookup, atlas texture or external file read.

Rows are rendered with integer sampling. Latin, Greek, Cyrillic, Han, kana and
other supported Unicode glyphs remain readable. Complex shaping/bidi is not
implemented by this small bitmap path; the full selected/editing UTF-8 name is
also shown in the native window title, where the platform handles text shaping.
Unknown codepoints use U+FFFD rather than losing document text. Label truncation
is by scalar boundary; full names remain available on selection and F2.
