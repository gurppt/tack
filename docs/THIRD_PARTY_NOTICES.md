# Native thumbnail dependency notices

This software is based in part on the work of the Independent JPEG Group.

The prototype statically links unmodified libjpeg-turbo 3.2.0, through
turbojpeg 1.5.1 and turbojpeg-sys 1.2.0. The Rust wrapper/binding licenses are
MIT OR Unlicense; this project relies on the MIT alternative. The wrapper's
[MIT notice](third_party/turbojpeg-MIT.txt) is retained.

The bounded huge-JPEG worker also launches the unmodified, statically linked
`djpeg` tool from that same pinned source, packaged as `tack-jpeg-decoder`
beside the application. The same upstream notices apply to this executable.

libjpeg-turbo includes IJG, BSD and zlib terms. Its upstream
[license and copyright notices](third_party/libjpeg-turbo/LICENSE.md) and
[IJG README/license](third_party/libjpeg-turbo/README.ijg) are reproduced
without modification. Binary redistribution must include these notices and
the IJG acknowledgment above. The preparation script also retains them beside
the local static library. Cargo-deny checks Rust package metadata; it does not
replace review of the separately compiled native library.

These notices cover these dependencies, not Tack's pending code/license model.
Other Rust dependencies retain their respective package licenses.

## Tack Label Bitmap

Bundled bitmap data derives from GNU Unifont 18.0.01 BMP and upper-plane HEX
files, distributed under SIL Open Font License 1.1. The converted font is named
Tack Label Bitmap. Copyright, full license, original notices, pinned sources,
SHA-256 provenance and reproduction instructions are in
[assets/pixel-font](../assets/pixel-font/README.md).
`tack-app --font-license` displays the embedded copyright and OFL notice.
No system font engine or new Rust dependency was added.

## Spleen bitmap primary

Bundled Spleen 8×16 bitmap cells are the unmodified glyph pixels of Spleen 2.2.0,
pinned to upstream commit `57f9219328c9f5873085320fe8bc8f7dd34b8791`, under
BSD-2-Clause. The upstream notice/license, BDF, packed derivative SHA-256 and
reproduction script are recorded in [assets/ui-font](../assets/ui-font/provenance.json)
and [font decision](design/ui_font_decision.md). `tack-app --font-license`
prints both bundled font notices. The earlier Liberation/SDF note atlas has been
removed; no Mac/Amiga font with unresolved redistribution rights is bundled.
