# Mission 0.6 native thumbnail decoder decision

The selected runtime path is `NativeThumbnail`: turbojpeg **1.5.1**,
turbojpeg-sys **1.2.0**, unmodified libjpeg-turbo **3.2.0**, statically linked
with mandatory SIMD. It replaces JPEG source decoding for **128-pixel thumbnails
only**. Medium/detail retain jpeg-decoder 0.3.2 without Rayon. No fallback,
decoder feature switch, prefetch change or renderer optimization was introduced.
The isolated microbenchmark retains a reference implementation solely to reproduce
the measurement; ordinary asset requests cannot select it for thumbnails.

## Maintenance, native build and license

[The established Rust wrapper](https://github.com/honzasp/rust-turbojpeg)
provides a safe, synchronous slice API and owns the native handle. Its bundled
native source is 3.1.0; we deliberately use the official
[3.2.0 release](https://github.com/libjpeg-turbo/libjpeg-turbo/releases/tag/3.2.0),
published 2026-06-30 and checked as the latest release on 2026-10-02.
The [upstream security policy](https://github.com/libjpeg-turbo/libjpeg-turbo/security)
and [change log](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/ChangeLog.md)
were reviewed. The development branch's further header-error hardening is another
reason to destroy each handle after error; it does not justify calling a future
release installed or tested. Native code remains a dependency attack surface.

`tools/prepare_turbojpeg.py` downloads the official release archive, verifies
SHA256 `6f30092cef9fb839779646608f4ee14ae3cbac989c47fa05e841b0841f09878e`,
builds Release/static/SIMD with CMake, and installs only under
`target/native/libjpeg-turbo-3.2.0`. Cargo's explicit local-library configuration
uses no bundled-source build, bindgen or automatic native download. Runtime has
no TurboJPEG DLL requirement. NASM 2.16.01 was extracted from a downloaded Debian
package under `/tmp` for this machine; no privileged system changes were made.
The local static library SHA256 is recorded in the benchmark environment.

Prerequisites are Python 3.12+, CMake, a C compiler and NASM on PATH. Windows/MSVC
needs the compiler environment and matching architecture. The script requests
the DLL C runtime and copies `turbojpeg-static.lib` to the `turbojpeg.lib` name
expected by turbojpeg-sys's explicit lookup. Windows CI prepares native source
before Cargo, but **Windows and remote CI were not executed in this mission**.
Static linking simplifies runtime deployment but adds native preparation, ABI
and update responsibilities. The existing Linux libXi RUNPATH workaround is
separate; locally frozen benchmark executables are evidence, not portable packages.

Wrapper/binding are MIT OR Unlicense; MIT is suitable for the configured policy.
The native library carries IJG/BSD/zlib terms, separately reviewed because
cargo-deny does not audit this external C archive. Full upstream notices and the
required IJG attribution are in [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
This does not decide Tack's pending license/contribution model. Existing
jpeg-decoder maintenance-mode risk remains for medium/detail.

## Safety and ownership invariants

1. Tack contains no unsafe block. FFI is isolated in the established wrapper
   and its sys crate. The wrapper's private `Handle` initializes/destroys the
   TurboJPEG handle; `NativeThumbnail::new` creates one per input and immediately
   propagates any error. No handle is reused after a fatal header error.
2. Encoded input is borrowed immutably for the job lifetime. Header and decode
   necessarily see the same slice. Native calls are synchronous, and no pointer
   escapes. Input remains live until native decode and handle destruction finish.
3. Input is nonempty and ≤64 MiB. The ordinary worker also uses a capped read
   with one sentinel byte. Width/height must be nonzero and ≤6000×4500 before
   pixel allocation. Accepted color spaces are RGB, YCbCr and grayscale; lossless,
   arithmetic and CMYK are recoverable errors.
4. Scaling uses only 1/8, 1/4, 1/2 and 1, selecting the smallest with at least
   one axis ≥128, or full resolution for smaller files. This matches the old
   power-of-two selection. Dimensions are bounded before the wrapper's scale
   arithmetic. Pitch = width×3 and total = pitch×height use checked arithmetic;
   zero, overflow or total >2 MiB is rejected.
5. `try_reserve_exact` reports allocation failure; the Vec is initialized to
   the exact checked size. Those invariants satisfy the wrapper's internal
   `Image::assert_valid` products and prevent its assertions from firing for
   external image data. The wrapper rechecks output dimensions before FFI.
6. Native output is written directly into this Rust RGB Vec, transferred into
   RgbImage without copying, then passed through the existing thumbnail/RGBA
   conversion. There is no full original RGB allocation for the thumbnail tier.
7. Worker cancellation is checked before header, before native decode and after
   it, then at the unchanged resize/encode/write/result boundaries. Native
   decoding itself remains nonpreemptible; a stale job is discarded safely.
8. Codec errors propagate through AssetError and the ordinary failed-key path.
   There is no implicit retry via the slower decoder. Rust/native resources are
   dropped on every error and completion.

## Limits which must not be hidden

The safe wrapper exposes a progressive scan limit (100), but not MAXMEMORY,
MAXPIXELS or STOPONWARNING. The checked output limit is **not a hard native
scratch/RSS quota**. Progressive coefficient storage depends on full source
dimensions and sampling; a conservative 3-component 16-bit coefficient estimate
at 6000×4500 is about 162 MB plus rounded block edges and auxiliary storage.
ICC/APP2 marker data may be copied during header parsing before dimension
validation; its payload is bounded by the encoded-source cap. Both workers can
hold independent scratch. Source Vec capacity/allocator retention can exceed
the profile's encoded payload length. No claim of zero native copying or of a
universal 2-MiB decoder footprint is made.

Without STOPONWARNING, corrupt data can consume a full decode before returning
Err; warnings are not silently accepted. Precision is absent from the wrapper's
header, so 12-bit inputs are rejected during decompression, after small checked
RGB allocation. New native 4:1:0 / 2:4 subsampling codes are not recognized by
this wrapper and become header errors. These restrictions are explicit prototype
compatibility limits, not support for all JPEG variants.

Neither old nor new source path applies EXIF orientation or ICC color management.
RGB channel order is tested. A source tagged orientation 6 remains in stored
pixel orientation with identical dimensions; the photo check confirms that
assumption, not production correctness for artist media.

## Measurement method and interpretation

`thumbnail_microbench` is an isolated executable; `native_decoder_experiments.py`
runs each codec in a fresh process, measures eight distinct 6000×4500 corpus
images over 20 cycles, retains PNG comparisons and records executable/harness
hashes. Source I/O is outside timing; total includes metadata, scaled decode,
identical thumbnail resize and RGBA conversion, excluding PNG encode/storage.
Native decode sub-timing includes handle destruction whereas the reference's
decode sub-timing ends before destruction. **Total is the fair primary metric.**
Linux `/usr/bin/time -v` captures process peak RSS; per-cycle `/proc` RSS is sampled
after resize and misses transient scratch peaks. A plateau is a leak diagnostic,
not formal proof against every leak. End-to-end profiles separately include
source read, encode, storage, queues, upload and display coverage.

The photographed sanity input is [Hopetoun Falls](https://commons.wikimedia.org/wiki/File:Hopetoun_falls.jpg),
photo by **DAVID ILIFF**, [CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/).
The original and reencoded progressive/baseline 4:2:0/4:4:4, narrow crop and
orientation-tag derivatives remain under that image license in ignored benchmark
directories, with attribution. Only metadata/results are committed. It is one
photograph with controlled variants, not a representative artist collection.
No private images were used. Numeric RGB differences and visual inspection
check decoder agreement/recognizability, not color-managed fidelity.

See [the mission report](../MISSION_0_6_REPORT.md) and `benchmarks/mission0_6-*.json`
for results, exact provenance, repeat measurements, memory and recommendation.
