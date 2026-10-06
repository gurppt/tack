# Mission 1H — native follow-up after access restoration

**B — implemented, saved/pushed and native automated gates complete; uncoached
human discovery/comfort review pending.** Runtime code is committed in `76e67c8`,
with owned-picker harness correction in `deeb0fe` and visible Keymap assertion
in `b519b7f`; all three exact configured CI runs
are green for Linux, Windows and dependencies. Current executable SHA256 is
`4d21e4ea6322f9898abf82682513d9339cd9b0f1abd15ac894d1d76a697af6ee`;
`./bin/BUILD.txt` records the finalization commit. The old read-only Git/network/
X11 restrictions below are closed.

[Updated 1G report](MISSION_1G_REPORT.md) and
[current native receipts](../benchmarks/phase1g-updated.json) contain 30 contextual
checks at 800×600/1024×768/1600×900, real 1×/2× captures, 21 production checks on an
800×600 desktop, 19 recovery checks on 1024×768, static open/closed menu idle,
six settled supply views, two instance pairs and current GPU/quality results.
The common document history passes contextual edits and long-drag Undo/Redo.
Menus preserve true idle of main/decoder workers; driver-thread activity and total
RSS are explicitly retained. No human or physical Windows desktop result is
claimed. Use [the human checklist](HUMAN_TEST_1H.md) before awarding discoverability
or comfort acceptance. No Phase 2 was started.

## Historical implementation and offscreen checkpoint

The original checkpoint below refers to the preceding `918843…` binary and its
frozen source/receipt archive. Statements that native checks, Git or CI were
blocked describe that checkpoint, not the current handoff. Offscreen UI cost
figures remain valid narrower evidence; current native measurements are above.

---

# Mission 1H — contextual pixel UI and feature discoverability

**B — implementation and local checks complete; mandatory native low-resolution
and human discovery gates pending.** The resumed session cannot create native
X11 sockets and `.git` is read-only. Do not mark this mission A/PASS from the
software offscreen evidence. Stop after 1H; no Phase 2, collaboration or final
performance pass has started.

## Result / existing architecture

The native window already owned `ImageInput`, the semantic `Action` catalog,
normalized keymap, image/annotation gestures and one `DocumentEditor` history.
Selection handles and temporary local panels use `ImageGizmo` rectangles and
existing bitmap glyphs in a bounded GPU overlay. There was no visible application
command entry; F10 opened the older large local-files panel. Many commands were
reachable only by shortcuts or remapping.

The canvas now has one **44×18 logical pixel Tack entry**, 0.165% of 800×600 at
1× (0.66% at 2×). It opens compact File/Edit/View/Tools/Preferences commands,
also available through F10. There is no sidebar, ribbon, permanent inspector or
new font/widget dependency. Existing panels remain temporary.

The [feature inventory](FEATURE_INVENTORY.md) audits all **89 current semantic
actions**, their status, invocation, shortcut, UI/context exposure, undo and
persistence. Additional noncatalog features include clipboard formats, image
drop/import, note drafts, OS close, recovery, multiple windows and source supply.
Dedicated Pan/Rotate-view tool modes and configurable temporary tool combinations
are explicitly experimental; the existing pan/zoom gestures are implemented.

## Context menu and shared commands

`context_menu.rs` builds small lists on demand. Right click uses the same hit
geometry as left selection, including frame-border fallback. It preserves a
selected multi-selection/group, targets an unselected object, or clears selection
on genuinely empty canvas. Six context kinds select the relevant list.

| Context | Exposed operations |
| --- | --- |
| Image | Crop; horizontal/vertical flip; Nearest/Smooth/Default sampling; 100/75/50/25% opacity; four order commands; group/ungroup; relink; native linked-source open/reveal/copy path; delete; undo/redo |
| Multiple | Group/ungroup where image-only; Align with six alignments, distribute and pack; order; delete; undo/redo |
| Canvas | Paste, Import images, New note (click/drag), New frame, Select all |
| Note | Edit, text color/size/alignment/opacity, order, delete, undo/redo |
| Frame | Rename, Focus, order, delete, undo/redo |
| Annotation | Color, rectangle fill, stroke width, opacity, order, delete, undo/redo |

