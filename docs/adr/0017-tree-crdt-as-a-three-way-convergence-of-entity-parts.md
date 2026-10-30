---
id: 17
title: Tree CRDT as a three-way convergence of entity parts
status: accepted
date: '2026-10-30'
version: v0.3.0
subsystems: [events]
supersedes: []
superseded_by: []
amends: [9]
amended_by: []
---

# ADR-0017: Tree CRDT as a three-way convergence of entity parts

## Context

ADR-0009 states the merge rules the semantic merge driver, the shadow merge and the relay share: members of a structure (fields, methods, imports, variants) behave as a set, a move and an edit of the same node commute, an event present on both sides is applied once, and a conflict is never hidden. It does not say how a state is built from events, which changes commute and which conflict, what happens to a removal against an edit or to the same entity added on two sides, nor how the result stays identical on every machine. Events (ADR-0014) carry a history clock (generation, sequence) but no version vector, so two events of different branches cannot be told concurrent from the events alone, and the event format is frozen. Every caller has a common base: Git gives the merge driver base, ours and theirs, and the shadow merge, streams and the sequencer know the merge base of what they combine. The identity layer (feature 003) already tracks, per entity, its kind, name, qualified path, file, unit and the signature, body and structural hashes.

## Decision

- **Parts.** An entity's state has a kind and five parts compared on their own: presence, place (unit, file and container path), name, signature hash and body hash, plus the structural hash when known. The qualified path is the container path and the name, split at its last separator outside angle brackets (`::` for Rust); for a method, the container path is its type's path. The base state comes from the identity results of the base commit.
- **Fold.** A side's state is the base with its events applied once each (deduplicated by event id), ids resolved through the identity merges of the caller and of every side (the lower id kept, ADR-0007), in the total order (clock, id) of ADR-0014. A creation sets presence, kind, place and name; a move sets place and name; a rename sets the name; a signature or body change sets that hash; a removal sets presence. An entity created and removed on one side leaves no trace. An event about an entity neither in the base nor created earlier on its side is an error.
- **Convergence per part.** For every part of every entity, a side changed the part when its value differs from the base value. The converged value is the base value when no side changed it, the common value when the sides that changed it agree, and a conflict otherwise; a conflicting part keeps its base value. Any number of sides converge at once; the result depends only on the base and the set of events of each side, and conflicts name sides by their input position.
- **What commutes.** Place and name commute with every part; signature and body commute with each other (the user's choice of 2026-10-30; the post-merge contract check, FEAT-076, reports a call that no longer fits). The structural hash of an entity whose parts come from different sides is unknown until the merge driver renders it.
- **What conflicts.** A removal on one side against a change of any part on another (the entity keeps its base state); a container removed on one side while another side adds or moves a member into it (the container keeps its base state, the conflict names the members); different entities with the same kind and qualified path added on several sides, unless their structural hashes are equal, in which case they are one entity with the lower id kept and the other linked to it. Implementation blocks are left out of the last rule, since several blocks for one type are legal.
- **Interfaces.** `gedo_crdt::tree` holds the pure rules (`TreeState`, `fold`, `converge`, `converge_with`, `Convergence`, `Conflict`); `EventLog::converge(base, sides)` reads the base state for the files the sides changed (the whole base when an event names an entity outside them) and the events of each side's commits through `events_of_commit`, plus live events given by the caller. Nothing is stored and no format changes.

## Alternatives Considered

- **Last writer wins by clock**: deterministic, but silently drops one side's change of the same part, which ADR-0009 forbids; clocks of two branches say nothing about intent.
- **An operation-based tree CRDT with a replicated move operation** (undo, apply and redo in a total order, cycles resolved by dropping a move): built for real-time editors without a base; it needs causal delivery of every operation, resolves conflicting moves silently and would be a new shared format.
- **Version vectors in events**: would show concurrency without a base, but change the frozen event format for a need no caller has.
- **The whole entity as one register**: every move against every edit would conflict.
- **Signature against body as a conflict**: safer for contracts, but turns the most common refactor-against-feature case into a textual merge; rejected by the user.

## Consequences

- Independent additions to the same structure, moves, renames and edits of different parts converge without conflict; incompatible changes come back with the sides and the event ids (and so the authors and labels) that made them, for the merge driver's textual fallback (FEAT-070), the radar (FEAT-074) and the inbox.
- Converging never writes; a graft or a cherry-pick applied on several sides converges because equal values agree, whatever the ids of their events.
- Known limits: the order of members in a file is not part of the state (the merge driver keeps each side's order); a removed implementation block is not a container of its methods, whose container path is the type's, so removing a block while another side adds a method to it is found only when the type itself is removed.
- Two sides of 10,000 events over 10,000 entities converge in about 44 ms at p50 on the Linux development container (`cargo bench -p gedo-crdt --bench tree`), and two branches of 100 commits in about 51 ms (`cargo bench -p gedo-app --bench converge`).
- Verified by the property tests of `crates/core/gedo-crdt/tests/tree.rs` (arrival order, duplicates, shared events, permuted sides, convergence with itself, no lost change) and the scripted branches of `crates/core/gedo-app/tests/converge.rs` with a snapshot of the convergence.
