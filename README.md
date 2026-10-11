# Tack

**A reference board you can work on alone or together.**

[![Quality](https://github.com/gurppt/tack/actions/workflows/quality.yml/badge.svg?branch=main)](https://github.com/gurppt/tack/actions/workflows/quality.yml)
<img width="947" height="739" alt="image" src="https://github.com/user-attachments/assets/ea2c8659-7608-4ab5-a151-30b9551492b6" />
<img width="943" height="734" alt="image" src="https://github.com/user-attachments/assets/b2aae767-2773-4df2-a477-6ee4bddbd928" />
<img width="943" height="739" alt="image" src="https://github.com/user-attachments/assets/feaa6eb4-72e8-4332-b60b-b99e6c586eab" />


Tack is a lightweight, native reference board for animators, illustrators, designers and creative teams. Collect images, explore ideas, annotate, organize and keep everything in sight while you work.

**The difference? Your reference board doesn't have to be yours alone.**

Work locally, completely offline, as you would with a traditional reference board. When you need other people involved, share a copy on your local network and work on the same board together — without moving your workflow into a browser.

**Local-first. Collaborative when you need it.**

> **Project status:** Tack is in **pre-alpha development**. Local boards and experimental collaboration over a **trusted LAN** are implemented. Collaboration over the wider Internet, browser-based access and a public alpha release are **not available yet**. See [Current status](#current-status) before testing it in production.

## One board. More than one pair of hands.

Reference boards are essential to visual work. But as soon as a director, an animator, a layout artist and a production team need the same references, the board often turns into screenshots, exported images, duplicated files and messages explaining what changed.

Tack is being built to remove that friction.

- **Make a board for yourself.** No login, hosted workspace or network connection required. Your board is a local `.tack` file.
- **Bring in your team.** Share a separate copy from Tack, send an invite and let teammates open the same board in their own native Tack windows.
- **Work on it together.** Changes accepted by the shared board are synchronized between connected clients. Independent references can be manipulated by different artists; conflicting actions are handled explicitly.
- **Take your work back offline.** Save an independent local copy of a shared board when its original image sources are available.

The shared experience is optional. **An ordinary local board never needs a server.**

### Sharing a board today

On a trusted local network, choose **File → Share Board → Share from this computer**. Tack creates a separate shared copy, preserving the original local board. Use **Copy Invite** and send it to another Tack user, who can choose **File → Join Shared Board** and paste the invitation.

This is an early LAN implementation, not a public Internet collaboration service. The current transport does **not** provide authentication or TLS encryption; do not expose the server directly to the Internet or untrusted networks.

## Made for visual work, not meetings

Tack stays focused on the things artists do with references every day:

- **Gather.** Drag in PNG and JPEG images, import files, paste screenshots or text, and keep references on a flexible canvas.
- **Compare.** Move, proportionally resize, rotate, flip, crop, fade or temporarily view images in grayscale. Switch between smooth and nearest-neighbor sampling.
- **Organize.** Create named Frames, group images, align and distribute selections, arrange them into rows or grids, and use snapping when it helps. Link objects to a Frame so they travel with it.
- **Annotate.** Add text blocks, rectangles, lines, arrows and pixel-crisp freehand Scribbles. Edit or erase strokes without flattening the underlying images.
- **Stay in control.** Undo and redo edits, lock items you don't want to move, bookmark useful views, and use customizable shortcuts and toolbars.
- **Keep the originals.** Embed images inside a portable board or link to files on disk. Reconnect missing sources and explicitly change how they are stored.

The goal isn't to become an all-purpose whiteboard, project manager or design suite. **It's to remain a very good reference board.**

## Fast by design. Quiet when idle.

Tack is a native desktop application built in Rust, with a GPU-rendered canvas and a deliberately compact pixel-art interface. It is designed to sit beside demanding creative applications rather than compete with them for resources.

Large images are supplied progressively instead of being decoded wholesale just to show a small preview. CPU and GPU caches are bounded, background work is limited, and a settled local board is designed not to redraw or poll unnecessarily.

Those are engineering goals backed by development tests, **not a promise that every board or every older machine will reach a particular frame rate**. The project's [benchmarks](benchmarks/) and [technical documentation](docs/README.md) record what has actually been measured.

## Your board. Your setup.

Tack saves documents as local `.tack` files. Embedded boards carry their original image data; linked boards refer to external files. There is no mandatory cloud account or subscription service.

Your workspace is customizable without turning into a complicated application: three compact themes, crisp bitmap controls, adjustable toolbars, named toolbar sets, and a remappable keymap. Export a `.tackprofile` to bring preferences, toolbar sets and shortcuts to another installation; `.tackey` remains available for keymap-only exchange.

## Getting started

**There is no public alpha download yet.** The project can currently be built from source, but the desktop and LAN workflows still need wider human testing. Portable Linux and Windows packages are being prepared for controlled alpha testing.

If you're building it now, the shortest path on a suitably configured development machine is:

```bash
git clone https://github.com/gurppt/tack.git
cd tack

python3 -m venv .venv
. .venv/bin/activate
python3 -m pip install Pillow==10.2.0
python3 tools/prepare_turbojpeg.py

cargo run --release --locked -p tack-app
```

The build needs the repository-pinned Rust toolchain, Python 3.12+, CMake, NASM, a C compiler and suitable desktop/graphics dependencies. The image decoder preparation step downloads and builds the pinned decoder. See [building and troubleshooting](docs/DEVELOPMENT.md) for the actual prerequisites, platform-specific details and portable build procedure.

Tack starts with an empty board. Drop in some images, right-click to explore commands and press **Ctrl+S** to save. You can also open an existing board directly:

```bash
cargo run --release --locked -p tack-app -- /path/to/board.tack
```

### A few useful controls

| Input | Action |
| --- | --- |
| Mouse wheel | Zoom around the pointer |
| Middle drag / Alt + left drag away from handles | Pan |
| Left click / drag | Select and move |
| Shift + click | Extend or reduce the selection |
| Right-click / F10 | Context menu / application commands |
| Ctrl+I / Ctrl+V | Import / paste |
| T / R / L / A / P | Text / rectangle / line / arrow / Scribble |
| Ctrl+Shift+F | Create a Frame |
| Ctrl+L / Ctrl+Shift+L | Link / unlink selection to a Frame |
| Ctrl+D | Duplicate |
| Ctrl+Z / Ctrl+Shift+Z | Undo / redo |
| Ctrl+S | Save |
| Ctrl+PageDown / Ctrl+PageUp | Next / previous toolbar set |

Most shortcuts can be remapped through **Keymap**. The [feature inventory](docs/FEATURE_INVENTORY.md) describes the current commands, their behavior and known limitations.

## Current status

Tack is actively developed and **not production-ready**. Its current development version is `0.1.0-dev.1`.

- **Local boards:** available in the development builds, with local saving, recovery, image manipulation, organization and annotations.
- **LAN collaboration:** implemented for trusted local networks, with explicit invites, synchronized accepted edits, conflict handling and local-copy workflows. Physical multi-machine acceptance is still in progress.
- **Desktop platforms:** Linux/X11 has native test coverage. Windows packages build and run through CI checks, but complete real-desktop validation remains pending. Some clipboard and desktop behaviors vary by platform.
- **Very large images:** bounded previews and regional detail loading exist, with format-specific constraints. See [image-supply details](docs/design/huge_images.md).
- **Not implemented yet:** general Internet/WAN hosting, browser access, built-in cloud accounts, full timed-media playback, and direct import of PureRef `.pur` or Miro boards.

If you're considering Tack for a studio, please treat it as a project to **test with copies of non-critical boards**, not the sole home for production references. The [alpha acceptance checklist](docs/HUMAN_TEST_2A8_ALPHA.md) tracks what still needs real-world validation.

## For developers

Tack's architecture is deliberately modular. The shared semantic document model is independent of the renderer and storage backend; local editing requires no network services. Optional collaboration runs through a separate headless server, with bounded client workers and source caches.

The workspace is divided into focused crates:

- `tack-core` — document model, commands and geometry
- `tack-assets` — image preparation, decoding and bounded image supply
- `tack-render` — GPU canvas, overlays and pixel rendering
- `tack-storage` — `.tack` persistence and recovery
- `tack-app` — native desktop UI and interaction
- `tack-shared` / `tack-server` — optional LAN protocol and authoritative server
- `tack-update` / `tack-updater` — manual portable update support

The project favors **small fixed costs, bounded work, no unnecessary idle activity and no mandatory web stack**. New importers or media support should remain optional rather than becoming permanent dependencies of the core.

For architecture, profiling evidence, test instructions, protocol boundaries and development history, see:

- [Documentation index](docs/README.md)
- [Architecture](docs/architecture.md)
- [Build and development guide](docs/DEVELOPMENT.md)
- [Complete feature inventory](docs/FEATURE_INVENTORY.md)
- [Board format and compatibility](docs/design/tack_document_compatibility.md)
- [LAN collaboration design](docs/design/lan_collaboration.md)
- [Benchmarks and test results](benchmarks/)
- [Contributing](CONTRIBUTING.md)

## License

The project license and contribution terms have **not yet been finalized**. Do not assume that the absence of a license file grants permission to redistribute or reuse the source. Dependency and font notices are documented in [third-party notices](docs/THIRD_PARTY_NOTICES.md).
