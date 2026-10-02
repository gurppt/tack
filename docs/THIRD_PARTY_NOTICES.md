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
