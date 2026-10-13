# Stage C — truthful bounded import failures

First boundary now survives worker → UI: static stage/category/code, actual
encoded byte count and safely known dimensions, applicable byte limit and native
I/O code. Missing paths, permissions, empty/truncated input, unsupported formats,
large JPEG scan variants, interlaced/animated PNG, independent encoded/staging
budgets, source mutation and cancellation have separate curated reasons. Unknown
provider error strings are not displayed or logged verbatim: they may contain
private paths. File names in the details view are bounded to 64 characters and
strip native/foreign parent paths and controls.

One lazy failure summary appears after import ends: per-category totals and at
most **16** filename/reason samples, with an explicit omitted count. Remaining
admitted images are retained on cancellation. New import/install resets the
summary. Clipboard omission counts remain separate from actual image failures.
No animation, polling timer, per-image spinner or permanent worker is added.
Ordinary successful imports allocate no failure summary.

Copy updates come from real writes: 128 KiB scratch remains unchanged, updates
are throttled by **4 MiB and 100 ms**, final completed byte count is preserved.
This is copy activity, not fictional decoder percentage. Existing one-slot local
worker channel remains bounded and cancellation rolls back a partial spool append.
Encoded admission still uses 64 MiB in this commit; no codec/storage limit raised.

Representation errors after admission are a distinct `decoder/preview` stage.
Existing ProductAssets retains at most sixteen lazy, curated failure receipts;
Image information shows the latest retained reason. Lookup requires the current
asset revision, so a repaired source cannot inherit its stale failure. No raw
path/provider error is retained; ordinary supply gains only an optional pointer,
with no allocated collection until an actual failure. This is bounded sample
retention, not a promise that every historical LOD failure remains available.

Opt-in DEV records include stage, category, code, exact byte count, dimensions,
limit, duration, native code and hashed path/error correlation. A bounded header
inspection supplies JPEG frame/progressive/precision/scratch or PNG IHDR bit
depth/color/interlace. PNG diagnostic IHDR fields are explicitly header-only;
admission still uses the validated parser. This extra read is performed only
with diagnostics enabled, never at ordinary startup/idle, and inaccessible
path/fingerprint failures do not trigger another header access.

## Validation

Private receipts: `benchmark-results/p0-win-image/stage-c/`.

- Source-contract witnesses: **2/2 pass**; UI previously discarded both reason
  and path, and the legacy generic unsupported/remote label is removed.
- Real worker/library/local-production run: **99 passed, 0 failed, 3 existing
  explicit GPU/resource ignores**. Includes valid tiny GIF unsupported by the
  current decoder, missing source, pinned spool, dedup, cancellation, save/reopen,
  recovery and shared-boundary tests. No ignored test added or weakened.
- Failure mapper/privacy/500-item bounded summary: **3/3 pass** (latest targeted
  run). Windows-style parent names cannot leak into Linux details.
- Actual supply missing→repaired-revision test: **1/1 passes**, confirming the
  stale failure receipt is hidden and fresh pixels can be supplied.
- Full Linux workspace/all-targets/all-features release Clippy `-D warnings`
  passes; fmt and diff checks pass. Final Windows target lint/build, ordinary
  before/after idle/binary measurements and new GUI acceptance are included in
  the next candidate rather than falsely claimed from these focused tests.

No external dependencies, `.tack` schema, durability, LAN protocol or installed
binary changes. Native dialog B1 physical result is in
[STAGE_B1_PHYSICAL_ACCEPTANCE.md](STAGE_B1_PHYSICAL_ACCEPTANCE.md).
Next: independently bounded admission/rendering in D; do not equate metadata
admission with successful display or multi-GB support.
