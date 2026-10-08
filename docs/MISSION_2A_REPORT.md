# Phase 2A — optional native LAN collaboration

2026-10-08. Implementation: `b55bf390ace5462f5408eba3f671a55029fe4973`.
**Technical PASS**: Linux source, native, GPU, performance, local-first regression,
independent review and exact-commit Linux/Windows/dependency CI gates pass.
Subjective artist-feel and real multi-computer LAN testing remain human review.

## Delivered behavior

An explicitly started, headless `tack-server` hosts shared boards. Publish
preserves the existing local board, stable IDs, layout and original bytes.
Join opens the same native canvas and tools against server authority. Ordinary
local startup creates no collaboration service, network socket, LAN thread,
discovery task or heartbeat.

```sh
./bin/tack-server --root /ABSOLUTE/PRIVATE/DIRECTORY --listen 127.0.0.1:7337
./bin/tack publish /PATH/LOCAL.tack 127.0.0.1:7337
./bin/tack join 127.0.0.1:7337 BOARD_ID_FROM_PUBLISH
```

Use an explicit numeric LAN address for other computers. This foundation has
no authentication or TLS and is intended only for trusted networks. See the
[hands-on checklist](HUMAN_TEST_2A.md).

`DocumentEditor` is the existing semantic action boundary: local editors apply
their normal commands/history; shared editors queue bounded requests and display
only accepted durable commands. Camera, selection, tools and preferences remain
local. There is one semantic model, renderer, annotation implementation and
codec/supply stack. A metadata-only local supply view resolves verified CAS
paths; it never replaces canonical editor authority.

The server serializes edits against their exact observed base revision. Stale
edits and conflicting undo fail visibly. Each completed manipulation is one
semantic transaction; existing transient previews are not durable history.
Remote edits cancel active drafts and unsent older requests visibly. Shared
undo/redo submits a new inverse operation; it cannot overwrite intervening work
from another writer. Original content is deduplicated by SHA-256 rather than
filename. Source revision high water survives undo and reconnect.

TCP-reported loss makes shared editing read-only. F5 installs a validated fresh
snapshot and discards obsolete pending work; ambiguous durable requests are not
automatically replayed. There is no offline merge. Without a heartbeat, silent
cable loss may remain undetected until an operation/reconnect reaches the TCP
stack; an unacknowledged request never changes durable client authority.

The reverse shared-to-local workflow is implemented and tested as the streaming
`publish::save_local_snapshot` storage API. Its desktop action, shared Save
Original As and arbitrary-source export are explicitly deferred. Existing local
Save/Save As, recovery and original export remain independent.

## Tested guarantees and independent review

`tools/check.sh` on pinned Rust 1.95.0 / cargo-deny 0.20.2 passes formatting,
all-target/all-feature check and Clippy, **260 Rust tests**, documentation,
**26 Python tests**, and advisories/bans/licenses/sources. Duplicate transitive
version warnings remain informational. **11 explicit GPU tests** pass on the
RTX 2060, including native renderer readback and LOD convergence.

Tests cover all existing durable command DTO variants and document kinds,
framing/version/truncation/lengths, malformed IDs/geometry, remote path refusal,
duplicates, stale edits, inverse conflicts, revision reconciliation, streaming
chunk/hash/quota/cancellation, cache corruption/eviction, source-binding forgery,
persisted revision floors, history eviction and reverse snapshots. Actual TCP
process integration exercises three sessions, both edit directions, restart,
duplicate receipts, deduplication and safe replacement of a half-open session.
Fault injection covers directory-sync failure after authority/CAS rename.

`native-03` passes **32 assertions** with three simultaneous real Linux/X11
shared windows and a fourth independent local window. A/B/C create and edit
notes, rectangles and image geometry; undo/redo, conflicting undo, relink → undo
→ F5 → relink, restart/rejoin, duplicate-content imports and two rapid imports
of the same path are exercised. A remote edit during a real mouse grab cancels
the stale draft visibly. Offline edits do not merge. Actual window pixels prove
image supply in every shared client, before and after these operations.

