# Independent verification — Phase 1I

2026-10-06. Independent read-only verifier `/root/phase1i_verifier`, separate
from implementation. **B — implementation and targeted native correctness PASS;
owner comfort/discovery and real Windows desktop review pending.** No unresolved
correctness defect remains in the audited scope. Exact implementation CI [Quality 37429790856](https://github.com/gurppt/tack/actions/runs/37429790856)
is green for `fd687d31789d9528d090eabe1aca0eee9ea1f066` (Linux, Windows,
dependencies); its receipt is recorded separately
in [the phase report](MISSION_1I_REPORT.md) and [JSON evidence](../benchmarks/phase1i.json).

## Audit and attempted falsification

- Reproduction: old native FR physical W/logical Z failed keyboard Undo; native
  menu Undo worked. Actual physical/logical/modifier/normalization traces support
  the root cause; the fix does not depend on a French-specific mapping.
- End-to-end: normalized AZERTY owner tests, physical-Z/logical-W negative cases,
  actual native keyboard/menu actions and saved document equality prove shared
  owner/history mutation rather than only matching a binding.
- Input model: bounded fixed-size logical/physical identities, repeat suppression,
  captured physical release, modifier order/focus; menu labels are shared/truthful.
  Conservative domain conflict and v1 unsupported-profile behavior are explicit.
- Tools: Note/Rectangle/Line/Arrow/Frame return Pointer without another history
  entry; Freehand persists. Cancellation is safe; nested temporary Pan/Text on
  a Pan base cannot leave an orphan override after modal reset.
- Arrangement: five bounded candidate passes plus sort/indexed group lookup,
  O(n log n); AABB translation preserves sizes/rotation/groups; one batch gives
  exact Undo/Redo and native reopen authority. Independent base-64 Snap is honest
  about potential overlap and adaptive-dot differences.
- Native: 170 assertions across US/FR × 800×600/1024×768 all pass. 800 Arrange and
  real Keymap captures were visually inspected; no human comfort claim is inferred.
- Cost: layout median/history/allocator numbers recomputed; 12 matched supply
  runs and four settled idle cases show no material regression in these samples.
  Driver wakeups remain; no whole-process zero-wakeup claim. Trace-disabled 10k
  resulting-redraw probe passes exact native Undo/Redo and has four visible
  annotation primitives; it is not 10k-visible GPU cost.
- Budget/provenance: 276 locked packages unchanged; stripped +103,616 bytes
  (+0.581956%). 191 source-file hashes and all 189 accepted raw-evidence hashes
  match the current artifacts. Verified native/runtime SHA is `56ffdaf83a6f4678e20127c887cf2b64788b7ce03cb87bbc2fe34a5facc4ab90`.

## Findings corrected during review

1. Releasing a second physical key aliasing a held logical key could release the
   first key's action. The owner now ignores releases without captured physical
   identity; Enter/NumpadEnter regression coverage was added.
2. An initial native reopen assertion reread disk rather than proving state from
   the reopened owner. It now saves from the reopened native session and compares
   exact document authority, and checks its renderer error count.
3. Two documentation phrases conflated modifier updates with focus draining and
   temporary-stack clearing with held-input clearing. The corrected policy says:
   focus drains; modifier/layout changes retain captured release; one-shot
   completion clears overrides; modal reset additionally resets held input.

Final semantic review also records the punctuation limitation: “logical” is the
unmodified base character, not Shift/AltGr text. Defaults with absent base
punctuation on French layouts may need remapping/menu. Core native shortcuts
remain proven; punctuation availability is an owner ergonomics review item,
so no unqualified A acceptance is awarded.

No shipping code was edited by the verifier. Earlier exploratory failures and
older native binaries are excluded from final acceptance evidence. The owner
should run [the human checklist](HUMAN_TEST_1I.md); native Windows desktop,
legacy hardware readiness and the final global performance pass remain separate.
