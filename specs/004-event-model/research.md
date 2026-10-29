# Research: Event Model

Decisions taken while planning `specs/004-event-model`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0013, `docs/product/TECH_SPEC.md`) and the clarifications of 2026-10-29 (reconstructed events are local derived data, tapes keep every recorded event, the author fingerprint hashes the trimmed, lowercased Git e-mail).

## R1. Where each step lives

- **Decision**:
  - `gedo-core` gains the event model in `event.rs`: `EventId`, `Clock`, `AuthorFingerprint`, `AgentId`, `AuthorKind`, `Author`, `LabelKey`, `LabelValue`, `Labels`, `SemanticOp`, `Origin`, `Event`, `TapeId`, `Tape`, with their validation (lengths, character sets, the reserved prefix) and their order. Types only and their invariants, like `identity.rs`.
  - A new core crate **`gedo-crdt`**, the crate the [event log page](../../docs/architecture/events.md) already names, gains the pure logic: `id` (derivation of event and tape ids), `clock` (generations and the clock of live and reconstructed events), `author` (fingerprint), `labels` (the `GEDO_LABELS` text form and `RecordingContext`), `reconstruct` (events of a commit from its deltas) and `filter` (`EventFilter`). The tree CRDT, lanes, streams and the sequencer join it in later features.
  - `gedo-app` gains the use case `events` (`EventLog`): record, publish, read and filter, over the `WriteAheadLog`, `ObjectStore`, `RefStore` and `SymbolCache` ports and the existing `IdentityIndex`.
- **Rationale**: the same split as feature 003 (model in `gedo-core`, algorithms in a pure crate, orchestration in `gedo-app`); the derivation and the clock rules are tested with property-based tests without Git.
- **Alternatives considered**: everything in `gedo-core` (it would need `blake3` and grow logic that belongs to the event layer); a crate named `gedo-events` (the architecture already reserves `gedo-crdt` for the event layer, and the CRDT of FEAT-066 lands there next).

## R2. Event ids are derived from content, live and reconstructed alike

- **Decision**: an `EventId` is 16 bytes displayed as a UUID version 8: `BLAKE3` in key-derivation mode with the context `"gedo event id v1"` over the postcard encoding of the event's content (every field but the id), truncated to 16 bytes, with the version and variant bits of RFC 9562, exactly as `AstNodeId` is built (ADR-0007). The stored record holds the content; the id is recomputed when it is read and compared with the entry's name (FR-014).
- **Rationale**: ADR-0007 derives reconstructed event ids from content; deriving live ids the same way gives the property ADR-0007 asks of live ids ("keep their id when they travel") for free, makes a retried publish and a graft idempotent, and lets the store key events by content (ADR-0008). 16 bytes match `AstNodeId` and keep tapes small.
- **Alternatives considered**: random UUID v7 for live events (the `uuid` row of TECH_SPEC): a retried publish would duplicate events and a crash between steps could leave two ids for one event; a 32-byte id: no practical gain at the scales of a repository, twice the tape size.

## R3. Clocks follow the commit history: generation and sequence

