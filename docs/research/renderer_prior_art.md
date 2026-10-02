# Renderer prior art

Research pass: 2026-10-02. Independent implementation; no third-party application
code, icons, branding or bundled assets copied. Links below are primary sources;
performance claims by their authors are not measurements of Tack.

| Reference | Evidence inspected | Lesson for this experiment |
| --- | --- | --- |
| [BeeRef](https://github.com/rbreu/beeref) | Python/PyQt6; embedded PNG/JPEG in SQLite's sqlar table. [Image import helper](https://github.com/rbreu/beeref/blob/main/beeref/fileio/image.py) loads a QImage and reads EXIF orientation. | Separate geometry, originals and display cache. Import helpers can perform I/O; their thread boundary must be explicit. EXIF handling and portable embedded saves matter later, but the helper alone does not establish which thread calls it. GPL source studied for concepts only. |
| [Kanvaz](https://github.com/p4inz-code/kanvaz) | [canvas.js](https://github.com/p4inz-code/kanvaz/blob/main/src/canvas.js) transforms a world DOM element, coalesces grid redraw with requestAnimationFrame, and keeps the zoom pivot fixed. README describes ZIP + JSON + hashed assets. | Coalesce input redraw and preserve the cursor's world anchor. Container integrity can degrade per asset. Its browser/DOM architecture is outside Tack's prescribed rendering direction; its feature count is not a throughput benchmark. |
| [AnimRef](https://github.com/lettucegoblin/AnimRef) | Project README describes GIF/video/YouTube references and media trimming. | Motion references are useful, but each player has a resource cost. Keep media playback out of Mission 0; future inactive players should display a static representation. No residency or latency conclusion inferred from its README. |
| [sriv](https://github.com/dllu/sriv) | README architecture and [Rust source](https://github.com/dllu/sriv/blob/main/src/main.rs): viewport-prioritized thumbnails, background full-image LRU, GPU-visible thumbnails, tiled large images. | Prioritize current demand and separate CPU/GPU residency. Keeping every thumbnail in RAM is an explicit tradeoff that Tack replaces with a byte budget. Full-detail tiling is a candidate if single-texture uploads become too expensive. |
| [Viewskater](https://github.com/ggand0/viewskater) | Native Rust image viewer with [automated replay documentation](https://github.com/ggand0/viewskater/blob/main/docs/replay_mode.md), README limits and navigation. | Script navigation and retain per-frame measurements. GPU texture limits need an explicit policy; downscaling prevents oversized allocations but is not full-resolution fidelity. |
| [RefBoard](https://www.refboard.org/faq.html) | Public FAQ describes web/Windows delivery and Google login; [another same-name project](https://refboard.win/) also exists. | Name is ambiguous; do not assume a repository or claim source-level cache findings. The account-driven service workflow is outside Tack's local-first scope. |
| Blackboard / BeeRef forks | Searches did not unambiguously identify the intended primary source repository. | No code/license/performance conclusions assigned to an unidentified fork. Clarify the exact repository if import compatibility later depends on it. |
| [PureRef](https://www.pureref.com/handbook/navigation/) | Current official navigation handbook; no proprietary implementation inspected. | Navigation compatibility is verifiable; undocumented renderer internals and advertised performance are not evidence. See the separate shortcut map. |

Dependency decisions: wgpu supplies safe GPU ownership and portable Vulkan/D3D12/GL
backends; winit is the thin native event layer. image supplies JPEG/PNG only
(default format/features disabled); its [limits](https://docs.rs/image/latest/image/struct.Limits.html)
distinguish strict dimensions from best-effort allocation limits. The exact
locked JPEG decoder reads encoded input before limits are applied, so Tack caps
the encoded stream separately. pollster is used only during startup/tests;
serde handles the bounded benchmark manifest and output; bytemuck derives safe
vertex conversion; tracing supplies structured lifecycle/failure diagnostics.
Versions/checksums are in Cargo.lock. Dependency license/advisory verification
uses cargo-deny; the final Tack application license remains undecided.

Initial experiments to measure: linear culling and per-image draws at 1,000
objects, full JPEG decode followed by worker-generated LOD, byte LRU eviction,
two concurrent decoders, and a 16 MiB upload budget. Add an index, texture-array
batching, scaled JPEG decode or tiles only after the baseline identifies the
relevant cost. Final persistence and `.pur` import are not foundational
dependencies of this experiment.
