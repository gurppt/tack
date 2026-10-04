# Native thumbnail dependency notices

This software is based in part on the work of the Independent JPEG Group.

The prototype statically links unmodified libjpeg-turbo 3.2.0, through
turbojpeg 1.5.1 and turbojpeg-sys 1.2.0. The Rust wrapper/binding licenses are
MIT OR Unlicense; this project relies on the MIT alternative. The wrapper's
[MIT notice](third_party/turbojpeg-MIT.txt) is retained.

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

## Tack Note Mono

Bundled signed-distance atlas derives from Liberation Mono Regular 2.1.5 under
SIL Open Font License 1.1, renamed Tack Note Mono. Upstream Google/Red Hat notices,
reserved names, full OFL, pinned original and derivative SHA-256 provenance are in
[assets/note-font](../assets/note-font/README.md). The original TTF is only a
reproduction asset. The binary includes the compact atlas/slot map and prints the
additional copyright notice through `tack-app --font-license`.
