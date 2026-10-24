---
title: Event Log and Convergence
summary: Records semantic events and converges concurrent changes deterministically.
code: [crates/core/gedo-crdt]
---

# Event Log and Convergence

Records semantic events and converges concurrent changes deterministically.

## Responsibilities

- Turns semantic deltas into events with Lamport clocks.
- Groups events into event tapes, one per canonical commit or Gedo checkpoint.
- Converges concurrent events with a hierarchical tree CRDT over syntax nodes, with deterministic conflict resolution.
- Separates events into **semantic lanes**: the refactor lane (renames, moves, extractions, formatting; behavior-preserving) and the feature lane (changes to control flow, types and logic). `gedo publish --split-lanes` turns them into two consecutive commits, refactor first.
- Provides **virtual streams** (`gedo stream <name>`): switching context or isolating an experiment in memory, without `git stash` or `git checkout`.
- Provides the **semantic merge** used by the merge driver, the shadow merge and the sequencer: members of a structure behave as a set, a move and an edit of the same node commute, and an event present on both sides is applied once.
- Hosts the deterministic sequencer that orders events from several developers (Lamport and CRDT rules) before a canonical commit is materialized.
- Does not read or write files; the WAL and the tapes are persisted through ports.

## Structure

Pure data structures and functions: event, tape, clock, tree CRDT, lanes, streams, sequencer.

## Behavior

- Each event carries `event_id`, `lamport_clock`, `target_node_id`, `operation` and `author_fingerprint`.
- Events are content-addressed and stored by id; tapes group event ids, and an index maps commits to tapes. Commits rewritten by a rebase or a server-side squash are re-attached to their events by semantic diff and event matching (ADR-0008).
- All records are serialized with Postcard and prefixed with a format version.
- Convergence is deterministic: the same set of events produces the same tree on every machine, whatever their arrival order.

## Interfaces

- Called by `gedo-app` (ingest, publish, graft, status).
- Persists through the write-ahead log and reference store ports.

## Decisions

- [ADR-0001](../adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md): CRDT and sequencer are pure core logic.
- [ADR-0003](../adr/0003-git-object-store-as-the-canonical-datastore.md): tapes live in `refs/gedo/tapes/`.
- [ADR-0008](../adr/0008-gedo-data-robust-to-history-rewrites.md): content-addressed events and re-attachment after rewrites.
- [ADR-0009](../adr/0009-semantic-merge-driver-and-shadow-merge.md): merge rules shared by the driver and the shadow merge.
