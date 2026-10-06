# Updated Phase 1G — independent verification, 2026-10-06

Verifier: `/root/phase1g_updated_verifier`, independent of implementation,
read-only; no concurrent GPU runs or repository mutations by the verifier.
Final verdict: **B — automated local/native gates complete; human review pending**.

No unresolved concrete product correctness issue was found. The verifier reviewed
high-LOD/stale-result publication, projected-edge selection, per-source fairness,
bounded admission/cache/worker behavior, idle, larger boards, presentation tails,
potato pressure, instance isolation, Save/recovery, low-resolution evidence,
dependency/binary impact and provenance.

The verifier identified two test-evidence mistakes: the initial 2× probe selected
1×, and an extra Down selected Undo instead of Keymap. Both were corrected before
acceptance. The final context harness asserts scale 2 and a visibly opened Keymap
panel; the verifier inspected the actual 1024×768 Keymap capture.

Independently recomputed and checked:

- 26 native supply runs at actual 800×600, 1024×768 and 1600×900 client sizes,
  including all cache/request peaks and distribution summaries.
- 30 contextual assertions, 21 production and 19 recovery assertions; picker
  ownership/geometry, real 1×/2× capture labels and excluded failed harness attempts.
- Six settled supply cases, six matched menu/runtime idle observations, both
  simultaneous-instance pairs and retained disk-cost evidence.
- 144 Rust/workspace/doc-test passes, nine Python passes, eight explicit GPU
  executions (two overlap workspace coverage), fresh dependency checks.
- The 186-file source archive byte-identical to the source worktree,
  raw evidence hash/size inventory and current binary hash. The final inventory
  contains 1012 files after adding the successful latest CI observation.
- Paired before/after medians: source serialization delays single-source first
  detail by 37.491 ms in three pairs; sparse first detail improves by 19.995 ms.
  These small samples do not establish a universal speedup.

Measured executable SHA256:
`4d21e4ea6322f9898abf82682513d9339cd9b0f1abd15ac894d1d76a697af6ee`.
Source snapshot commit: `b519b7fc5fdc7aabe2df415c71e0d15fa42df4f6`.
Source ZIP SHA256:
`ba0b4aa412ecea5ab3bb8a01dc488ea4e0c63bbf9bcb604ce5c869ff88d28cd5`.
Runtime Rust/configuration remains at `76e67c8`; subsequent source commits change
only the native test harness. The final repository-local binary rebuild is
byte-identical; its final commit/timestamp are recorded in `./bin/BUILD.txt`.

Configured Linux, Windows and dependency Quality jobs are successful for
[76e67c8](https://github.com/gurppt/tack/actions/runs/37380755187),
[deeb0fe](https://github.com/gurppt/tack/actions/runs/37420440802) and
[b519b7f](https://github.com/gurppt/tack/actions/runs/37421904943).
The last observation is retained in the current receipt; earlier in-progress
snapshots are historical evidence, not the final gate result.

Native automation proves functional paths, geometry and idle behavior. It does
not establish uncoached human comfort/discovery, physical-monitor latency/DPI
switching, native Windows desktop behavior or 128 MiB/Pentium III readiness.
Keymap physical/matcher labels can truncate; this limitation is recorded in the
current report. Whole-process RSS/driver activity, dense-overlap GPU cost,
presentation tails and retained container disk allocation remain explicit.
Stop for human review; no Phase 2 or final global optimization was started.

Current details: [mission report](MISSION_1G_REPORT.md),
[compact evidence](../benchmarks/phase1g-updated.json).

---

## Historical original 1G review — preceding runtime

The following review uses the original `13b294…` executable. Its Git/network/CI
blockers were subsequently resolved; its measurements are not attributed to the
current `4d21e4…` binary.

# Phase 1G — independent verification

Verifier: `/root/phase1g_verifier`, independent of the implementation, read-only.
Final verdict: **B — local work complete; execution-profile gates pending**.

No concrete unresolved correctness issue was found. Reviewed high-LOD/crop/
sampling identity, source/revision obsolescence, queue replacement and priority,
cache/potato admission, suspended/shutdown drainage, ordered visibility memo and
invalidation, Save/recovery and retained originals. Development findings were
corrected before final review.

The verifier independently recomputed:

- All12 supply report checksums and GPU peaks,6 final idle reports,4 matched1F
  baseline reports and both pairs of native instance receipts.
- All21 production and19 recovery checks marked true, plus the exact binary and
  build stamp.
- The182 archived code/tool/asset/config files byte-identical to the worktree,
  778 raw artifacts with matching hash/size,4 quality-log hashes,133 Rust passes.
- The paired fixtures' identical hashes and query p99 changes1.026→0.077ms /
  11.398→0.434ms at5k/50k.

The final report accurately retains measured active-board RSS/driver idle-CPU
increases, overlapping-image GPU cost, synchronous storage-only timing, per-session
cache/reopen behavior, memo rebuild costs and absent human Linux/Windows claims.
The source count typo183 was corrected to182. No files were changed or GPU
benchmarks launched by this verifier.

Final source ZIP SHA256:
`023c88bb5698d273fcff8ac3555661795ca9b87b36eda8a8562095097ab7254a`.
Native binary SHA256:
`13b294987327090b38c563001965b902c1a860529f30688ebd64d87d071a4a71`.
Build stamp (rebuild after review; identical binary): `fae987c1f9006881b846b3ac171c6f10faa5a686`, release,
`2026-10-05T20:39:56Z`, dirty true.

A — PASS remains conditional on actual final success of Quality for `fae987c`
and saving the completed documentation. `a1ef665` is verified green but is not
represented as verification of the later commit. At final review the resumed
session cannot retrieve network CI status or write `.git`; neither is a product
correctness finding. Stop for human review; no Phase2 was started.