Menus have 18px logical rows, two-pixel padding, opaque flat colors, integer
coordinates, one immediate child submenu and bounded scrolling. Escape, outside
click, focus loss, activation and document-generation invalidation close them.
The outside click is consumed; it cannot also start a canvas drag. Right-arrow
opens a submenu and cannot accidentally activate Delete. Disabled rows are gray;
order endpoints, unavailable undo/redo, mixed grouping, empty Select all,
frame navigation and unsuitable fill/source commands use availability checks.

Shortcut labels read actual configured **press** bindings. Unassigned actions
have no invented label; remapped physical keys update the display. Keyboard and
menu actions go through `ImageInput::dispatch(ActionEvent)`, including pending
local operations handled by the existing native worker owner. No alternate
context-menu edit/history implementation was added.

Eight new catalog adapters expose **existing** `SetZOrder` and `SetOpacity` core
commands. Ordering preserves the selected objects' relative order and uses one
batch/history transaction. Frame chrome now iterates document order rather than
object-ID order. It still sits above images/annotations as an overlay; frame
order is not a filled-layer or container-membership feature.

A popup commits existing text drafts, cancels uncommitted captured canvas preview,
and clears held gesture captures without forgetting current modifier keys. Ctrl/
Shift changes while a popup is open are retained for subsequent canvas shortcuts.
Async changes invalidate the popup instead of executing a stale action on new
state. Simple platform cursors provide move/resize/rotate/crop/text feedback.

## Intentionally omitted / direct manipulation audit

No Duplicate command or add-selection-to-frame membership operation exists in
the current authoring architecture, so neither is advertised. No new ellipse,
rich-text editor, layer panel, online feature or GUI framework was introduced.
Canvas menus contain no file/preferences/debug dump. File commands live behind
the tiny application entry; Quit remains native OS/window-manager Close with the
existing Save/Discard/Cancel lifecycle. Pan/Rotate-view experimental tool modes
are not presented as finished menu tools.

Click selection, Shift selection, marquee, group selection, move/resize/rotate,
crop and deletion retain their existing geometry/dispatch. Frames remain spatial
regions and axis aligned; groups remain image-only. Annotation rendering and
image/annotation interleaving retain the established renderer. Native usability
of these paths on this build still requires the checklist below.

## Undo / Redo

Ctrl+Z invokes the existing document Undo, Ctrl+Shift+Z Redo, with existing Ctrl+Y
as an alias. Each menu edit enters the same dirty/recovery/history path. Continuous
move, resize, scale, rotate, crop and alpha previews do not mutate committed
history; release creates one entry. Existing gesture tests and a new 200-motion
physical-input drag test verify this behavior and exact restoration.

Notes have **no pre-existing local text undo buffer**. Text editing consumes Ctrl+Z;
Escape discards the draft, Ctrl+Enter commits a note, and document Undo then
reverts that commit. Frame rename commits with Enter. Opening a menu commits a
text draft before changing focus. No second text/document history was invented.
Undo history itself, selection, tool choice and popup state are not serialized.

## Low-resolution results / screenshots

[Compact receipts](../benchmarks/phase1h-ui.json) record **56 context renders and
48 existing-panel renders** from the production bitmap overlay and GPU renderer.
Software Vulkan used `llvmpipe (LLVM 20.1.2, 256 bits)`. These are offscreen images,
not native windows, hardware GPU tests or human observations.

| Display | Deterministic/offscreen result | Mandatory native/human result |
| --- | --- | --- |
| 800×600 | All six contexts/application placement, scrolling, child menus and modal rectangle bounds tested; 1×/2× pixel captures visually inspected and readable | Pending: isolated X11 server exited 1 before any Tack window could map |
| 1024×768 | Same bounds/renderer coverage and inspected application/multiple/keymap captures | Pending: same native-session restriction |

