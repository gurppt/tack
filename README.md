# Tack

Native, local-first reference board. This repository currently contains
**Phase 1D: grid, snapping, arrangement, groups and named frames**. The executable
provides a minimal product CLI alongside the existing renderer benchmarks.
See [the current report](docs/MISSION_1D_REPORT.md),
[Phase 1C](docs/MISSION_1C_REPORT.md),
[Phase 1B](docs/MISSION_1B_REPORT.md),
[Phase 1A](docs/MISSION_1A_REPORT.md),
[Mission 0.7](docs/MISSION_0_7_REPORT.md),
[Mission 0.6](docs/MISSION_0_6_REPORT.md),
[Mission 0.5](docs/MISSION_0_5_REPORT.md),
[the first prototype report](docs/MISSION_0_REPORT.md),
[architecture](docs/architecture.md) and
[asset experiments](docs/research/asset_pipeline_experiments.md).

Rust 1.95.0 with rustfmt/Clippy is pinned. Python 3 + Pillow 10.2.0 generates
the reproducible corpus. No private images or network are needed at runtime.

```bash
python3 -m pip install Pillow==10.2.0
python3 tools/generate_corpus.py benchmark-data/mission0
python3 tools/prepare_turbojpeg.py
cargo run --release --locked -p tack-app
```

The native thumbnail build needs Python 3.12+, CMake, a C compiler and NASM
on PATH. It verifies the official libjpeg-turbo 3.2.0 archive and installs a
static library only into `target/native/`; Cargo performs no native download.
Run this preparation once before any Rust build, including tests. For MSVC,
use an appropriate compiler environment; Windows CI is configured but has not
been executed locally. See [decoder review](docs/research/native_thumbnail_decoder.md)
and [dependency notices](docs/THIRD_PARTY_NOTICES.md). Runtime needs no native
decoder DLL or network. The separate Linux libXi workaround below still applies.

On Linux/X11 with an old libXi (notably 1.8.1), mouse movement during startup
can crash inside the system library. Use this optional local build once:

```bash
bash tools/prepare_linux.sh
cargo run --release --locked -p tack-app
```

It downloads the pinned official libXi 1.8.3 source, verifies its SHA256 and
builds into `target/native/`. Tack's Linux executable prefers that local library;
system packages are untouched. The helper needs a C compiler, make, pkg-config
and X11 development headers. See [startup diagnosis](docs/X11_STARTUP.md).

Pan with middle-button drag or Alt + left-button drag; zoom with the wheel.
The prototype uses a native window and progressive image loading. X11 uses
window-manager decorations; Wayland requires compositor decorations (no bundled
client titlebar).
Product creation/open commands and manipulation shortcuts are below. An ordinary launch without a product subcommand
still runs the benchmark prototype.

Run the original five scripted scenarios on your desktop GPU:

```bash
cargo build --release --locked
python3 tools/run_benchmarks.py
```

Each run creates a new results directory. Cold/warm runs reuse only the
persistent display cache; aggressive pan/zoom and memory pressure use isolated
caches. Results include raw frames, content coverage, submission completions,
cache/queue peaks, process RSS, asynchronous GPU pass timestamps when supported,
and native acquisition/presentation costs. Scripted runs fix the camera and
window size; use the ordinary interactive launch for manual pan/zoom.
Add `--headless` for an offscreen run; software
adapters do not validate the representative hardware target. Raw generated
assets/results are ignored by Git.

```bash
cargo install cargo-deny --version 0.20.2 --locked
bash tools/check.sh
cargo test -p tack-render --test gpu_smoke --locked -- --ignored
```

Run the Mission 0.5 adjacent pan, zoom/scan and full-board cold/warm suite:

```bash
cargo build --release --locked -p tack-app
python3 tools/run_streaming.py
```

It takes roughly six minutes, checks that all 1,000 overview thumbnails exist
on SSD, then reopens each trace with empty CPU/GPU caches. Later warm traces
progressively enrich the shared SSD cache. Two workers and no prefetch are the
default; `--workers 1|2|4` compares bounded concurrency. Individual experiments
can select `--scenarios pan-slow pan-normal pan-fast zoom-traverse scan board-tour`
and `--prefetch none|symmetric|directional` through `tools/run_benchmarks.py`.
The harness retains the executed binary and a source ZIP alongside raw JSON;
compact measurements live in [benchmarks](benchmarks/).

Run the Mission 0.7 charged preparation/partial-start suite:

```bash
cargo build --release --locked -p tack-app
cargo build --release --locked -p tack-assets --example overview_prepare
python3 tools/run_preparation.py prepare --output benchmark-results/my-preparation
python3 tools/run_preparation.py navigate --inputs benchmark-results/my-preparation --output benchmark-results/my-navigation
```

