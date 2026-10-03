# Tack

Native, local-first reference board. This repository currently contains
**Phase 1B: local `.tack` persistence and product image rendering**. The executable
provides a minimal product CLI alongside the existing renderer benchmarks.
See [the current report](docs/MISSION_1B_REPORT.md),
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
Product creation/open commands are below; selection and image manipulation UI
remain outside this mission. An ordinary launch without a product subcommand
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
actions; the prototype retains its existing pan/zoom gestures. No final keyboard
preset is selected. See the [.tack compatibility contract](docs/design/tack_document_compatibility.md).

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
bounded to 6000×4500 and 64 MiB encoded parser work. No selection/manipulation UI or
higher-LOD product refinement is built. Pan/zoom remain available. A missing or
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
separate. Human review of Phase 1B's report is the stop gate before Phase 1C.

Diagnostic `--output` / repair report destinations must be new files. Existing
files and document aliases are refused to protect saved work and source images.

`repair` accepts a new destination or the original canonical input path. It
refuses a distinct existing destination, including another `.tack` document.
