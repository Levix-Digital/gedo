---
id: 7
title: Deterministic node identity derived from history
status: accepted
date: '2026-10-24'
version: v0.1.0
subsystems: [identity, core]
supersedes: []
superseded_by: []
amends: [2]
amended_by: [12]
---

# ADR-0007: Deterministic node identity derived from history

## Context

ADR-0002 gives new nodes a random UUID v7. A random id is only shared if the machine that created it publishes it before anyone else indexes the same code. In practice the same entity is indexed independently in many places: a fresh clone, commits made by teammates without Gedo, a server-side merge, the same function written on two branches, a graft into a colleague's branch. Each would receive a different id, and the history of one entity would split into several at merge time, breaking `trace`, `graft`, the merge driver and the threads.

## Decision

- **Identity is a pure function of content and history.** A new node's `AstNodeId` is a UUID v8 built from `BLAKE3("gedo-node-v1" ‖ kind ‖ qualified_path ‖ structural_hash)` of the node at the moment it first appears. A node that is matched to an earlier version (tiers 1 and 2 of the matching) keeps the earlier id. The same history therefore produces the same ids on every machine, whether or not Gedo was running when the code was written.
- **The matching is versioned.** The algorithm version is stored with every `BlobAstMap`. Improving the algorithm creates a new version; ids already published are kept, never recomputed.
- **Independent identical creations converge.** Two developers who create the same entity with the same content get the same id, which is what the CRDT needs to deduplicate it.
- **Reconciliation at merge.** When a merge finds two different ids for what the matching considers one entity (for example, the same function created with different bodies on two branches and later made equal, or a re-indexed history), the lower id is kept and the other becomes an alias, recorded as an `IdentityMerged` event so every machine applies the same rule.
- **Event ids** of events reconstructed from history (commits made without Gedo) are derived the same way from their target, operation, payload, Lamport clock and author fingerprint. Events recorded live keep their id when they travel, which is what makes grafts idempotent.
- This replaces "UUID v7 for new nodes" in ADR-0002; UUID v4 and v7 ids already stored remain valid when read.

## Alternatives Considered

- **Random ids (v7) with first-publisher-wins**: simple, but every independent indexing diverges and needs a network round trip to converge, contrary to offline-first.
- **Ids derived from the commit where the node first appeared**: deterministic, but a graft or a cherry-pick gives the same entity a different origin commit on each branch.
- **Ids equal to the qualified path**: deterministic and readable, but a rename would change the identity, which is exactly what Gedo must survive.

## Consequences

- Re-indexing any repository, on any machine, reproduces the same ids; the determinism metric of the vision tests it on the reference corpus.
- An entity deleted and re-created later with exactly the same path and content gets its old id back; it is treated as a resurrection.
- The alias table is part of the shared data and must be compact and fast to resolve.
- The identity phase (Phase 2) implements the derivation and the reconciliation, with property-based tests that index the same history in different orders and on different branches.
