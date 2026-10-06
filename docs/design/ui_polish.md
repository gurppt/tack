# Native UI polish and clipboard policy — Phase 1J

Right-click builds the existing application descriptor block followed by a
decorative context heading and the existing contextual descriptors. F10 uses
the same application catalog. Action dispatch and document history remain the
existing authorities. Headings do not consume keyboard selection. One bounded
child submenu is allowed; paired columns fit the viewport and leave the parent
accessible at 800×600/2×. There is no permanent Tack button, hover timer or
animation. Menus allocate only while open and release their lists on dismissal.

Preferences has direct Auto/1/2/3/4 scale and three-theme selectors. Enter
selects, Escape returns to Preferences; both directions work. Integer geometry,
bitmap glyphs and scale snapping retain the existing native policy. Theme is
an optional enum in profile version 2, defaulting to Neutral Gray for older
profiles. Unknown future enum values follow protected-profile rejection; they
are not silently reset and saved. Scale and theme never enter board storage.

`Theme::palette()` returns one small immutable semantic palette. Primary and
secondary text, cyan navigation, magenta tools, yellow shortcuts, disabled
states, selections, guides and grid use the same tokens. Commands retain labels
and selected markers; color is not the only state signal. Tests check contrast
on both menu and selected backgrounds. Editor chrome follows the palette;
unfilled notes with insufficient background contrast get a cheap flat backing
at presentation time. Authored colors and document geometry are unchanged.

The existing background/grid fullscreen triangle computes a slight vertical
screen-space mix of two nearby colors. An 80-byte uniform carries endpoints,
grid color and grid enable state. It draws in the existing canvas render pass
with no texture, new pass, animation or time input. Grid-hidden drawing now
needs a fullscreen fill where the old path could only clear; that measurable
cost is reported in the phase report rather than described as zero.

Clipboard requests use the existing on-demand local worker. A single normalizer
prioritizes local URI lists/gnome-copied-files, PNG/JPEG, then text. Active editor
paste selects only text. Legacy text containing explicit file URIs or an
existing absolute PNG/JPEG path stays compatible; ordinary slash-leading prose
is text. Each invalid/remote URI is rejected independently, never fetched.
Valid image imports keep deterministic admission and the existing history.

Linux resolves ordinary PATH helpers: native Wayland prefers `wl-paste`, X11
uses `xclip`, and `xsel` is a text-only fallback. Helpers must be executable files.
Missing desktop/backend and missing helper errors are separate and actionable.
Calls use fixed argument arrays, never shell interpolation. TARGETS is bounded
to 4096 bytes/3 seconds, text/references to 64 KiB, images to 64 MiB/10 seconds;
references also retain the existing 4096-file import cap. Notes retain their
16 KiB cap. Diagnostics log MIME/backend/length, never clipboard content.

Linux pipe reads are nonblocking and bounded, with cancellation checks. Timeout
or cancellation kills/reaps only the spawned helper's owned process group,
including descendants retaining stdout. PNG/JPEG staging is private, exclusive,
header-validated and always imported embedded. Publication requires worker
acknowledgment: cancellation between read and UI admission discards the result
and removes staging before completion. Successful import also removes its owned
temporary source; other failures may retain a bounded partial staging file until
the next paste/session cleanup. Unrelated files never become cleanup targets. There is no
resident selection owner, watcher, periodic clipboard read or idle worker.

No Rust dependency was added. External desktop helpers are not linked into Tack.
Windows retains its existing text/reference path; Windows build/test evidence
does not certify Windows image clipboard runtime. Native Wayland runtime and
the owner's screenshot producer remain separate hands-on checks.

Development builds use reduced debug information and the shared target directory.
Phase fixtures and one comparison binary are reused; compact raw measurements
and selected screenshots are retained. `CONTRIBUTING.md` defines the 10 GiB free
space reserve, preferred 512 MiB phase-data budget and targeted cleanup policy.
