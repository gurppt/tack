# About modal and package

The shared Tack menu adds one `Action::About` for right-click and F10. It has
no default shortcut or permanent control. The modal consumes canvas input;
Escape, Enter and the single Close button dismiss it. It uses existing bitmap
text, palette and flat overlay rectangles; no gradient, new font engine,
animation, scrolling or timer. Website/contact/source are readable colored text.

`gfx/about.toml` is owner editable. Cargo runs `tools/prepare_about.py` using
Python 3.12+ and Pillow 10.2.0, already used by repository image tooling. Cargo's
package version is authoritative: an explicit metadata version must match it.
Unknown fields, control characters, fields over 96 characters, empty name and
more than 13 wrapped rows at 18 cells are rejected. Word wrapping keeps names
readable, and long URLs can split without discarding characters. Cargo compiles
immutable strings; the application never parses TOML.

The editable `gfx/tack_about.png` is prepared with nearest sampling into a
207×224 lossless RGBA PNG (38,426 bytes for the supplied artwork). Runtime memory
is independent of the full 587×635 source. Cargo places `tack-about.png` beside
normal and test executables. `tools/build-test-bin.sh` prepares and replaces the
human package in `bin/`, recording both binary and asset hashes. Keep the small
PNG beside the executable when transferring a package.

An embedded prototype was measured first: all 11 file-backed pages intersecting
the PNG were resident before About opened (45,056 bytes, including boundary
constants). The shipped choice is a lazy sidecar, with no PNG bytes in the
ordinary executable and no asset-file access until the semantic action. This
avoids that unnecessary fixed image residency. The fixed additions are small
metadata/menu strings, an optional image pointer and a request counter.

Opening uses the existing bounded local worker and one result slot. It reads at
most 128 KiB + one overflow-detection byte, decodes through the existing PNG
library, rejects dimensions over 256×256 and limits decoder scratch to 1 MiB.
The supplied decoded RGBA is 185,472 bytes; encoded storage is transient (bounded
read-buffer capacity can exceed encoded length). A request ticket and current
modal guard discard late results after closure or replacement. There is no
resident About thread or queued polling timer.

The renderer owns one optional texture plus six vertices (185,592 payload bytes
for this image, excluding driver objects/staging). It reuses the existing image
pipeline, nearest binding and canvas pass, drawing after the modal overlay.
Dismissal drops this resource and the decoded CPU result; submitted commands may
retain GPU resources until completion. Normal frames traverse only empty-option
checks. Driver allocation caches and total process RSS are separate from owned
payload accounting.

At 800×600, the panel is 600×288 logical pixels at 1× and 376×276 at 2×. Artwork
is respectively 207×224 and 196×212 logical pixels; aspect rounding stays within
0.3%. The narrow layout has 18 text cells and 13 visible rows. Text and Close
remain inside the viewport. Larger UI scales on tiny windows retain the existing
application limitation; they are not claimed by this addendum.

Missing, corrupt, oversized or dimension-mismatched packaged artwork yields a
text-only About with “Artwork unavailable”. Escape, Enter, Close and subsequent
canvas actions remain usable. No browser-opening dependency is added. Contact
and project license remain undecided; this feature chooses neither.