At 1×, the image menu measures 244×238 logical pixels; its sampling submenu is
100×58 and flips left at the bottom-right edge. At 2× these layouts remain fully
on-screen. 3× layout tests and 4×/8× render stress checks test containment and
scrolling, **not** comfort/readability; very large scale truncates text. Existing
modal panels no longer impose a 100px minimum exceeding small logical viewports;
compact layouts reserve reachable rows instead of overlapping footers.

Representative actual renderer captures:

- [Image + sampling, 800×600 1×](../benchmarks/phase1h-ui/image-800x600-1x.png)
- [Image + sampling, 800×600 2×](../benchmarks/phase1h-ui/image-800x600-2x.png)
- [Application + File, 1024×768](../benchmarks/phase1h-ui/application-1024x768-1x.png)
- [Multiple, 1024×768](../benchmarks/phase1h-ui/multiple-1024x768-1x.png)
- [Preferences, 800×600](../benchmarks/phase1h-ui/panel-Preferences-800x600-1x.png)
- [Keymap, 1024×768 2×](../benchmarks/phase1h-ui/panel-Keymap-1024x768-2x.png)

Raw PPMs and their checksums remain in ignored `benchmark-results/phase1h-ui/`.
The owned Xvfb attempt on `:94` reported `failed to bind listener` / `Cannot
establish any listening sockets`. It did not use the user's desktop, change a
window manager or start a GPU stress test. See the receipt for exact command/log.

Reproduce offscreen evidence without private images or network:

```bash
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
WGPU_BACKEND=vulkan XDG_RUNTIME_DIR=/tmp \
cargo run --release --locked --offline -p tack-app --example context_ui \
  -- benchmark-results/my-1h-ui
```

## CPU / RAM / idle impact

A closed menu owns no list/string heap allocation. It adds only an optional
popup pointer and small pointer/cursor state to the window owner. Menus allocate
on demand and release their owned lists on dismissal; there is no per-object
widget, font preload, offscreen UI surface cache, timer, animation or thread.
The existing CPU overlay vector/GPU buffers retain bounded high-water capacity
and are reused after closure; this is not a claim of zero retained renderer bytes.

Across these offscreen menu cases, developer accounting measured **520–1456
bytes** for menu struct + list capacities + shortcut string capacities. CPU
quad-vector payload was 15,360–30,720 bytes, including its pre-existing 128-quad
reserve; observed UI draws used 26–218 quads including the Tack entry. These
figures exclude allocator metadata, other canvas quads, GPU/driver allocation
and total process RSS. They are not a system memory budget or 128 MB readiness
claim.

Menu construction samples were 2.29–17.30µs and quad-build samples 3.09–29.49µs;
these are individual developer release measurements without confidence bounds.
They exclude `Context::selection`'s document-order availability scan. Ordering
commands also scan/allocate proportional to document order; no global optimization
or large-board constant-cost claim is made. The closed entry costs five quads
only on an already requested redraw, about 0.330µs mean over 10,000 redraw builds
on this host; it does not itself request periodic frames.

Native scheduling remains `ControlFlow::Wait` once supply/submissions/save settle;
static open/closed popup state adds no deadline. Opening, pointer/key input,
resize or document change requests redraw. This establishes the architecture;
**native idle CPU ticks, wakeups and total RSS have not been measured for 1H**.
1G native idle numbers belong to the prior binary and are not reused as 1H proof.
The existing `tools/run_idle.py` now supports isolated-display `--context-menu
canvas|application|closed|none` probes; help/Python checks pass, native execution
is pending. The [human checklist](HUMAN_TEST_1H.md) gives reproduction details.

## Tests / dependency checks

- Workspace/all-features checks, formatting, rustdoc and Clippy with warnings
  denied pass. **141 Rust tests** pass; six explicitly ignored GPU tests were
  subsequently run and passed on software Vulkan.
- **Nine Python tests** pass. The idle harness extension is help/syntax checked;
  these tests do not substitute for its blocked native execution.
