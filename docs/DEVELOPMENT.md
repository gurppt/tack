# Building and developing Tack

Run commands below from the repository root. The [public README](../README.md)
introduces the application; [documentation](README.md) collects design and reports.

## Build prerequisites

The repository pins Rust 1.95.0 with rustfmt and Clippy in
`rust-toolchain.toml`. Use a Rust installation managed by rustup to select the
pinned toolchain automatically. Native image decoding additionally needs
Python 3.12+, Pillow 10.2.0, CMake, a C compiler and NASM on PATH.

On Linux, install the development libraries for your desktop/X11/Wayland build
and a graphics driver supported by wgpu. For MSVC builds, use a Visual Studio
compiler environment with CMake and NASM available. The Linux and Windows setup
used by CI is recorded in [Quality](../.github/workflows/quality.yml).

```bash
python3 -m pip install Pillow==10.2.0
python3 tools/prepare_turbojpeg.py
cargo build --release --locked -p tack-app
cargo run --release --locked -p tack-app
```

`prepare_turbojpeg.py` verifies the pinned official libjpeg-turbo 3.2.0 archive
and builds a static library into `target/native/`. Run it before Rust builds,
including tests. Cargo uses this explicitly prepared library. Pillow prepares
the compact About artwork at build time; it is not a runtime dependency. No
image corpus or benchmark run is required for an ordinary board.
Build-time downloads are separate from the offline application runtime.
See [decoder review](research/native_thumbnail_decoder.md) and
[dependency notices](THIRD_PARTY_NOTICES.md).

If your distribution blocks system-wide pip installs, create an isolated
`.venv`, activate it before Cargo, and install Pillow there. `PYTHON` can also
select a specific build interpreter. Reuse an existing environment rather than
creating one per experiment.

## Desktop helpers and troubleshooting

| Platform / operation | Helper |
| --- | --- |
| Linux X11 image/file/text clipboard | `xclip` |
| Linux Wayland clipboard | `wl-paste` / `wl-copy` from `wl-clipboard` |
| Linux X11 text-only clipboard fallback | `xsel` |
| Linux file pickers | `zenity` |
| Linux open linked source | `gio open` |

Helpers must be on the launching process's PATH. Tack requests clipboard data
only when Paste is invoked and reports missing helpers explicitly. Screenshots
are embedded regardless of the default import mode. File lists take priority
over image payloads and text; remote URIs are never downloaded. Windows retains
its native text/reference/file helper; Windows image clipboard runtime is not
validated by the build/test gate.

Linux/X11 with an affected old libXi (notably 1.8.1) can crash during startup
mouse motion. This optional workaround builds a verified app-local libXi 1.8.3
without changing system packages:

```bash
bash tools/prepare_linux.sh
cargo run --release --locked -p tack-app
```

It needs a C compiler, make, pkg-config, curl and X11 development headers. See
[startup diagnosis](X11_STARTUP.md). Wayland requires compositor decorations;
Tack does not bundle a client titlebar.

## Local executable and documents

On Linux, create or refresh the ignored repository-local human-test executable:

```bash
bash tools/build-test-bin.sh 'local development build'
./bin/tack
```

The helper atomically replaces `bin/tack` after a successful release build.
`bin/BUILD.txt` records commit, profile, UTC time, SHA256, dirty state and
checkpoint. A build in progress or failed build marks the previous stamp stale.
`bin/tack-about.png` is the compact packaged artwork; keep it beside the executable
when moving a build. These files are local outputs, not distributed binaries in Git.

Normal launch creates an Untitled board. Open a board by passing its path or
using `open`; `new` can reserve a new, unused filename:

```bash
target/release/tack-app /path/to/board.tack
target/release/tack-app open /path/to/board.tack
target/release/tack-app new /path/to/new-board.tack
```

CLI diagnostics can create, inspect or repair owned documents. `create` and
Save As refuse existing destinations; linked imports store references while
embedded imports retain source bytes. Diagnostic output files must also be new.

```bash
target/release/tack-app create /path/to/new-board.tack --embedded /path/to/image.jpg
target/release/tack-app inspect /path/to/new-board.tack
target/release/tack-app --help
```

