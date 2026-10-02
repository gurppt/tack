# Tack

Native, local-first reference board. This repository currently contains
**Mission 0: a measured renderer experiment**, before product development.
See [the report](docs/MISSION_0_REPORT.md), [architecture](docs/architecture.md)
and [prior art](docs/research/renderer_prior_art.md).

Rust 1.95.0 with rustfmt/Clippy is pinned. Python 3 + Pillow 10.2.0 generates
the reproducible corpus. No private images or network are needed at runtime.

```bash
python3 -m pip install Pillow==10.2.0
python3 tools/generate_corpus.py benchmark-data/mission0
cargo run --release --locked -p tack-app
```

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
The full UI and final document format are intentionally outside this mission.

Run the five scripted scenarios on your desktop GPU:

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

The code license/contribution model is pending; dependency license checks are
separate. Mission 0 ends with human review of its report before Phase 1.
