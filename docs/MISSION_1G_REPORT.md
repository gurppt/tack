# Mission 1G — local supply/performance consolidation

Status: implementation complete; native stress receipts, independent final
verification and configured CI are being collected. **B — validation pending**.
No Phase 2 or final global maximum-performance branch was started.

## Changes

Product views now supply projected 128/512/2048 image representations off-thread,
including crop demand, obsolete-view rejection, source-shared detail and stable
budget admission. Dense views can reduce overview edges to 8px. Replaceable
queues have no hidden worker FIFO; potato constraints preserve graceful quality
fallback. Hidden-window and shutdown queues revoke demand instead of churning.

Measured 50k repeated-query cost justified a bounded ordered AABB memo above
4096 objects, preserving document order and live selection previews. It is not
a spatial index. Edit/document replacement rebuilds remain synchronous and are
explicitly separate from stable camera-pan gains.

[Supply, fairness, bounds and timing contracts](design/local_image_supply.md).
[Human Linux checklist / Windows manual scope](HUMAN_TEST_1G.md).
A successful repository-local build atomically installs `bin/tack`; `bin/BUILD.txt`
records commit, dirty state, profile, timestamp and SHA256. `bin/` is ignored.

## Validation in progress

Generated fixtures cover 1k/5k/50k image objects over 64 distinct generated
1600×1000 sources, 10k shapes, mixed annotations/notes/frames/group, sparse zones
80 million units apart and an overlapping high-LOD pressure case. The shared
source corpus is an explicit limitation, not proof of 50k independent originals.

Before/after bounds-memo raw receipts and exact binary hashes are preserved in
ignored `benchmark-results/phase1g-supply-final` and `phase1g-supply-query`.
The latter measured query p99 0.077ms at5k /0.434ms at50k, versus approximately
0.97ms /12.1ms before. First memo construction at50k still cost10.26ms. Xvfb
present remains the dominant callback component, around37–48ms; acquire/encode/
submit/GPU pass are independently recorded. No monitor-latency claim.

Previous native 1F production/recovery checks passed 21/19 on the intermediate
runtime. Final-runtime coverage, multi-instance edits/recovery, dense idle,
quality/CI, exact build stamp and anti-bloat audit will be recorded before PASS.

## Stop

Stop after 1G for human review. No server, remote sync, account, daemon, media,
plugin, `.pur`, rich text or ellipse branch was introduced.
