# Bounded continuation after physical Gate B

This is a follow-up scope for P0-WIN-IMAGE-CORE-02, not a claim that stages C–G are delivered. First inspect the private Stage B Windows report; if native actions still fail, isolate that failure before broad changes. Do not update the installed/public build.

## C — truthful import failures, separate commit

Retain the worker channel capacity=1 and UI notification path. Introduce a typed failure record with independent stage/category and optional safely obtained encoded bytes, dimensions and codec/layout. Wrap actual operations at their boundary rather than inferring stage from translated text. Preserve raw OS codes for local UI; diagnostics may contain only structural metadata and path IDs. Categories: path/open, fingerprint, sniff, metadata/header, format/variant, encoded admission, staging/disk, source-changed, cancelled; decode/preview is a separate later worker state, not successful admission.

Keep a fixed-size category counter array and a bounded sample (e.g. 16 failure records, bounded basename/reason lengths). Do not retain 4096 arbitrary full error strings. User can see the representative filenames/reasons and counts; the diagnostics must retain the exact first boundary, not claim all batch failures share one cause. A bounded detail panel can give precise samples while summary remains compact. Current `ImportUpdate::Failed {path,message}` is discarded in updates.rs: replace that consumer in this commit only.

Copy progress must be actual copied bytes emitted by the existing worker, throttled by bytes/time while copying, with cancellation even when channel is full. Existing `bytes` means completed embedded staging, not live per-file percentage. Do not add an idle polling timer or a spinner per dormant image. Use indeterminate preparation text for decode. Handle all-failed/empty/cancel and new-batch reset.

Tests: table of missing path, denied read, >64 MiB known valid source, huge progressive variant, PNG malformed/interlace rejection, changed source, disk full and cancellation. Assert phase/category and privacy, bounded 500 failures, actual bytes/cancel behavior and settled idle. Current source-contract witness for UI loss remains failing until C is actually implemented.

## D — bounded offline source admission, separate format-sized stages

Re-audit all limits from current HEAD, including representation/native_thumbnail 64 MiB, jpeg_header streamed encoded budget **256 MiB**, PNG cumulative read 256 MiB, header/row/progressive scratch, local spool 2 GiB batch and 4 GiB session, all u64 payload offsets and lengths. 1.33 + 1.478 GB exceeds the 2 GiB batch independently of per-file admission. Do not mix online 512 MiB original/cache limit into offline policy.

Start with genuine 8000×9000 grayscale baseline JPEG (113,652,624 bytes) and noninterlaced PNG (72,034,446 bytes), hashes in Stage A receipt. Keep heavy fixtures outside Git/CI. Ordinary under-limit path must retain its dispatch and bounds. Route admitted large source through bounded header → overview → demand detail/tile generation → cache/source revision → UI presentation → Embedded save/reopen integrity. Prove this before increasing the import limit for that particular format. A decoder budget rejection is an exact supported/unsupported outcome, not a successful render.

JPEG: existing single-scan baseline streaming can be extended only after end-to-end read caps/resource measurements. Do not allocate the entire compressed body for native_thumbnail. Progressive/multiscan giant images require a measured coefficient-storage/native-codec strategy; stop at ADR/prototype if it is a substantial redesign. Test actual Photoshop Save/Export headers, ICC/orientation, malformed marker/truncation and source mutation. No 1.478 GB claim from a header check alone.

PNG: stream filtered rows within configured scratch and cumulative encoded budget; alpha/16-bit correctness and interlaced behavior need separate cases. No random-access claim for DEFLATE. Native high-zoom default remains gated on physical Win10/resource evidence; `--huge-tiles` can stay explicit during experimentation.

Record first preview/usable detail/full zoom, peak private/PSS, configured CPU/GPU budgets and disk scratch, source read/write bytes, worst active frame, cancel/close/disk-full/source-change result and settled idle. Compare the existing 20-image control and reused-source 1k/4k/10k object boards with unique source counts explicit. Do not regenerate icon assets or delete production caches to make the measurements fit.

## E/F/G boundaries

GIF: first-frame poster only if a bounded, correct palette/transparency/disposal representation fits the shared supply contract; otherwise deliver a measured codec-specific ADR. No animation timer or eager all-frame parsing. Large Embedded Save: architecture/prototypes on owned files only; no schema change, permanent external blob dependency, weakened sync/CRC/atomic replacement or acknowledgement. Compare whole-container metadata fast path, immutable blob + generation metadata, and transactional COW while preserving standalone files and recovery. Future online authority/streaming optionalization is an ADR, not a premature server/Manager rewrite.

G: physical Windows 10 ordinary file actions, exact large JPEG/PNG rendering outcomes, correct unsupported-variant UI, Save/reopen hashes, ordinary baseline and Start Sharing after picker. No public release until the original brief's final veto is satisfied.
