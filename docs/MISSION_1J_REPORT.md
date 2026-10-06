# Phase 1J — clipboard, unified menu, scale and visual polish

Prepared 2026-10-06. Implementation: `38529aa5262bcf02ffd7dd67fcb6f24091c4c489`.
Technical implementation, local gates and exact implementation CI pass.
Owner comfort/aesthetic review remains pending. Phase 2 has not started.

## Result

Linux clipboard uses the existing local worker with safe deterministic MIME
priority: local file list, PNG/JPEG, then text. Screenshot payloads are embedded.
Valid local images survive invalid/remote entries with one bounded rejection
summary. Plain text creates one undoable note or pastes into the active editor;
ordinary slash-leading text stays text. Errors identify a missing desktop,
missing helper, unsupported content or oversized payload. Helpers are reaped on
completion, failure and cancellation. Successful import and cancellation after
read publication remove owned staging before completion. A partial staging file
from another failure can remain bounded until the next paste or session cleanup;
it never becomes document source authority.

Right-click always combines the shared Tack application block with the original
context-specific actions. Canvas, image, multiple selection, note, frame and
annotation use the same upper block; F10 shares the catalog and dispatcher.
The permanent top-left Tack button is removed. Paired submenu placement at
800×600/2× leaves parent Preferences and Keymap accessible by pointer/keyboard.

Preferences directly selects Auto/1×/2×/3×/4× and exactly Very Dark, Neutral Gray
or Light. Both scale directions and profile restart are tested. Old profiles
default to Neutral Gray; unknown future theme values preserve protected-profile
behavior. A tiny immutable palette gives cyan headings/navigation, magenta tool
actions and acid-yellow shortcuts with readable selected/disabled states.
Unfilled low-contrast notes get a flat presentation backing on the light theme;
stored style/geometry remains unchanged. Theme never enters `.tack` storage.

All three backgrounds use a very slight screen-space vertical gradient in the
existing background/grid triangle and canvas render pass. No background texture,
additional pass, animation or new GUI dependency was added. Bitmap glyphs,
integer UI geometry and independent Smooth/Nearest image sampling remain intact.
See [design](design/ui_polish.md) and [inventory](FEATURE_INVENTORY.md).

## Clipboard reproduction and native evidence

Host: Ryzen 7 2700X, NVIDIA RTX 2060, Vulkan, Linux. Automation owns Xvfb `:98`
without a window manager; it does not inject input into the owner's desktop.
The selected rendering adapter is NVIDIA/Vulkan. Unselected GL probing can
leave llvmpipe task names; those names alone do not identify the active renderer.

Original Phase 1I executable SHA256:
`56ffdaf83a6f4678e20127c887cf2b64788b7ce03cb87bbc2fe34a5facc4ab90`.
With a controlled ordinary helper-missing PATH, screenshot PNG, one URI,
multiple URIs and UTF8 text each admitted zero objects and reported exactly
`native file/clipboard helper unavailable on this host`. Helpers were absent
from the initial host PATH. This is a controlled reproduction, not a claim to
have observed every owner's original producer invocation.

This session installed the distro `xclip` executable locally in
`~/.local/bin` (no system package mutation; external executable, not linked).
Normal PATH resolves it. A missing helper now gives an explicit install message;
X11 uses xclip, Wayland uses wl-paste, xsel is a text-only fallback. Desktop
launchers must include the helper's PATH or use the distro's ordinary installation.

The final native harness passes **27/27** assertions at 800×600/1× and 2×,
1024×768 and 1600×900. Cases cover PNG/JPEG payload, PNG/JPEG URI, GNOME-style
multiple copying, mixed valid/invalid/remote entries, text/new note/editor,
shared menu Paste, unsupported MIME, remote-only refusal, 64 MiB overflow,
missing X11/Wayland helper/backend, late cancellation/staging cleanup, worker
release, direct scale and all themes, profile restart, F10 and actual Keymap
unassign/reset, and unchanged saved board bytes after theme/scale selection.
MIME/backend/byte-count diagnostics exclude clipboard content.

Actual **Dolphin** copying is checked separately: one PNG, one JPEG and two
images advertise `text/uri-list` alongside KDE/portal/text formats. The chosen
MIME is `text/uri-list`; normalized file counts are 1/1/2 and all are admitted.
An actual capture of the owned native window, published through X11 image/png,
is pasted as one embedded image. Isolated **Flameshot/private-D-Bus activation
failed** and is retained as a failed producer attempt, not a Flameshot pass.
The owner's usual screenshot application on the normal desktop remains a
[hands-on check](HUMAN_TEST_1J.md). Windows/Wayland clipboard runtime is untested.