- Eight new deterministic context tests cover object/multiple/canvas resolution,
  truthful availability, menu dispatch/Undo/Redo, actual/remapped shortcuts,
  scrolling/dismissal/edge placement, no Delete on Right, one physical drag commit,
  exhaustive five-object selection/order subsets, modifier/preview cleanup,
  order/opacity save-reopen, and compact existing-modal bounds.
- Five product GPU readback tests plus one GPU smoke test pass. Existing nonignored
  product geometry/grid tests are included in the workspace total. Integer glyph
  rows, annotation ordering and overlay layering remain covered.
- No dependency or lockfile change. Advisories, bans, licenses and sources pass
  against the **cached advisory DB** with a writable temporary copy. The normal
  check script reached its dependency step but failed to lock the read-only home
  cache; a separate offline metadata/cache run passed (advisory snapshot
  `ef6173cbc5c50ec8166f9a5b28f07834144373ee`, 2026-10-03). No network freshness or
  exact-commit remote CI result is claimed. Duplicate transitives remain warnings.

Raw validation logs are retained in `benchmark-results/phase1h-validation/` and
hashed in the compact receipt. No brittle golden-image test for each menu exists.

## Human validation / remaining UX work

[HUMAN_TEST_1H.md](HUMAN_TEST_1H.md) describes the required uncoached import →
move/resize/rotate → context edit → Undo/Redo → group/align → blank creation →
save/reopen flow. **Not run by a human or in a native 1H window in this session.**
Automated semantic/storage and offscreen coverage are narrower evidence.

Pending acceptance: native 800×600 and 1024×768 with actual handles, menu click
routing, modal dialogs, OS pickers, DPI changes and readable text; a new-user
observation; open/closed native idle/RSS; real Windows clipboard/picker/display
checks. High requested scale truncates labels on small displays. Shortcut labels
are physical-key names, not localized keyboard-layout captions. The existing
plain-note editor remains intentionally limited. The menu's helpfulness and
canvas feel require the uncoached user test before asserting discoverability.

## Human binary provenance

`./bin/tack` is the atomic successful release build left for immediate testing;
never use the unrelated system `/bin/tack`. Rebuild with
`bash tools/build-test-bin.sh`. The checked build stamp is:

```text
STATUS: CURRENT
commit: fae987c1f9006881b846b3ac171c6f10faa5a686
profile: release
built_utc: 2026-10-05T21:42:37Z
binary: ./bin/tack
worktree_dirty: true
sha256: 918843973e6e28240add6bfb7fe551200382b32dd81fa386314c5ec02bcec091
checkpoint: phase 1H — contextual UI and shared Undo/Redo; native validation pending
```

The dirty flag is essential: this is **1H worktree runtime**, not the old HEAD
runtime. ELF size 114,790,776 bytes (debug info); a developer `/tmp` fully stripped
copy is 17,794,216 bytes. The installed binary is left unstripped. Binary and
source/evidence checksums are recorded in the receipt (187 public source files);
documentation-only edits
do not change runtime. User untracked `gfx` files are untouched and excluded
from the source snapshot/suggested commit.

## Git status / handoff

HEAD remains the already-pushed 1G measurement commit
`fae987c1f9006881b846b3ac171c6f10faa5a686`. Mission 1H changes are local and
reviewable. Existing unfinished 1G reports/receipts are preserved separately;
none of the user's new bitmap/font/image files was edited or staged.

This session's permission profile explicitly makes `.git` read-only and blocks
terminal network; approval escalation is unavailable. Commit/push therefore
cannot be reported complete. `git add` failed (exit 128): `Unable to create .git/index.lock: Read-only file system`.
A precise 1H file list, patch and source archive are retained in ignored
`benchmark-results/phase1h-handoff/`; source/receipt hashes are recorded above. No Git lock/permission bypass was
attempted. Once a normal writable/network-enabled session is available, commit
these changes, run the configured CI on that exact commit and push. Rebuild the
stamp afterwards so its HEAD describes the committed source.
