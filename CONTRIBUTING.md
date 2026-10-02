# Contributing to Tack

Tack currently contains a renderer experiment, not the product. Read
`docs/architecture.md` and `docs/MISSION_0_REPORT.md` before changing it.
The local briefs are normative but intentionally excluded from Git.

Correctness, clarity, measured performance, maintainability, then small
reviewable changes. Keep geometry independent of GPU/window/storage APIs.
The frame/event path must never wait for image decoding, filesystem access,
worker completion or a long lock. Queues, caches and worker concurrency are
bounded; account for scratch space and in-flight GPU work as well.

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
