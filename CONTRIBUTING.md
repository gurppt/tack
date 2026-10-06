# Contributing to Tack

Tack is a native board application under active development. Read
`docs/architecture.md` and the current phase report before changing it.
The local briefs are normative but intentionally excluded from Git.

Correctness, clarity, measured performance, maintainability, then small
reviewable changes. Keep geometry independent of GPU/window/storage APIs.
The frame/event path must never wait for image decoding, filesystem access,
worker completion or a long lock. Queues, caches and worker concurrency are
bounded; account for scratch space and in-flight GPU work as well.

Disk space is limited. Inspect available space and existing build/test data
before a large build or benchmark; keep at least 10 GiB free and stop new
large writes before reaching that reserve. Use the shared target directory,
not a fresh target per experiment. Keep reduced development debug information
and incremental builds; preserve useful release builds and native libraries.
Do not copy a binary or an image corpus into every test run. Reuse fixtures
and one baseline binary per phase. Bound generated data and remove disposable
caches, duplicate captures and superseded build artifacts after verification.
Retain compact raw measurements, reports and necessary reproducible evidence;
never discard the only proof of a result. Prefer a 512 MiB generated-data
budget per phase; document any necessary exception before exceeding it.
Cleanup must target known generated paths, never user files, briefs, source,
profile data or board assets. Inspect first; avoid broad home/tmp/cache wipes
and full `cargo clean` when selective cleanup suffices.

Run `bash tools/check.sh` with the pinned toolchain and cargo-deny 0.20.2.
Tests for renderer policy should run without a GPU; explicitly run the GPU
smoke test on a supported adapter when changing rendering. Keep raw frame
telemetry for performance changes; do not infer GPU timing from CPU submission.

No unsafe Rust, no panics for external failures, no speculative frameworks.
Review files approaching 500 lines/functions approaching 50 lines for cohesion.
Delete superseded paths and unused dependencies. Non-trivial changes need an
independent review. Commits should state the subsystem and concrete change.

The application's license and external contribution terms have not been
decided. Substantial outside contributions are deferred until that decision;
dependency license checks do not grant a license to Tack-owned code.
