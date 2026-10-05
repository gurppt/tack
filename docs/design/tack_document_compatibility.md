# `.tack` compatibility contract for Phase 1B

Phase 1A contract, 2026-10-03, retained as the design requirements. Phase 1B now
implements a bounded indexed snapshot reader/writer; see the concrete
[format v1](tack_file_format_v1.md) and [storage decision](tack_storage_decision.md).
Future-tense requirements below describe the contract, not missing 1B features.

## Identity and authoritative state

Document schema version is distinct from app semantic version, dependency
versions, derived-generator version and any future replication protocol version.
File headers must advertise schema and required capabilities before expensive
allocation. Phase 1B must choose and test the actual version representation.

`DocumentId`, `ObjectId`, `AssetId` and `SourceId` are distinct, opaque, nonzero
128-bit identities. The domain accepts caller-supplied IDs and checks duplicates
within each document table. Callers must generate sufficiently unique IDs without
blocking the event path, retain them through save/reopen, and never recycle a
deleted identity for a different entity. Undo restores the original identity.
They are neither vector slots, paths, content hashes, cache IDs nor user/session
identities. No randomness or ID generator dependency is added in Phase 1A.
Phase 1B must specify a lossless canonical encoding (e.g. 32 hex digits rather
than a JSON floating-point number) and collision handling/import remapping.

Relationships are `DocumentObject → ImageAsset → Source`. Multiple objects may
share an asset, and assets may reference one source. Embedded source payloads
are addressed by SourceId through future storage; linked sources retain an
opaque location descriptor. Objects own world geometry, normalized source crop,
opacity and sampling override. Moving or relinking a source must preserve
object/asset IDs and layout. Source replacement/revision and cache invalidation
require explicit tested commands in the later slice; paths alone are not content
identity. Original embedded content is authoritative. A linked descriptor points
outside the file and is not a portability promise for those external bytes.

Phase 1A's PathBuf is an in-memory host-platform descriptor, NOT a portable wire
format. Persistence must define relative/absolute bases, Unicode and native
non-Unicode path encoding, platform provenance and relocation/missing-source
behavior. A foreign path must not silently resolve to an unrelated local file.
Linked paths are intentionally not canonicalized or opened in core constructors.

Objects use an explicit kind enum currently containing only Image. Future kinds
extend that boundary, not an unrelated entity/plugin framework. Unknown file
kinds must be handled by the storage compatibility layer before conversion into
this known, validated domain subset. The current enum is not a deserializer and
must not be used as one that rejects/drops all newer objects silently.

## Geometry and defaults

Durable geometry is f64 world units, independent of pixels and camera. Transform
stores center, positive world width/height, clockwise radians in y-down space,
and two flip flags. Its rotated AABB must satisfy current world-coordinate
limits; it is derived and must be recomputed/validated, not trusted from disk.
Rotation is finite, not normalized implicitly. Normalized crop is a positive
`x,y,width,height` UV rectangle entirely inside `[0,1]`; crop changes which source
region fills the existing box and does not silently change world geometry.
Opacity is finite in `[0,1]`. Invalid persisted geometry must produce a bounded,
structured recovery/error outcome, not NaN propagation or hidden normalization.

Explicit defaults for an absent supported optional field: crop=full image,
opacity=1, filtering=Default, flips=false, rotation=0. Position/size and IDs are
required, not invented on load. Default sampling delegates to the user's local
preference; Smooth/Nearest overrides travel with the object. An unknown future
filtering value must be preserved and shown with a documented visual fallback,
not rewritten as Default on save. Z-order is an explicit back-to-front sequence
of stable ObjectIds; duplicates/missing IDs are invalid. Command insertion
indices are local operation positions, never persisted entity identities.

Camera, selection/tool holds, bindings/user preferences, GPU handles, active
decoders and undo/redo history are not durable object state. History is local
and explicitly discarded at editor consumption; Phase 1A does not serialize it.

## Reading old and newer documents

New Tack must read old supported documents through explicit, tested migrations.
Keep fixtures for every published schema and define semantic changes, absence
defaults and conversion costs. Do not silently reinterpret existing fields.
Migrations work on a separate recovered/in-memory copy; never modify the user's
only viable file while discovering whether a migration is possible.

Older Tack should open newer documents gracefully where reasonably possible:

1. Native read when schema/capabilities permit it.
2. Degraded non-destructive read with unsupported records/properties preserved
   as bounded opaque payloads and honest fallback/placeholder rendering.
3. Explicit incompatible-format refusal or read-only mode if faithful retention
   cannot be guaranteed. Unknown values never cause a panic.

Preservation belongs to a future storage envelope alongside the known domain,
not speculative opaque blobs inside every Phase 1A object. Preserve ownership,
IDs, record ordering and unknown-field payloads where possible. Limits still
apply: unknown data does not get unlimited allocation/decompression privilege.
If editing known state could invalidate an unsupported record or relationship,
disable that edit or require an explicit destructive conversion into a new copy.

An older writer must **not silently destroy unsupported newer information**.
Saving back to the original file is permitted only when all retained unknown
data and relationships can round-trip safely. Otherwise require read-only use
or a separate explicitly acknowledged conversion. A warning after overwriting
the original is insufficient. Test unknown enum values, object kinds, fields,
asset relationships and derived encodings independently, including re-save.

## Export for an older version

Future “Save/Export for older Tack version…” writes a separate target copy,
leaving the source file intact. Report exact unsupported features and proposed
degradations before conversion. Prefer faithful appearance over disappearance:
timed media may become a selected static frame; unsupported effects/constructs
may be baked/rasterized; unsupported properties require an explicit warning
before discarding. Loss of editability and changed source relationships must be
reported. Conversions must be deterministic, bounded and tested, with IDs retained
where meanings survive and explicit remapping where converted meanings differ.
No conversion or media implementation is authorized by this note.

## Derived data, budgets and recovery

**Source size is not working-set size.** Metadata opening must not require eager
reading/decoding/hashing all originals. Obtain geometry/source descriptors first;
resolve visible representations through explicit asynchronous bounded workers.
Renderer queries only inspect resident immutable logical metadata.

Overview/LOD data is disposable. Schema records should carry asset ID, role,
encoding, dimensions, generator/version, source revision/fingerprint and payload
location. Do not hardcode “128 PNG” into durable document semantics. Missing or
corrupt previews degrade to placeholders and regenerable representations without
damaging authoritative records/content. Source hashing, if needed, is bounded
background work; linked-source identity/revision needs cross-platform tests.

Validate container directory/record sizes and counts, ID uniqueness/reference
integrity, offsets/length arithmetic, geometry, metadata length, unknown payload
length and decompression limits BEFORE allocating or trusting data. Domain
default metadata limits (100k objects/assets/sources and linked paths ≤4096 native
encoded bytes) are configurable policies, not proof of full rendering scale.
They do not bound reader allocation by themselves: Phase 1B must enforce file
byte/count/decompression budgets before constructing the domain. Source bytes
are streamed separately with working-set budgets; enormous source metadata must
not request enormous RAM allocations. Never treat an untrusted stored filename
as permission to extract arbitrary paths or execute/open a source automatically.

Storage comparison/tests must cover interrupted writes at several stages,
truncation/corruption, missing linked assets, missing/corrupt derived data,
restart, unknown newer data and migrations, Unicode/native Windows/Linux paths,
large metadata sets and geometry-before-heavy-assets opening. Recovery must not
overwrite the only viable copy. Atomic replacement/transaction and fsync/durability
semantics must be measured; process SIGKILL + rename consistency is not a claim
of power-loss durability. Choose the container only after this evidence.
# Phase 1D extension

Schema 1 remains supported and is still written for ungrouped image-only boards.
Frames and groups require schema 2; see [spatial wire format](tack_file_format_v2.md).
Readers that only understand schema 1 refuse schema 2; Tack refuses schemas >2,
unknown durable record kinds and invalid references without changing the input.
The schema is selected from the actual durable content, not the application version.

# Phase 1E extension

Schema 3 adds plain text, rectangle, line, arrow and scribble; see
[annotation wire format](tack_file_format_v3.md). Current readers support schemas
1/2/3 and refuse >3 or unknown authoritative records without editing the input.
The previous paragraph's >2 limit describes the shipped Phase 1D reader only.
Schema-2 frame/group records remain byte compatible, including inside schema 3.
Annotations have no media/source relationship. Transient editing/capture/font
resources remain outside persistence. Mixed annotation groups are deferred and
refused explicitly in the authoring UI; existing image groups are retained.
