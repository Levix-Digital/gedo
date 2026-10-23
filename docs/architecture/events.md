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
- Separates events into **semantic lanes**: the refactor lane (renames, method extraction; idempotent with respect to business rules) and the feature lane (changes to logic and meaning).
- Provides **virtual streams**: switching context or isolating an experiment in memory, without `git stash` or `git checkout`.
- Hosts the deterministic sequencer that orders events from several developers (Lamport and CRDT rules) before a canonical commit is materialized.
- Does not read or write files; the WAL and the tapes are persisted through ports.

## Structure

Pure data structures and functions: event, tape, clock, tree CRDT, lanes, streams, sequencer.

## Behavior

- Each event carries `event_id`, `lamport_clock`, `target_node_id`, `operation` and `author_fingerprint`.
- Tapes are serialized with Postcard, prefixed with a format version, and stored at `refs/gedo/tapes/<commit-sha>` by the store adapter.
- Convergence is deterministic: the same set of events produces the same tree on every machine, whatever their arrival order.

## Interfaces

- Called by `gedo-app` (ingest, publish, graft, status).
- Persists through the write-ahead log and reference store ports.

## Decisions

- [ADR-0001](../adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md): CRDT and sequencer are pure core logic.
- [ADR-0003](../adr/0003-git-object-store-as-the-canonical-datastore.md): tapes live in `refs/gedo/tapes/`.