All three final native documents equal persisted server authority at revision
15, with 9 objects and 4 source descriptors:
`2442195e2c628db1f394091f24483ccfbce161bb5be6ed1149b2b181d36d011a`.
All three have zero errors, missing, unavailable or foreign sources.

Independent authors cross-reviewed the protocol/client, native adapter and
server publication. Their [native/client review](reviews/phase2a_native_client.md)
and [protocol/client review](reviews/phase2a_protocol_client.md) identify their
authorship exclusions and record the concrete fixes and final evidence. No
concrete correctness blocker remains in those reviewed scopes.

The first native run proved semantic convergence but missed failed image supply:
a generated storage lock sidecar was rejected by the original cache. That run
is retained as diagnostic evidence, **not** final acceptance. Strict bounded
sidecar handling, lifecycle tests and actual-pixel native assertions fixed the
gap. Deferred originals now admit no codec jobs or render retry loop before
verification. Startup cleanup also preserves/refuses unknown or non-regular
files instead of deleting suffix-matching owner files.

## Measured performance

Raw receipts are retained under `benchmark-results/phase2a/`; compact results,
raw-file SHA-256s, source manifest and binary provenance are committed in
[phase2a.json](measurements/phase2a.json) and
[phase2a-source.json](measurements/phase2a-source.json).

Measurements use serial native 800×600 windows on an owned isolated X11 display
`:99`, NVIDIA RTX 2060/Vulkan, Linux and loopback. OS page cache is warm or
unspecified. Traffic counts actual framed protocol bytes, not IP packet overhead.
This is not a physical LAN bandwidth, physical cold-I/O, Windows desktop,
end-to-end input latency or low-end-memory claim.

| Measurement | Observed result |
| --- | --- |
| Idle traffic, each of three native clients | 0 bytes / 0 messages in each approximately 5-second interval |
| Server idle, 1 / 2 / 3 clients | 0 CPU ticks; 21.07 / 22.11 / 23.15 MiB RSS; 3 / 5 / 7 threads |
| Server idle scope | Both generated boards already loaded; 0-client RSS 21.03 MiB, one listener thread |
| Shared native clients | About 337 MiB RSS, 45 threads including inherited GPU/runtime workers; idle 0–2 CPU ticks over 3 seconds |
| Completed one-object semantic manipulation | 290 bytes request + 387 bytes acceptance; one message each way |
| Completed ten-object semantic manipulation | 1,731 bytes request + 1,846 bytes acceptance; one message each way |
| Note edit | 255 bytes request + 345 bytes acceptance; one message each way |
| Original upload | 4,561,776 bytes in 0.109 seconds: 39.92 MiB/s on loopback; repeated hash deduplicates |
| Rejoin across 101 accepted revisions | One 2,168,647-byte metadata snapshot; 14.50 ms; no durable replay |
| 100 edits on 10k shapes + 100 notes | Acknowledgement p50 34.04 / p99 42.80 / max 58.37 ms |
| Native interaction during synchronization | 40 accepted edits while panning/zooming; all clients converge; wheel callback p99 0.00385 ms |
| Missing shared original | 1k objects remain navigable; explicit error; zero CPU ticks, I/O and redraws over the measured 3 seconds |

Manipulation traffic is isolated with a protocol peer emitting the same completed
semantic operation; the native harness separately proves real drag transaction
behavior. Input measurements are callback-processing proxies: the native report
also includes other window callbacks outside the concurrent interval. Its
overall event p99 is 25.72 ms, including presentation; it is not physical input
latency. Driver background context switches are recorded separately from the
blocking main/LAN workers and do not imply a collaboration heartbeat.