- **Finding**: a classic Lamport counter is not a function of the history alone. A reconstructed event's id includes its clock, so its clock must be derivable from the commit by any machine, with or without the team's live events (US4); and a live event must still be later than everything in its base commit's ancestry (US5).
- **Decision**: `Clock { generation: u64, sequence: u32 }`, compared as a pair. The **generation** of a commit is 1 for a root and 1 + the highest generation of its parents otherwise: it is the Lamport time of the commit graph. A reconstructed event of commit `C` has generation `gen(C)` and the sequence of its position in the commit's events. A live event recorded on top of base commit `B` has generation `gen(B) + 1` (1 without a base) and the next sequence of the write-ahead log, so the events of one publish keep their recording order. The total order of events is `(generation, sequence, id)` (FR-004).
- **Rationale**: an event of a commit is later than every event of its ancestors (their generations are lower), live events on top of `B` are later than `B`'s events, and the order is a pure function of the events. Events of two branches with equal clocks are concurrent; the id breaks the tie the same way everywhere.
- **Where generations come from**: `EventLog` computes them through the `ObjectStore` and keeps them in memory, walking only the ancestors not yet known. They are local derived data, like the identity results. (Planned as a symbol cache memo; kept in memory during implementation because `IdentityIndex` already borrows the cache, and the benchmark showed no need: reconstructing one commit of this repository takes 0.3 ms at the 95th percentile.)
- **Alternatives considered**: a classic Lamport counter (not reproducible by reconstruction); wall-clock time (not causal, differs between machines); the first-parent depth already in the identity records (two parents of a merge could have the same depth as the merge's other ancestry; the generation takes every parent).

## R4. The author fingerprint

- **Decision**: `AuthorFingerprint` is the 32-byte `BLAKE3` key derivation with the context `"gedo author v1"` over the Git e-mail, trimmed of ASCII whitespace and lowercased (clarification 3). A reconstructed event uses the commit's author e-mail; a live event uses the e-mail of the recording context, which the caller (the CLI or the daemon, v0.4.0) takes from the repository's Git identity. `CommitInfo` gains an `author_email` field so the core can read it through the `ObjectStore` port.
- **Rationale**: TECH_SPEC stores the hash, not the identity; hashing only the e-mail keeps one person one fingerprint across display-name changes. The e-mail is already public in the commit, so the hash hides it only from casual reading, which is the intent of TECH_SPEC (no new personal data).
- **Alternatives considered**: name and e-mail (splits a person on a display-name change, rejected in clarification 3); a salted hash per repository (tools could no longer match a person across repositories, which Mission Control needs).

## R5. Author kind and agent identifiers

- **Decision**: `Author { fingerprint, kind }` with `AuthorKind::Person` or `AuthorKind::Agent(AgentId)`. An agent's events carry the fingerprint of the Git identity it runs on (FR-005). `AgentId` and `LabelKey` share one rule: 1 to 64 bytes of lowercase ASCII letters, digits, `.`, `-` and `_`, starting with a letter or digit.
- **Rationale**: the dev-cycle contract (ADR-0013) names agents and filters by them; a restricted character set keeps identifiers safe in refs, file names, JSON and command lines.
- **Alternatives considered**: free UTF-8 identifiers (harder to quote and compare, no use case needs them).

## R6. Labels

- **Decision**: `Labels` is a sorted map of at most 16 `LabelKey` to `LabelValue` (ADR-0013). Values are 1 to 256 bytes of UTF-8 without control characters. `Labels::for_recording` refuses keys starting with `gedo.`; decoding accepts them (Gedo may set its own). The text form of `GEDO_LABELS` is `key=value` pairs separated by `,`, with `\,` and `\\` as escapes in values; an empty text is no labels; duplicates, empty keys or values and unknown escapes are refused with the position and the reason. `GEDO_AGENT` is an `AgentId`. `RecordingContext { email, agent, labels }` is built from structured values or from these texts; reading the environment is the CLI's job (v0.4.0).
- **Rationale**: a sorted map encodes deterministically with postcard, so labels take part in the id without ambiguity; the limits bound the record size.
- **Alternatives considered**: JSON in `GEDO_LABELS` (heavier for shells, no gain for flat pairs); labels as a list (order would change ids for the same set).

## R7. Operations reuse the deltas of feature 003

- **Decision**: `SemanticOp::Delta(SemanticDelta)` or `SemanticOp::IdentityMerged { kept, merged }`. The event's target is the delta's id, or the merged id. `result_hash` is the entity's structural hash after the operation: from the commit's `BlobAstMap` for a reconstructed event, from the caller for a live one; `None` for a tombstone and an identity merge. New operations are appended as new variants (postcard encodes variants by position).
- **Rationale**: one vocabulary from identity to events; `result_hash` is what re-attachment (FEAT-065) matches on with the target and the operation (ADR-0008), so it is in the format from the start. The names in `core.md` (Insert, Delete, Move, Rename, PatchBody) were placeholders and are replaced by these.
- **Alternatives considered**: a separate operation enum mapped from the deltas (two vocabularies to keep in step); no `result_hash` (FEAT-065 would need a format change).

## R8. Tapes and the commit index

- **Decision**: a `Tape` is the ordered list of the event ids of one publish (at most 65,536), with no commit id in it (FR-010). Its `TapeId` is derived like an event id with the context `"gedo tape id v1"`. The commit index has one entry per (commit, tape) pair; a commit may have several tapes.
- **Rationale**: tapes and events survive rewrites unchanged; FEAT-065 only adds index entries for the rewritten commits.
- **Alternatives considered**: a tape that names its commit (a rewrite would need a new tape with the same events); one index record per commit listing its tapes (two machines adding tapes to one commit would write different content under one key, which the union of collections refuses).

## R9. Storage layout: three collections

- **Decision**: three collections (ADR-0011), so each holds one kind of entry:
  - `refs/gedo/events`: entry = event id (32 hex), blob = the event record;
  - `refs/gedo/tapes`: entry = tape id (32 hex), blob = the tape record;
  - `refs/gedo/commit-tapes`: entry = commit id followed by tape id (72 hex for SHA-1, 96 for SHA-256), blob = an empty versioned record.
  Every entry is content-addressed, so divergent chains join by union without conflict. The commit's tapes are found by reading only the fan-out directory of the commit's first two hex digits and keeping the entries that start with the commit id (`Collection::entries_with_prefix`). `put` order for a publish is events, tapes, then the index entry, so a reader never finds an index entry whose tape or events are missing on the machine that wrote it.
- **Rationale**: no reference per object (ADR-0011's measurements), plain fast-forward push and fetch, and the existing remote setup (`refs/gedo/*`) carries the new collections without any change.
- **Consequences**: TECH_SPEC's data table said events, tapes and the index live in `refs/gedo/tapes/`; ADR-0014 records the three collections and the other format decisions of this research, amends ADR-0003 and ADR-0008 on the layout, and the table is edited. The events collection grows by one entry per recorded event; the scale study (ADR-0011) measured chains to 100,000 entries, and the publish benchmark (R13) reruns that size with event records. Compaction of very large collections is left to later releases, as ADR-0011 already says.
- **Alternatives considered**: one collection with three kinds of entries (the kinds would need an id prefix and every reader would list all of them); events stored inside the tape blob (an event could no longer be fetched or deduplicated by id, against ADR-0008).

## R10. Reconstructed events are local derived data

- **Decision** (clarification 1): for a commit without index entries, `EventLog::events_of_commit` reconstructs the events from the identity record of the commit: the deltas against the first parent, then the identity merges, in that order, each with the commit author as a person, no labels, `Origin::Reconstructed`, the clock of R3 and the result hash from the commit's maps. Nothing is written to `refs/gedo/*`. The ids are taken **as recorded at the commit**, not resolved through the alias table: the table grows as descendants are indexed, and resolving would make a commit's events depend on what else was indexed. Reconstruction stores nothing; it reads the commit's record and maps from the `identity` namespace.
- **Rationale**: the events are a pure function of the history, so sharing them would add volume without information (the argument of ADR-0012); only live events carry what history cannot give (author kind, labels, the evolution of a delivery).
- **Alternatives considered**: reconstructing at fetch and pushing the result (rejected in clarification 1).

## R11. Recording and publishing

- **Decision**:
  - `EventLog::record(context, base, ops)` builds the events (R2, R3), wraps each in a `WalRecord::Event` envelope and appends it to the `WriteAheadLog`; it returns the ids. The next sequence is the highest sequence in the log plus one for the same generation, and 0 otherwise; it is read once when the log is opened.
  - `EventLog::publish(commit)` reads every record of the log, refuses more than 65,536 events (the log is kept), and if there is at least one event: puts the events, the tape and the index entry `(commit, tape)` into their collections (R9), then truncates the log through the last record read. Events appended after the read stay for the next publish.
  - Crash safety: every step is idempotent because every entry is content-addressed; a crash before the truncation leaves the same events in the log, and the next publish writes nothing new and then truncates (SC-003).
- **Rationale**: the WAL already gives durability, ordering and torn-tail recovery (feature 001); the collections give atomic, conflict-free sharing.
- **Alternatives considered**: truncating first and publishing from memory (a crash would lose events); consolidating events at publish (rejected in clarification 2; lanes are FEAT-067).

## R12. Filtering

- **Decision**: `EventFilter { labels, author_kind, agent }` where every given condition must hold (labels: every `key=value` given must be present). `EventLog::filter` scans the stored events of the `events` collection (and, when asked for a commit range, the events of those commits) in the total order of R3. No secondary index in this feature.
- **Rationale**: the stores are small until the CLI exists; SC-006 measures the scan at 10,000 events. A label index in the symbol cache is a backlog item if the benchmark or v0.4.0's `--label` needs it.
- **Alternatives considered**: a label index now (more code and invalidation rules before a user needs it).

## R13. Tests, fuzzing and benchmarks

- Property-based tests in `gedo-crdt`: id stability and sensitivity, label parsing round trips and refusals, clock monotonicity along generated histories, total order independent of input order.
- `gedo-app` tests on the in-memory adapters: record and publish with simulated crashes at every step (`MemoryWal` and `MemoryRefStore` hooks), joined publishes from two machines, reconstruction of generated histories in 3 orders, snapshot of the events reconstructed from selected commits of the Gedo repository on both platforms (SC-001).
- Fuzz target `decode_event_record` for events, tapes, index entries and WAL records (SC-007).
- Benchmarks `record_event` (SC-004), `publish_tape` (1,000 events; and a chain of 100,000 event entries), `reconstruct_commit` (SC-005), `filter_events` (SC-006), reported by the non-blocking bench job.

## R14. Observability

- `tracing` events in `gedo-app::events` (profile `library`): events recorded, publish (events, tape id, commit, time), publish refused for size, reconstruction (commit, events, from memo or computed). A corrupted record is an error with its id, never skipped (FR-015). Tests assert the publish and refusal events.
