# Tack Note Mono

Compact, renamed OFL-1.1 derivative of Liberation Mono Regular 2.1.5. The pinned
original compressed TTF is build provenance only and is not embedded in Tack.
The runtime embeds `atlas.png` (416,042 bytes) and `slots.bin` (2,560 bytes), once.
1197 glyphs cover supported scalars 32..1279 except control/missing scalars.
The 1024×1824 R8 distance atlas occupies 1,867,776 GPU texture bytes after first
visible primary note glyph; no glyph eviction, service or runtime font parser.
Other scalars use the existing Tack Label Bitmap fallback; complex shaping,
bidi and colored emoji are unsupported. Exact UTF-8 remains durable.

`COPYRIGHT.txt` preserves upstream copyrights/reserved names; `OFL-1.1.txt` is the
complete license. `provenance.json` pins the source and derivative SHA-256 values,
Debian package/source version, generation libraries and FreeType version.

Reproduce in an isolated build-only Python environment with Pillow 10.2.0,
NumPy 1.26.4, SciPy 1.12.0 and fontTools 4.55.3 (FreeType 2.13.2):

```bash
python tools/build_note_font.py
```

No Python build package is an application/runtime dependency. The generator
verifies pinned input and resulting atlas/slots hashes. There is no RLE experiment,
full extra Unicode font, system-font discovery or runtime shaping dependency.
