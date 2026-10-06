# Independent verification — Phase 1J

Read-only review by `/root/phase1j_verifier`, required by the Phase 1J brief.
Implementation commit: `38529aa5262bcf02ffd7dd67fcb6f24091c4c489`.
Tested release binary SHA256:
`418d1fa7ea66e87c79eaaf3e178af37d2b9b5156768fe2556e6d6caa3d49ac18`.

The reviewer independently audited source, native receipts, screenshots, raw
performance-report hashes, idle summaries and ELF section identity. It reported
no unresolved correctness finding after corrections. This is technical review;
owner aesthetics, normal desktop producer review and real Windows/Wayland
clipboard runtime remain unclaimed.

- Controlled helper-missing reproduction retains the original opaque error.
  Final 27 native assertions and actual Dolphin/screenshot receipts support
  bounded MIME-priority admission, single/multiple PNG/JPEG, mixed-entry rejection
  summaries, ordinary/editor text and late-cancellation staging cleanup. Helpers
  run on demand with bounded nonblocking reads and owned-process-group cleanup;
  no resident clipboard polling worker remains.
- Every context prepends the same application descriptors as F10 and dispatches
  through existing Actions/history. Removed button does not remove commands.
  Native 800×600 1×/2× screenshots and pointer tests show bounded child/parent
  access. Keymap reachability is proved by unassigning/restoring Paste, not merely
  by capturing a screenshot.
- Direct scale increases/decreases and persists. Exactly three themes persist;
  old profiles default safely, future enum rejection preserves profile authority,
  and theme/scale changes do not mutate saved board bytes. Enabled/disabled
  palette text contrast tests require 4.5:1/3:1 respectively. Native screenshots
  show readable Light notes, bitmap menus and nearby monotone screen gradients.
- Twenty-four paired RTX 2060/Vulkan reports and 2696 measured samples are
  consistent with the reported table. Largest fixed grid-hidden GPU increment
  is 23.024 µs; grid-visible and CPU costs show no material regression on this
  hardware. GPU timestamps are separate from CPU submission/presentation.
- Nine idle windows show zero app redraws, GPU submits, I/O, main-thread activity
  and RSS growth. Driver housekeeping is observed and distinguished from app
  timers/polling. There are no new idle worker/helper leaks.
- Independently parsed ELF `.text` hashes match shipping and stripped current
  executables. Stripped delta is +23,296 bytes (+0.130%); zero new Rust packages.
  Local gates pass 163 Rust tests, 9 Python tests and 1+7 explicit hardware GPU
  tests; ordinary ignored GPU cases are separately exercised.

Exact implementation [Quality CI](https://github.com/gurppt/tack/actions/runs/37523458475)
passes Linux, Windows and dependency jobs for the implementation SHA above.
Compact reproducibility manifest, raw-report hashes and selected capture hashes
are in [phase1j.json](../benchmarks/phase1j.json).

Findings corrected during review include low-contrast Light notes, disabled
selection contrast, parent/child overlap at 2×, stale mixed-import summaries,
clipboard cancellation after publication, misleading plain-text path detection,
and a pure-Wayland missing-helper message. Some other failing operations may
retain bounded staging until next paste/session cleanup; this is documented and
never becomes source authority. Private Flameshot activation failure remains
recorded as failed; the successful screenshot proof uses actual owned-window
capture plus X11 clipboard. No unperformed producer pass is asserted.

Conclusion: **technical verification passed; owner review pending**. Final
delivery atomically stamps the identical tested binary with the final documentation
commit. No Phase 2 work was started.
