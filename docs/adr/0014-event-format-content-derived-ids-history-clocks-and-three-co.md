---
id: 14
title: 'Event format: content-derived ids, history clocks and three collections'
status: accepted
date: '2026-10-29'
version: v0.3.0
subsystems: [events, store]
supersedes: []
superseded_by: []
amends: [3, 8]
amended_by: [15, 19]
---

# ADR-0014: Event format: content-derived ids, history clocks and three collections

## Context

Feature `004-event-model` (FEAT-063, FEAT-064) turns the semantic deltas and identity merges of feature 003 into events, the shared history of v0.3.0. Stored formats freeze at v1.0.0 (FEAT-127), so the fields of an event and the layout of the shared data must be fixed now. ADR-0007 derives the ids of reconstructed events from their content, including their Lamport clock; ADR-0008 stores events by id, groups them in tapes and keeps a separate index from commits to tapes; ADR-0003 and the technical specification put events, tapes and the index in `refs/gedo/tapes/`, and ADR-0011 stores every namespace as one commit-chain collection whose entries are content-addressed, so divergent chains join by union. Three facts shaped the format. A classic Lamport counter depends on which live events a machine has seen, so a reconstructed event's clock, and therefore its id, would differ between machines. An index entry keyed by commit alone would hold different content on two machines that publish tapes for the same commit, and the union of ADR-0011 refuses that. Events reconstructed for commits made without Gedo are a pure function of the history, like the identity results of ADR-0012. The user chose on 2026-10-29 that reconstructed events stay local, that tapes keep every recorded event and that the author fingerprint hashes the Git e-mail (`specs/004-event-model/spec.md`, clarifications).

## Decision

- **Ids from content, for every event.** An `EventId` is 16 bytes, a UUID version 8 built like `AstNodeId`: the BLAKE3 key derivation with the context `gedo event id v1` over the postcard encoding of the event (clock, target, operation, result hash, author, labels, origin), truncated to 16 bytes. Live and reconstructed events use the same rule, so a retried publish or a graft stores an event once. A `TapeId` is derived the same way with `gedo tape id v1`. Readers recompute the id and refuse a record whose content does not match it.
- **Clocks on commit generations.** A clock is `(generation, sequence)`. The generation of a commit is 1 for a root and one more than its highest parent otherwise (the Lamport time of the commit graph). A reconstructed event of a commit has the commit's generation and its position among the commit's events; a live event recorded on top of a commit has that commit's generation plus one and the next sequence of the write-ahead log. Events are totally ordered by `(clock, id)`.
- **Authors and labels in the format.** The author is the BLAKE3 fingerprint (`gedo author v1`) of the Git e-mail, trimmed and with ASCII letters lowercased, and an author kind: a person, or an agent with an identifier of 1 to 64 bytes of `[a-z0-9._-]`. Labels follow ADR-0013 (at most 16, keys like agent identifiers, values of 1 to 256 bytes of UTF-8 without control characters, `gedo.` reserved), stored as a sorted map. A reconstructed event is a person's, without labels.
- **Operations.** An operation is a semantic delta of feature 003 or an identity merge (kept and merged ids); each event carries the entity's structural hash after it (none for a tombstone or a merge), which re-attachment (FEAT-065) matches on.
- **Three collections.** `refs/gedo/events` (entry: the event id, 32 hex digits), `refs/gedo/tapes` (entry: the tape id) and `refs/gedo/commit-tapes` (entry: the commit id followed by the tape id; an empty versioned record). A tape lists the event ids of one publish in recording order (at most 65,536) and names no commit. A publish writes the events, then the tape, then the index entry, and only then truncates the write-ahead log. Every record is a format-1 `RecordEnvelope`.
- **Reconstructed events stay local.** For a commit without index entries, the events are derived on demand from the commit's identity record, with the ids as recorded at the commit (never resolved through later aliases), and are never written to `refs/gedo/*`.

## Alternatives Considered

- **Random UUID v7 for live events** (the `uuid` row of the technical specification): a retried publish or a crash between steps would store one change under two ids.
- **A classic Lamport counter**: reconstruction on two machines would give different clocks and ids for the same commit.
- **One collection for events, tapes and the index** (`refs/gedo/tapes/` as written in ADR-0003): every reader would list every kind of entry, and ids would need a kind prefix.
- **An index record per commit listing its tapes**: two machines adding tapes to one commit would write different content under one key, which the union of collections refuses.
- **Sharing reconstructed events**: adds volume to every push and fetch without information the history does not already hold (rejected by the user).
- **Hashing the name and e-mail**: one person would split into two fingerprints on a change of display name (rejected by the user).

## Consequences

- Events, tapes and index entries travel through the existing `refs/gedo/*` refspecs and join by union; FEAT-065 re-attaches a rewritten commit by adding index entries, without touching tapes or events.
- The events collection grows by one entry per recorded event; ADR-0011's scale study covers 100,000 entries, and the `events` benchmark measures a publish into a chain of that size. Compaction is left to later releases.
- The CRDT and the sequencer (FEAT-066, FEAT-069) order events by `(clock, id)`; concurrent events of two branches share a generation and are ordered by id.
- Verified by property tests (ids, clocks, order), crash-injection tests of publish, a snapshot of reconstructed events over fixed commits of this repository on Windows and Linux, and the `decode_event_record` fuzz target.
