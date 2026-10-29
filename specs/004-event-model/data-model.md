# Data Model: Event Model

Types live in `gedo-core::event` unless noted. Persisted values are encoded with postcard inside a `RecordEnvelope` (format version first). Enums used in stored values are encoded by variant position: never reorder or remove a variant, append new ones at the end.

## Changes to existing types

- `CommitInfo.author_email: String` (port `ObjectStore`): the commit author's e-mail as written in the commit, read by the gitoxide and in-memory adapters (research R4). Commits written by Gedo (`NewCommit`) are unchanged.
- `Collection::entries_with_prefix(prefix: &str) -> EntryMap`: the entries whose id starts with `prefix` (at least 2 hex digits), reading only the fan-out directory of its first two digits (research R9).

## EventId and TapeId

- 16 bytes, displayed as a UUID (`8-4-4-4-12` lowercase hex), version 8, derived from content (research R2, R8); ordered by bytes.
- Entry name in a collection: the 32 lowercase hex digits without dashes.

## Clock

- `generation: u64` (1 for a root commit or an event without a base), `sequence: u32`.
- Order: by `generation`, then `sequence`. Total order of events: `(clock, id)` (research R3).

## AuthorFingerprint, AgentId, AuthorKind, Author

- `AuthorFingerprint([u8; 32])`: research R4; displayed as 64 hex digits.
- `AgentId(String)`: 1 to 64 bytes of `[a-z0-9._-]`, first character `[a-z0-9]` (research R5).
- `AuthorKind`: `Person` | `Agent(AgentId)`.
- `Author { fingerprint: AuthorFingerprint, kind: AuthorKind }`.

## Labels

- `LabelKey(String)`: same rule as `AgentId`. `LabelValue(String)`: 1 to 256 bytes of UTF-8 without control characters.
- `Labels(BTreeMap<LabelKey, LabelValue>)`: at most 16 entries. `Labels::for_recording` additionally refuses keys starting with `gedo.` (research R6).
- Text form (`GEDO_LABELS`): `key=value[,key=value]*`, escapes `\,` and `\\` in values.

## SemanticOp

- `Delta(SemanticDelta)`: any delta of feature 003.
- `IdentityMerged { kept: AstNodeId, merged: AstNodeId }`.
- `target()`: the delta's id, or `merged`.

## Origin

- `Live` | `Reconstructed`.

## Event

| Field | Type | Notes |
|---|---|---|
| `clock` | `Clock` | research R3 |
| `target` | `AstNodeId` | equals `operation.target()` (checked on decode) |
| `operation` | `SemanticOp` | |
| `result_hash` | `Option<StructuralHash>` | `None` exactly for `Tombstoned` and `IdentityMerged` (checked) |
| `author` | `Author` | |
| `labels` | `Labels` | empty for reconstructed events |
| `origin` | `Origin` | |

- `gedo_crdt::event_id` derives the `EventId` from the encoding of these fields (research R2).
- Stored record: `RecordEnvelope { format: 1, payload: postcard(Event) }` in `refs/gedo/events`, entry = id. Decoding checks the size limit, the format, the invariants above and that the derived id equals the entry name; otherwise `Corrupted` with the entry.

## Tape

- `events: Vec<EventId>`: 1 to 65,536 ids, in recording order, no duplicates.
- `gedo_crdt::tape_id` derives the `TapeId`.
- Stored record: format 1 in `refs/gedo/tapes`, entry = tape id, checked like events.

## Commit index entry

- Entry name: commit id hex followed by tape id hex, in `refs/gedo/commit-tapes`; blob: `RecordEnvelope { format: 1, payload: [] }`.
- A commit may have several entries; the index never stores a tape for a commit whose events were not written first (research R9).

## WalRecord (`gedo-app::events`)

- `Event(Event)`: the only variant now; later features add theirs at the end. Encoded in a `RecordEnvelope` with format 1 as the payload of one WAL record.

## RecordingContext (`gedo-crdt::labels`)

- `email: String` (Git identity of the person, hashed into the fingerprint), `agent: Option<AgentId>`, `labels: Labels` (validated with `for_recording`).
- Built from values or from the texts of `GEDO_AGENT` and `GEDO_LABELS`.

## EventFilter (`gedo-crdt::filter`)

- `labels: Vec<(LabelKey, LabelValue)>` (all required), `author_kind: Option<Person | Agent>`, `agent: Option<AgentId>`, `fingerprint: Option<AuthorFingerprint>`.

## Local derived data

- Commit generations are kept in memory by `EventLog` (research R3); reconstructed events are recomputed on demand from the `identity` namespace of the symbol cache (research R10). Nothing new is stored in the cache.

## Relations

- An event targets one `AstNodeId`; a tape lists events; an index entry links one commit to one tape; a commit without index entries has reconstructed events.