The first command suite measures independent 0/25/50/75/100% cache inputs,
two versus four workers, progressive JPEG memory, interrupted restart and cache
repair. Navigation clones each input separately for every 12-second trace;
partial starts continue bounded preparation. Fully prepared and subsequent warm
reopen have fresh RAM/VRAM. Preparation time is reported separately and must be
charged to startup; kernel page cache is not flushed. `--prepare-overview` on
`tack-app` is available only with a scripted `--scenario`; ordinary interactive
launch is unchanged. See [experiment details](docs/research/overview_preparation_experiment.md).

The core now supplies typed document/object/asset/source identities, validated
image metadata, reversible commands, bounded undo/redo and immutable render
queries. App foundations normalize input into bounded bindings and semantic
actions; the prototype retains its existing pan/zoom gestures. The product image window uses the initial documented manipulation preset. See the [.tack compatibility contract](docs/design/tack_document_compatibility.md).

Create and reopen a real local document after building:

```bash
cargo build --release --locked -p tack-app
target/release/tack-app create /tmp/reference.tack --embedded /absolute/path/image.jpg
target/release/tack-app inspect /tmp/reference.tack
target/release/tack-app open /tmp/reference.tack
```

`create` requires a new output path; it refuses existing files/symlinks.
`--linked` stores an absolute external descriptor instead of source bytes.
Embedded documents retain their originals after external deletion. Both store
cheap previews; open resolves visible previews on workers. JPEG/PNG import is
bounded to 6000×4500 and 64 MiB encoded parser work. Selection and manipulation are available in the product window. Higher-LOD
product refinement remains deferred. A missing or
changed linked source retains explicit state and may show a last-known preview.

```bash
target/release/tack-app repair /tmp/reference.tack /tmp/repaired.tack /tmp/repair.json
python3 tools/run_product_persistence.py --output benchmark-results/my-product-run
cargo test -p tack-render --test product_gpu --locked -- --include-ignored
```

Repair preserves authority and regenerates damaged/missing previews only. It
refuses a different parent directory if relative links exist; a future explicit
Save As/relink operation must preserve their bindings. See [format v1](docs/design/tack_file_format_v1.md)
for compatibility, streaming, save/recovery guarantees and limitations.

The code license/contribution model is pending; dependency license checks are
separate. Human review of Phase 1D's report is the stop gate before Phase 1E.

Diagnostic `--output` / repair report destinations must be new files. Existing
files and document aliases are refused to protect saved work and source images.

`repair` accepts a new destination or the original canonical input path. It
refuses a distinct existing destination, including another `.tack` document.

Open a `.tack` board to manipulate its images (the ordinary prototype launch
without `open` remains a navigation benchmark). Selection and camera are local;
image edits are undoable document state.

| Input | Product action |
| --- | --- |
| Left click / drag image | Select / move selected images |
| Shift + click | Add/remove selection |
| Empty click / drag | Clear / marquee selection |
| Corner / edge handles | Proportional / one-axis resize |
| Alt + handle drag | Resize about center |
| Rotation handle or Ctrl + left drag | Rotate about image/selection center |
| Ctrl + Alt + left drag | Uniform resize |
| Ctrl + Alt + Shift + C | Toggle single-image crop mode |
| Crop handles | Trim/restore pixels without stretching |
| Alt + Shift + H / V | Flip selected images horizontally / vertically |
| Alt + T | Cycle Default / Smooth / Nearest filtering |
| Ctrl + Alt + Shift + left drag | Adjust selected image opacity |
| Delete / Ctrl + A | Delete / select all images |
| Ctrl + Z / Ctrl + Shift + Z or Ctrl + Y | Undo / redo |
| Escape | Cancel current preview |
| Middle drag / Alt + left drag off handles | Pan |
| Wheel / double click image | Zoom / focus image |
| Ctrl + S | Save committed edits on a worker |

One gesture makes one undo step; crop is non-destructive and single-image only.
The title reports selection, dirty/save status and missing links. Close does not
autosave; use Ctrl + S. Window-manager shortcuts can intercept Alt combinations.

```bash
python3 tools/run_image_interaction.py --board /path/to/generated.tack --output benchmark-results/my-interactions
python3 tools/run_native_image_checks.py --output benchmark-results/my-native-checks
python3 tools/run_native_zoom_checks.py --binary target/release/tack-app --board benchmark-results/my-native-checks/board.tack --output benchmark-results/my-zoom-checks
```

Interaction benchmarks modify RAM and undo each cycle; they never save the input
board. Native checks generate and save their own two-image board. Run native
measurements separately from other GPU tests. See the
[interaction contract](docs/design/image_interaction.md),
[PureRef research](docs/research/pureref_image_interaction.md) and
[desktop freeze incident](docs/INCIDENT_2026_10_03_DESKTOP_FREEZE.md).


