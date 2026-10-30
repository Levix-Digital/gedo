---
title: Event Log and Convergence
summary: Records semantic events and converges concurrent changes deterministically.
code: [crates/core/gedo-crdt, crates/core/gedo-app/src/events]
---

# Event Log and Convergence

Records semantic events and converges concurrent changes deterministically.

## Responsibilities

- Turns semantic deltas and identity merges into **events** with ids derived from content, clocks consistent with the commit history, an author fingerprint, an author kind and opaque labels.
- Records live events in the write-ahead log and publishes them as one **event tape** per publish, indexed by commit.
- Reconstructs, on demand and identically on every machine, the events of commits made without Gedo.
- Filters events by labels, author kind, agent and fingerprint, for the tools built on Gedo (ADR-0013).
- Later releases add the **tree CRDT** that converges concurrent events (set semantics for members of a structure, commutative move and edit, an event present on both sides applied once), **semantic lanes** (refactor and feature, `gedo publish --split-lanes`), **virtual streams** (`gedo stream <name>`, whose events inherit the stream's author and labels), re-attachment of tapes after rebases and squashes (ADR-0008), and the deterministic **sequencer** shared by the merge driver, the shadow merge and the relay.
- Does not read or write files; the WAL and the collections are reached through ports.

## Structure

| Where | What |
|---|---|
| `gedo-core::event` | `Event`, `EventId`, `Clock`, `Author`, `AuthorKind`, `AgentId`, `Labels`, `SemanticOp`, `Origin`, `Tape`, `TapeId` and their invariants |
| `gedo-crdt` | Pure rules: `id` (event and tape ids), `clock` (generations, live clocks, the total order), `author` (fingerprint), `labels` (the `GEDO_LABELS` text form and `RecordingContext`), `reconstruct` (the events of a commit), `filter` (`EventFilter`) |
| `gedo-app::events` | `EventLog`: record, publish, read, reconstruct and filter over the object, reference and WAL ports; `records` holds the stored forms |

## Behavior

**Events.** An event names its target entity, its operation (a `SemanticDelta` of feature 003, or an identity merge with the kept and merged ids), the entity's structural hash after it (none for a tombstone or a merge), a clock, the author and the labels, and whether it was recorded live or reconstructed. Its `EventId` is a UUID version 8: BLAKE3 in key-derivation mode (`gedo event id v1`) over the postcard encoding of all those fields, truncated to 16 bytes. Equal content gives equal ids everywhere, so a retried publish or a graft stores an event once ([ADR-0014](../adr/0014-event-format-content-derived-ids-history-clocks-and-three-co.md)).

**Clocks.** A clock is `(generation, sequence)`. A commit's generation is 1 for a root and one more than its highest parent otherwise. A reconstructed event takes its commit's generation and its position among the commit's events; a live event recorded on top of a commit takes that commit's generation plus one and the next sequence of the log. An event of a commit is therefore later than every event of its ancestors, live events are later than their base's, and `(clock, id)` orders any set of events the same way on every machine. `EventLog` keeps the generations it computed in memory and walks only unknown ancestors.

**Authors and labels.** The author fingerprint is BLAKE3 (`gedo author v1`) of the Git e-mail with ASCII whitespace trimmed and ASCII letters lowercased; the e-mail is never stored. The author kind is a person, or an agent with an identifier (1 to 64 bytes of `[a-z0-9._-]`, starting with a letter or digit) whose events carry the fingerprint of the Git identity it runs on. Labels are at most 16 pairs, keys with the agent identifier rule and values of 1 to 256 bytes of UTF-8 without control characters, sorted by key, stored and returned unchanged. A `RecordingContext` is built from values or from the texts of `GEDO_AGENT` and `GEDO_LABELS` (`key=value` pairs separated by `,`, with `\,` and `\\` escapes); a tool cannot record a key with the reserved prefix `gedo.`. Reconstructed events are a person's and carry no labels.

**Record and publish.** `record` appends one `WalRecord::Event` per event to the WAL. `publish(commit)` reads the log, deduplicates by id, refuses more than 65,536 events (the log is kept), and writes three content-addressed collections in order: `refs/gedo/events` (entry: the event id), `refs/gedo/tapes` (entry: the tape id; the tape lists the event ids in recording order and names no commit) and `refs/gedo/commit-tapes` (entry: the commit id followed by the tape id). Only then does it truncate the log through the last record it read, so events appended meanwhile stay. Every step is idempotent: after a crash, publishing again gives the same data. Chains written on several machines join by union, like every collection.

**Reading.** `tapes_of(commit)` reads the index entries of the commit from one fan-out directory; `tape` and `event` read records, check their format, invariants and that their content matches their id; a tape or event named by an index or a tape but missing locally (a partial fetch) is `Missing`. `events_of_commit` returns the published tapes of a commit, or, when it has none, the events reconstructed from its identity record: its deltas against the first parent, then its identity merges, with the ids as recorded at the commit (never resolved through later aliases), the commit author and the commit's generation. Reconstructed events are never written to `refs/gedo/*`. `filter` scans the stored events and returns the matching ones in the total order.

**Event cache.** With a symbol cache (`EventLog::with_cache`), a publish also keeps the records of its events in the cache's `events` namespace, after the index entry and before the log is truncated, and `event`, `events`, `events_of_commit` and `filter` read the cache first and the collection on a miss, keeping what they read, so a filter reads one database instead of one object per event ([ADR-0015](../adr/0015-packed-batch-writes-and-an-event-cache.md)). Cached records go through every check of stored ones; a refused entry is read again, and a cache failure is only a warning.

**Observability.** `tracing` events: events recorded (debug), events published (info, with the commit, the tape and the count), publish refused because the tape is too large (warn), events reconstructed (debug).

## Interfaces

- Called by `gedo-app` use cases (ingest, publish, graft, status) and, through the CLI's `--json` output (ADR-0013), by tools built on Gedo.
- Persists through the write-ahead log, object store and reference store ports; reads identity results through `IdentityIndex`.

## Decisions

- [ADR-0001](../adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md): CRDT and sequencer are pure core logic.
- [ADR-0003](../adr/0003-git-object-store-as-the-canonical-datastore.md): Gedo data lives in the Git object store under `refs/gedo/` (the layout of events is amended by ADR-0014).
- [ADR-0008](../adr/0008-gedo-data-robust-to-history-rewrites.md): content-addressed events and re-attachment after rewrites.
- [ADR-0009](../adr/0009-semantic-merge-driver-and-shadow-merge.md): merge rules shared by the driver and the shadow merge.
- [ADR-0013](../adr/0013-versioned-json-output-as-the-contract-for-tools-built-on-ged.md): opaque labels and author kind inherited from streams, reserved `gedo.` prefix.
- [ADR-0014](../adr/0014-event-format-content-derived-ids-history-clocks-and-three-co.md): content-derived ids, history clocks, the three collections, reconstructed events kept local.
- [ADR-0015](../adr/0015-packed-batch-writes-and-an-event-cache.md): large publishes written as packs, stored events read through the `events` cache namespace.