The generated shared image fixture has **1,000 image objects using one original**.
The second board has **10,000 rectangles and 100 text annotations**. A real native
client opens/rejoins its 10,100 annotations at revision 101. These are metadata
stress counts, not a claim of 1,000 unique source downloads, simultaneous rendering
of every annotation or 50k readiness. Native duplicate imports and missing assets
are checked separately. Full metadata rewrite costs approximately 219 MB of
process writes for the 100-edit stress run; a bounded journal is deferred.
Hex chunks double encoded payload traffic. Neither tradeoff is hidden by the
loopback throughput figure.

## Local-first comparison and component cost

The baseline is the preserved final 1L executable, implementation
`57dea005b398c4fccd3f2f09b3f385c4223b311b`, stripped SHA-256
`b733f0e98e882359ac52ebc860ec7157985b0aa2d971c3d7885772d468081e92`.
Both versions run on the same display/machine/window size. Eighteen serial runs
cover three paired repeats per version on each board. An external bounded quiet
detector precedes each three-second idle sample; all runs reach quiet state.

| Local board | First frame median, 1L → 2A (ms) | Idle RSS median, 1L → 2A (MiB) |
| --- | ---: | ---: |
| Small linked board | 456.14 → 467.58 | 334.64 → 335.36 |
| Mixed 1,000-object 1L board | 458.95 → 458.90 | 359.35 → 359.74 |
| 50,000-pixel-wide PNG 1L board | 469.22 → 440.20 | 336.37 → 336.87 |

Every local run has **zero IP sockets, no LAN worker, 41 threads, zero idle
redraws** and only its normal X11/OS Unix socket. 1L records 1–2 CPU ticks;
2A records one. GPU-accounted peak bytes are exactly unchanged within each
board: 557,056 / 12,632,064 / 1,381,376 bytes respectively. These are renderer
allocations, not full driver VRAM attribution. The small startup sample shows
ordinary variation rather than a material unexplained regression; it does not
establish a statistical speedup. The authoritative comparison is 1L as specified
by this updated brief, not an incomparable older Phase 1F binary.

| Component | Purpose, bounded cost and simpler alternative |
| --- | --- |
| Semantic DTO/framing | Reuses existing Serde JSON; independent version, no framework. A custom binary codec would add a second codec to maintain. |
| SHA-256 | One direct external dependency, seven newly locked external crates total. CRC/path identity cannot supply strong content addressing. |
| Shared editor/adapter | Optional pointers/queues; no shared allocation or service locally. Renderer/tool forks were unnecessary. |
| Shared workers | Four opt-in blocking workers; one persistent control socket, at most two active transfer sockets; no idle timer/heartbeat. Separate transfer workers preserve visible supply/ack priority. |
| Client original cache | Lazy 512 MiB / 4096 hashes, plus at most two bounded metadata snapshots and empty owned locks; no eager full-board mirror. |
| Server authority | Headless standard-library TCP, bounded snapshots/inverses/receipts; no Tokio, WebSocket stack, database, discovery or daemon. |
| Persistence | Existing validated metadata codec and checksummed atomic envelopes; simpler than a new database. Full rewrites have measured cost. |

Stripped client size grows from 18,271,176 to **19,182,856 bytes**: **+911,680
bytes / 4.99%**. The stripped server is **1,723,568 bytes**. Unstripped release
files retain debug information (123,385,704 / 15,991,672 bytes) and are reported
separately. The server dependency tree contains no Winit, WGPU, image decoder or
GUI dependency. Aggregate executable/RSS costs are measured; per-dependency
linker attribution is not instrumented.

## Bounds, persistence and security boundary

The independently versioned `TLAN` protocol checks magic/major/declared lengths
before allocating a body: 64 MiB frame ceiling, 1 MiB ordinary operation, 4096
flat batch edits, 64 KiB asset chunks and an 8 GiB original ceiling. DTOs reject
unknown fields; existing constructors validate IDs, geometry and references.
Remote paths cannot become canonical sources or choose server filenames.