Spatial organization (Phase 1D) uses the same board, selection and undo stack:

| Input | Spatial action |
| --- | --- |
| G / Shift+G | Toggle dots / toggle object and grid snapping independently |
| Hold X during move/resize | Bypass snapping; release restores it |
| Ctrl+Left/Right/Up/Down | Align world-axis edges |
| Ctrl+Shift+Left/Up | Align horizontal/vertical centers |
| Ctrl+Alt+Shift+Up/Down or Ctrl+Shift+D/V | Distribute horizontally/vertically with equal nonnegative gaps |
| Ctrl+P / Ctrl+Shift+P | Pack row / column with 16 world-unit gaps |
| Ctrl+G / Ctrl+Shift+G | Group / ungroup selected images |
| Ctrl+Shift+F | Create frame around selection, or centered in view |
| F2 / Enter / Escape | Rename selected frame / confirm / cancel |
| Space / PageDown / PageUp | Focus selected / next / previous frame |
| Frame border/title / edge handles | Select/move / resize frame |

Groups are flat image membership and select/move as one unit; ungroup to edit a
member separately. Frames own no contents: moving one leaves images in place.
Marquee selects frames only when fully enclosed. Alignment uses rotated world
AABBs and treats groups as units. Distribution expands right/down if existing
space cannot fit the selected widths/heights without overlap.

Frame names preserve UTF-8. Compact labels use bundled OFL bitmap glyphs;
complex-script shaping is limited there, with full names also visible in the
native title. `tack-app --font-license` prints embedded font notices/license.
Images retain their selected sampling; pixel treatment applies only to overlays.
Schema 1 boards remain readable; groups/frames use [schema 2](docs/design/tack_file_format_v2.md).

```bash
python3 tools/run_spatial.py --output benchmark-results/my-spatial-run
python3 tools/run_native_spatial_checks.py --output benchmark-results/my-spatial-native
python3 tools/run_idle.py --board benchmark-results/my-spatial-native/board.tack --output benchmark-results/my-idle
```

## Phase 1E annotations and sources

Open a local board, choose `T` text (`Ctrl+N` also selects this tool), `R` rectangle,
`O` ellipse, `L` line, `A` arrow or `P` scribble, then drag. Text also accepts a
click for a default box. Type plain text; Enter adds a line, Ctrl+Enter commits,
Escape/focus loss cancels. `V` returns to selection. F2 or double-click edits a
selected note; Ctrl+A replaces its text, Backspace removes one Unicode scalar.
Editing supports append/replace, not a rich text cursor/navigation system.

For selected annotations, `C` cycles eight stroke/text colors, `F` toggles fill,
`[`/`]` decrease/increase world stroke width, Shift+`[`/`]` adjust opacity,
Ctrl+Shift+`,`/`.` change note font size, Ctrl+Shift+E cycles left/center/right
alignment. Move/resize/rotate/delete and mixed marquee use existing gestures and
atomic Undo/Redo. Crop/flips/filtering and flat groups remain image-only; grouping
a selection containing annotations gives an explicit error. Ctrl+S saves all
committed kinds exactly in [schema 3](docs/design/tack_file_format_v3.md), while
old schema-1/2 boards remain supported.

With exactly one linked image selected, Ctrl+Shift+O opens its actual source,
Ctrl+Alt+O reveals its parent directory, Ctrl+Shift+C copies the canonical path.
Linux uses `gio open`, and `xclip` on X11 or `wl-copy` on Wayland for clipboard;
Windows uses Explorer and clip. These tools must exist on the host. Embedded,
missing, foreign, executable or unsupported source files give explicit errors.
Only JPEG/PNG regular files are opened; no shell command is built from a path.
Each request runs off the UI thread; at most one is active, no idle source worker.

Notes use a lazy compact smooth font subset with the existing Unicode fallback;
UI labels remain pixel-native. See [font decision](docs/design/text_rendering_decision.md)
and [annotation integration](docs/design/annotation_objects.md). Advanced shaping,
bidi and rich text are deferred. Display is capped at 32768 annotation primitives;
selected/edited/transient objects receive priority, whole omitted objects trigger
a visible warning, and durable data is retained. Stroke capture is capped at 4096
points with a visible warning and deterministic simplification on completion.

Reproduction: `tools/run_annotations.py`, `tools/run_native_annotation_checks.py`
and the existing image/spatial/persistence/idle harnesses. `annotation-scale`
generates owned fixture documents; `open --annotation-benchmark` forces redraw
only for timed instrumentation, never for ordinary editing/idle.