See [storage compatibility](design/tack_document_compatibility.md) and
[local workflow guarantees](design/local_production_hardening.md) for exact
save/recovery, locking, repair and retained-original semantics.

Profiles live under `$XDG_STATE_HOME/tack` or `~/.local/state/tack` on Linux and
`%LOCALAPPDATA%/tack` on Windows. `TACK_PROFILE_DIR` selects an isolated profile.
Keymap import/export uses readable JSON. Invalid or future profiles are protected
from accidental overwrite. Logging goes to stderr; `RUST_LOG` controls verbosity.

## About text and artwork

Open **About Tack** from right-click → Tack or F10. Escape, Enter and Close
dismiss it. Edit [`gfx/about.toml`](../gfx/about.toml) and rebuild to change the
mentions. Cargo's package version is authoritative: omit `version` or match it.
The tiny schema supports name, version, author, website, contact, license,
copyright, tagline and source. Empty contact displays “Not set”; no license is
chosen by this feature. Links are readable text, with no browser helper.

[`gfx/tack_about.png`](../gfx/tack_about.png) is the editable source. Cargo runs
[`prepare_about.py`](../tools/prepare_about.py) to validate text and derive a
207×224 nearest-sampled lossless PNG for the current artwork. A replacement must
be a portrait PNG, width/height ≤0.95 and dimensions ≤4096. Metadata must fit
13 wrapped rows at 18 bitmap cells for the 800×600 2× layout. Invalid inputs stop
the build with an explicit error; they do not silently truncate owner mentions.
Cargo packages the compact PNG as `tack-about.png` beside the executable (and
test executables). It is read and decoded on demand by the existing local worker;
its decoded payload and texture are dropped on dismissal. Missing/corrupt
artwork leaves the text and Close usable. The editable full source is never read
by the runtime. There is no asset-file access when About is unused.

## Quality checks

After preparing the native decoder:

```bash
python3 -m pip install Pillow==10.2.0
cargo install cargo-deny --version 0.20.2 --locked
bash tools/check.sh
cargo test -p tack-render --test gpu_smoke --test product_gpu --locked -- --include-ignored
```

The check script runs formatting, all-target/all-feature check, warnings-denied
Clippy, workspace tests, docs, Python tests and dependency checks. The separate
GPU command executes hardware-dependent tests skipped by the ordinary suite.
Windows build/test CI does not substitute for a real Windows clipboard session.

## Native checks and performance evidence

Use the [current manual checklist](HUMAN_TEST_1J.md) for desktop validation.
`tools/run_native_polish_checks.py`, `tools/run_clipboard_producers.py`,
`tools/run_polish_performance.py` and `tools/run_polish_idle.py` provide the
current clipboard/menu/theme/background/idle evidence. Use explicit binary/output
paths and the fixture inputs required by each suite; inspect its `--help` before running. The native
polish tools require an explicitly owned isolated X11 display and
`TACK_NATIVE_NO_WM=1`; they refuse the default user desktop.

Earlier suites remain available for targeted reproduction: `run_benchmarks.py`,
`run_streaming.py` and `run_preparation.py` cover synthetic renderer/preparation
experiments; image, annotation, spatial, persistence and supply harnesses cover
local product behavior. Their owning [reports](README.md#development-reports)
document fixtures, commands and scope. Historical results are not a blanket
performance claim for current hardware or current code.

Run native measurements separately from other GPU work. Retain raw CPU/GPU
telemetry; GPU timing is distinct from CPU submission/presentation. Headless
software adapters validate correctness, not representative hardware performance.
Raw assets and results stay in ignored `benchmark-data/` and `benchmark-results/`;
compact published receipts are in [benchmarks](../benchmarks/).

## Disk discipline

Inspect disk space before large builds or fixture generation. Reuse the shared
`target/`, native libraries and existing corpus; keep at least 10 GiB free.
Prefer a 512 MiB generated-data budget per phase. Some historical harnesses copy
binaries or large cache inputs, so inspect their cost before starting a suite.
Clean only known disposable generated files after preserving compact evidence.
Never delete user boards, originals, profiles or the sole proof of a result.
See [contribution rules](../CONTRIBUTING.md) for the full policy.
