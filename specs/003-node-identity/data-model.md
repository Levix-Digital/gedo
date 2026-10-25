# Data Model: Node Identity

Types live in `gedo-core` unless noted. Persisted values are encoded with postcard inside a `RecordEnvelope` (format version first) and stored only in the local symbol cache (research R2).

## Changes to the syntax model (feature 002)

- `ParsedFile.unit: String` and `SyntaxMap.unit: String`: the repository path of the entity's compilation unit (research R4), filled by the parser adapter. Not hashed; `HashFormat` stays `V1`.

## AstNodeId

- 16 bytes, displayed as a UUID (`8-4-4-4-12` lowercase hex).
- New ids are UUID version 8, derived as in research R7. Ids of version 4 or 7 from earlier formats are valid and opaque.
- Ordered by bytes; "the lower id" (reconciliation) is this order.

## MatchFormat

- `V1`: the matching tiers, similarity weights and threshold, tie-breaking, derivation and delta rules of research R3 to R8 and R12. Stored with every `BlobAstMap`; a reader refuses a format it does not know. Ids produced by one format are never recomputed by a later one (FR-005).

## AnchoredNode

- `id: AstNodeId`
- `kind: EntityKind`, `name: String` (the own name, for `Renamed`), `qualified_path: String`, `occurrence: u32` (the unit is the map's)
- `byte_range: Option<ByteRange>`: `None` for a node carried while its file has a syntax error (research R8, `detached`)
- `container: Option<u32>`: index of the enclosing node in the same map
- `structural_hash`, `signature_hash`, `body_hash: StructuralHash`
- `has_error: bool`

Signature and body tokens are not stored: they are recomputed from the blob when a later version is matched against this one (the parser is deterministic).

## BlobAstMap

- `source_blob: GitOid`, `path: String`, `unit: String`, `language: Language`
- `hash_format: HashFormat`, `match_format: MatchFormat`
- `nodes: Vec<AnchoredNode>` in source order (detached nodes last, by qualified path)
- `has_error: bool`
- Invariants: ids are unique within a map; containers precede their children; placed nodes precede detached ones. `check()` verifies them, and a map decoded from the cache that fails them is refused as corrupted.
- Identified in the cache by `map/<digest>`, where the digest is BLAKE3 of the encoded map, so identical maps are stored once.

## SemanticDelta

Ordered by variant (as listed), then by `id` (research R12):

| Variant | Fields |
|---|---|
| `Created` | `id`, `kind`, `unit`, `path`, `qualified_path` |
| `Moved` | `id`, `from: Location`, `to: Location` (`Location = { unit, path, qualified_path }`) |
| `Renamed` | `id`, `from_name`, `to_name`, `qualified_path` (after) |
| `SignatureMutated` | `id`, `before: StructuralHash`, `after: StructuralHash` |
| `BodyModified` | `id`, `before: StructuralHash`, `after: StructuralHash` |
| `Tombstoned` | `id`, `kind`, `unit`, `path`, `qualified_path` (last known) |

## IdentityMerged

- `kept: AstNodeId`, `merged: AstNodeId` (`kept < merged`), `commit: GitOid` (the merge commit where it was found).
- Ids resolve to the lowest id of their connected component over all records (research R11).

## Match input and output (gedo-ast)

- `ChangeSet { old: Vec<BlobAstMap with tokens>, new: Vec<SyntaxMap> }`: the old side carries the old maps plus the signature and body tokens recomputed from their blobs; files only in `old` were deleted, files only in `new` were added.
- `MatchOutcome { maps: Vec<BlobAstMap>, deltas: Vec<SemanticDelta> }`: one map per new file, sorted by path.
- `match_change(&ChangeSet, live_ids: &BTreeSet<AstNodeId>) -> MatchOutcome`: `live_ids` are the ids of the commit's unchanged files, so a derived id never collides with one of them (research R7).

## Cache records (gedo-app, namespace `identity`)

| Key | Value (format 1) |
|---|---|
| `commit/<oid hex>` | `CommitRecord { parents, depth (first-parent distance to a root), files: Vec<(path, Option<map digest>)>, deltas, merges: Vec<IdentityMerged> }`: only the Rust files that changed against the first parent; `None` is a deletion |
| `checkpoint/<oid hex>` | `Checkpoint { files: Vec<(path, map digest)> }`: every Rust file of the commit, written every 64 first-parent commits |
| `map/<digest hex>` | `BlobAstMap` |
| `merges` | `MergeList { merges: Vec<IdentityMerged> }`: every identity merge, in the order recorded; the alias table is rebuilt from it when the index opens |

State of one commit: indexed (its `commit/` record exists) or not. A record is written in one cache transaction with its new maps, after the records of all its parents.
