# Tack

**A native reference board for images, ideas and notes.**

[![Quality](https://github.com/gurppt/tack/actions/workflows/quality.yml/badge.svg?branch=main)](https://github.com/gurppt/tack/actions/workflows/quality.yml)

Tack gives illustrators, animators and designers a place to collect, compare and
arrange visual references. Drop in images, add notes, and build a board you can
keep beside your creative tools.

Built in Rust, with a native GPU-rendered canvas. Boards are saved locally as
`.tack` files and remain usable offline.

## What you can do

- **Collect references.** Drag and drop PNG/JPEG images, import files, or paste
  screenshots and copied image files on Linux. Paste text to create a note.
- **Compare and adjust.** Move, resize, rotate and flip images. Adjust opacity,
  crop without destroying the original, and choose Smooth or Nearest sampling.
- **Arrange your board.** Group images, align and distribute selections, pack
  rows or columns, and arrange references in a grid. Use snapping when needed.
- **Mark up ideas.** Add text notes, rectangles, lines, arrows and freehand
  strokes. Label regions with frames and jump between them.
- **Edit with confidence.** Undo and redo board edits, save your work, and recover
  completed recovery snapshots after an interrupted session.
- **Choose how images are stored.** Embed originals for portable boards or link
  to external files. Relink missing sources when files move.
- **Make the workspace yours.** Flat Very Dark, Neutral Gray and Light themes,
  crisp bitmap UI, direct integer scaling and editable shortcuts. Right-click keeps application and contextual commands together.
- **Share a board on your LAN.** Explicitly publish a local board to the optional
  headless server and edit it from several native clients. Local boards work
  independently; the server runs only when you start it.

The renderer loads image detail progressively and uses bounded caches. See the
[feature inventory](docs/FEATURE_INVENTORY.md) for exact behavior and limits.

## Get started

Tack is an actively developed prototype, currently distributed as source.
Build requirements: **Rust 1.95.0** (pinned by the repository), **Python 3.12+**,
**Pillow 10.2.0** (build only), **CMake**, **NASM**, a C compiler and a supported
desktop graphics driver.

```bash
git clone https://github.com/gurppt/tack.git
cd tack
python3 -m venv .venv
. .venv/bin/activate
python3 -m pip install Pillow==10.2.0
python3 tools/prepare_turbojpeg.py
cargo run --release --locked -p tack-app
```

The preparation step downloads and builds the pinned image decoder locally.
No generated test corpus is needed to use the application.

Large baseline JPEGs use streamed previews and requested detail tiles. Keep
the packaged `tack-jpeg-decoder` executable beside Tack when copying a build;
Cargo and `tools/build-test-bin.sh` prepare this automatically. Large progressive
JPEGs remain unsupported by the bounded detail path.

Launching opens an empty board. Drop in a few images, use right-click to explore
commands, and press **Ctrl+S** to save. Open an existing board with:

```bash
cargo run --release --locked -p tack-app -- /path/to/board.tack
```

On Linux, clipboard support needs **xclip** for X11 or **wl-clipboard** for
Wayland. **zenity** provides native file pickers; **xsel** is a text-only X11
clipboard fallback. See [building and troubleshooting](docs/DEVELOPMENT.md) for
platform setup and the optional X11 startup workaround.

## Everyday controls

| Input | Action |
| --- | --- |
| Middle-button drag / Alt + left drag away from handles | Pan |
| Mouse wheel | Zoom around the cursor |
| Left click / drag | Select / move |
| Shift + click | Add or remove an item from the selection |
| Resize / rotation handles | Resize / rotate the selection |
| Right-click / F10 | Context and application commands / application menu |
| Ctrl+I / Ctrl+V | Import images / paste |
| T / R / L / A / P | Text / rectangle / line / arrow / freehand |
| Ctrl+Z / Ctrl+Shift+Z | Undo / redo |
| Ctrl+S / Ctrl+Shift+S | Save / Save As |
| Ctrl+O / Ctrl+N | Open / new board window |
| Ctrl+, | Preferences |
| Escape | Cancel an interaction or dismiss a menu |

These are the default bindings. Open **Keymap** from right-click or F10 to
search, change or reset shortcuts. In a text note, **Ctrl+Enter** confirms editing.

## Project status

The current desktop runtime is tested on **Linux/X11**. **Windows** builds and
tests run in CI; Windows image clipboard and Wayland clipboard runtime have not
yet been validated. Current image import supports PNG/JPEG up to 6000×4500 pixels
and 64 MiB per encoded image. Boards use Tack's own format; `.pur` import is not
implemented.

Optional LAN collaboration is a foundation for trusted networks. It has no
authentication or TLS, uses conservative conflict/undo refusal, and requires an
explicit reconnect after connection loss. Shared snapshot export has a tested
storage API; its desktop action is still pending. See the
[LAN test guide](docs/HUMAN_TEST_2A.md) for server, Publish and Join commands.

See the [LAN report](docs/MISSION_2A_REPORT.md),
[image supply report](docs/MISSION_1L_REPORT.md),
[UI/clipboard report](docs/MISSION_1J_REPORT.md),
[About report](docs/MISSION_1K_ABOUT_REPORT.md) and
[manual test checklist](docs/HUMAN_TEST_1J.md) for validation details.

## Documentation

- [Build, troubleshooting and developer checks](docs/DEVELOPMENT.md)
- [Complete feature inventory](docs/FEATURE_INVENTORY.md)
- [Architecture](docs/architecture.md)
- [Board format and compatibility](docs/design/tack_document_compatibility.md)
- [Design notes, reports and performance evidence](docs/README.md)
- [Contribution guidelines](CONTRIBUTING.md)

## License

The project license and contribution terms are still being decided. Dependency
and font licenses are documented in [third-party notices](docs/THIRD_PARTY_NOTICES.md).