The client has four-slot control/visible/preparation queues, eight events and at
most one unconsumed snapshot; the editor separately holds at most 32 requests /
8 MiB, with 512 KiB retained-command admission. Overflow disconnects visibly
and requires reconciliation. Cache hashes are verified before exposure; corrupt
and obsolete bytes cannot establish authority. Cancellation occurs between
hash reads/chunks and active I/O has five-second timeouts.

Server limits are 16 loaded boards, 16 peers per board, 32 TCP connections,
64 output messages / 8 MiB queued bytes with one larger snapshot permitted in
an otherwise empty queue. Slow peers disconnect. Up to 16 recently edited
client histories retain 32 undo/redo commands and 128 receipts, sharing 8 MiB
per client. Authority envelopes are capped at 128 MiB; current state must remain
snapshot-rejoinable before publication. Default CAS quota is 4 GiB including
reserved uploads. Original bytes are separate from metadata rewrites.

Publication syncs temporary bytes, renames atomically and syncs the parent on
Unix. A post-rename sync failure advances accounting/authority and poisons the
board until validated reopen; it cannot overwrite newer disk authority from
older memory. Windows compiles/tests this path, but Unix directory-sync and
Windows native durability are not equivalent measured guarantees.

These are individual bounds, **not a 64 MiB or 128 MiB whole-process RSS ceiling**.
JSON/hex/parser scratch, cloned metadata, GPU/runtime memory and simultaneous
bounded peers add costs. Native imported-original staging retains the inherited
64 MiB/file, 2 GiB/request and 4 GiB/session disk ceilings until window close,
separate from the 512 MiB disposable original cache. CAS orphan collection is
deferred, so the explicit server quota matters.

No internet-exposure safety, authentication, physical network resilience or
malicious-local-owner filesystem isolation is claimed. Inherited 1L huge PNG
overview/scratch limitations, experimental opt-in tiles, guarded/progressive
JPEG limitations and low-end-memory uncertainty remain unchanged. Originals
over the shared 512 MiB admission limit remain explicit placeholders; remote
ranged codec/ROI supply is deferred.

## CI, binaries and retained evidence

The implementation was pushed to `main`. Exact-commit CI:
[Quality run 37819211140](https://github.com/gurppt/tack/actions/runs/37819211140).
Linux, Windows and dependency jobs all concluded **success**, recorded with
exact commit, timestamps and job URLs in `measurements/phase2a.json`. Configured jobs cover Linux/Windows workspace quality,
headless server process tests, Linux software-Vulkan readback/LOD and dependency
license/advisory checks. Windows desktop behavior remains untested.

Both human-test executables are current, executable and ignored by Git.
`tools/build-test-bin.sh` builds them together, marks a previous stamp stale
before building, stages successful outputs, then replaces `./bin/tack`,
`./bin/tack-server`, artwork and `./bin/BUILD.txt`.

The measured build stamp identifies implementation `b55bf390...`, release profile,
UTC `2026-10-08T17:47:14Z`, and these exact checksums:

- `./bin/tack`: `0cd2e13e6679b829316f64fff9c71900655a61826a0a0cd52feb5d4efc605a9b`.
- `./bin/tack-server`: `3e43ef7addba4eae197761f87e6287c2b570bb6ab2121a5950c93c8cd0417108`.

These match final native, shared performance and local-regression receipts.
Documentation-only closeout rebuilds update the stamp to the final Git commit;
the executable checksums must remain identical. The working-tree dirty flag
also reflects unrelated owner/director files intentionally preserved.

Generated Phase 2A evidence occupies approximately 29 MiB, including one
18 MiB stripped 1L baseline. Existing 1L corpora are reused. Temporary stripped
weight-measurement copies were removed after retaining checksums and sizes.
Approximately 28 GiB remains free, above the 10 GiB reserve. No owner board,
brief, profile, original or retained sole evidence was removed. Owned X11/test
processes are stopped during closeout.

Phase 2A stops here. Artist-feel and real multi-computer LAN review remain
human checks; TLS/auth, cloud, timed media, `.pur` and later performance work
require a separate brief.