Selected native screenshots are retained in `benchmarks/phase1j-ui/`; receipts
and hashes are in [phase1j.json](../benchmarks/phase1j.json). Full local raw
evidence is in ignored `benchmark-results/phase1j/`, including failed attempts.

## CPU/GPU comparison

Empty local board, same machine/adapter, warmed caches, user-driven pan,
serialized GPU ownership. Two pairs reverse before/after order. Three viewport
sizes × grid hidden/visible × baseline/current × two pairs = **24 runs**,
**2696 measured frames and GPU timestamps**; initial three frames excluded.
Both executables are stripped consistently, with `.text` identity verified
against the human executable. No binary/corpus copy is made per run.
Values below are means of the two run medians; raw p99/max and all reports are
retained. GPU pass timestamps are distinct from CPU submit/present timings.

| Viewport | Grid | Before GPU ms | After GPU ms | Before CPU ms | After CPU ms |
| --- | --- | ---: | ---: | ---: | ---: |
| 800×600 | hidden | 0.005360 | 0.011936 | 0.174916 | 0.165031 |
| 800×600 | visible | 0.012496 | 0.012272 | 0.190871 | 0.156191 |
| 1024×768 | hidden | 0.005120 | 0.017008 | 0.219006 | 0.216597 |
| 1024×768 | visible | 0.017456 | 0.017632 | 0.208761 | 0.199431 |
| 1600×900 | hidden | 0.005456 | 0.028480 | 0.162936 | 0.154506 |
| 1600×900 | visible | 0.028240 | 0.028688 | 0.156716 | 0.153401 |

Grid hidden adds approximately **6.576 / 11.888 / 23.024 µs**: the old path
could clear only; the new gradient fills the screen in the existing pass.
Grid visible changes −0.224 / +0.176 / +0.448 µs. This is a small measured
fill cost, not zero GPU work. The largest delta is about 0.14% of a 16.7 ms
frame budget. No consistent CPU regression is observed; lower medians are not
claimed as a general speedup. This is current hardware evidence, not a legacy
machine guarantee or the deferred global maximum-performance pass.

## Settled idle

Nine approximately five-second windows cover the baseline, all three themes
with hidden/visible grid, application menu open and menu closed. After settling:
zero app redraws, zero GPU submissions, zero `/proc` I/O deltas, zero main-thread
CPU ticks/context switches, zero RSS growth, no new worker/helper. There is no
clipboard watcher or background animation. NVIDIA driver housekeeping remains
observable as in baseline; whole-process wakeups are not falsely claimed zero.

## Binary, dependencies and disk

Final release executable SHA256:
`418d1fa7ea66e87c79eaaf3e178af37d2b9b5156768fe2556e6d6caa3d49ac18`.
Shipping release retains reduced crash-debug information: **115,418,672 bytes**.
Comparable stripped baseline: **17,908,392 bytes**; current: **17,931,688 bytes**;
delta **+23,296 bytes (+0.130%)**. Zero new Rust dependencies; unchanged lockfile
with 276 packages. External clipboard helpers remain on-demand desktop tools.

`CONTRIBUTING.md` now requires inspecting disk before large writes, preserving
10 GiB free, shared build directories/fixtures and preferably under 512 MiB of
generated phase data. Cleanup targets known disposable artifacts while preserving
source, briefs, user data and the only proof of a result. Development debug info
is reduced to level 1; release debug policy is unchanged. This phase stayed well
under the generated-data budget; redundant copies are removed after verification.
Useful build caches stay reusable rather than forcing expensive full rebuilds.

## Gates and delivery

Local gates: formatting, all-target/all-feature check, warnings-denied Clippy,
**163 Rust tests passed**, six explicitly ignored GPU cases in the ordinary
suite, docs, **9 Python tests**, dependency/advisory/license/source checks.
The separate hardware GPU suite executes the ignored cases: smoke plus seven
product GPU tests. `git diff --check` passes.

Exact implementation [Quality CI](https://github.com/gurppt/tack/actions/runs/37523458475)
is green for `38529aa5262bcf02ffd7dd67fcb6f24091c4c489`: Linux and Windows
format/check/Clippy/test/docs, Linux Python/software-Vulkan GPU, and dependencies.
Windows clipboard runtime is not implied by compile/test CI. Independent
source/native/measurement review found and corrected late-cancellation, MIME,
mixed-batch, placement, contrast and plain-text issues; no unresolved correctness
finding remains. See [VERIFICATION_1J.md](VERIFICATION_1J.md).

The atomic `./bin/tack` and `./bin/BUILD.txt` identify the final delivery commit,
release profile, UTC time, SHA256, dirty state and checkpoint. Untracked owner
`gfx/` files are preserved and account for the dirty state. **Owner review is
pending; stop here before the next major branch.** No owner aesthetic approval
or unperformed Windows/Wayland clipboard runtime pass is asserted.
