# Research: Tree CRDT

Decisions taken while planning `specs/006-tree-crdt`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0016, `docs/product/TECH_SPEC.md`) and the clarifications of 2026-10-30 (signature and body changes on different sides, removal against change, entities added on both sides).

## R1. Where each step lives

- **Decision**:
  - `gedo-crdt` gains the pure rules in a new module `tree`: the entity state and its parts (R3), the fold of a side's events into a tree state (R4), the convergence of a base and several sides (R5 to R7) and the result with its conflicts (R8). Inputs are plain values (a base tree state, the events of each side), the output a `Convergence` value; no I/O.
  - `gedo-app::events` gains the use case in a new module `events/converge.rs` (an `impl` block of `EventLog`): build the base tree state from the identity results of the base commit, collect the events of each side, call `gedo-crdt` (R9).
- **Rationale**: the split of features 004 and 005 (model in `gedo-core`, rules in `gedo-crdt`, orchestration in `gedo-app`); the convergence is the part that needs property-based tests without Git, and the crate was named for it (ADR-0001).
- **Alternatives considered**: the rules inside `gedo-app` (property tests would need a repository); a new crate (the event layer already lives in `gedo-crdt`, and the sequencer of FEAT-069 joins it there).

## R2. The convergence model: three-way per part over a common base

- **Finding**: every caller has a common base. The merge driver gets base, ours and theirs from Git; the shadow merge and streams know the merge base of the branches; the sequencer of FEAT-069 orders events that all descend from published commits. Events carry a history clock (generation, sequence, ADR-0014) but no version vector, so concurrency between two arbitrary events cannot be read from the events alone, and the event format is not changed by this feature.
- **Decision**: a **state-based, three-way convergence per part**: each side's events are folded into the side's tree state (R4); for every part of every entity, a side **changed** the part when its value differs from the base value; the converged value is the base value when no side changed it, the common value when every side that changed it agrees, and a **conflict** otherwise (R5). Membership in a container is a property of each member (its place), so members behave as a set without a separate structure.
- **Rationale**: this is a join over the lattice "unchanged < one value < conflict" per part, so the result is commutative, associative over sides and idempotent by construction; it never picks a winner silently (ADR-0009); it needs only what events and identity results already hold; and it is the same rule the merge driver applies to base, ours and theirs.
- **Alternatives considered**:
  - **Last writer wins by clock**: deterministic but silently drops one side's change of the same part, which ADR-0009 forbids; clocks of different branches are not comparable in meaning.
  - **Operation-based tree CRDT with a replicated move operation** (undo, apply, redo in a total order; cycles resolved by dropping a move): designed for real-time editors without a base; it needs causal delivery and a total order of every operation, resolves conflicting moves silently, and would be a new shared format.
  - **Version vectors per event**: would show concurrency without a base, but changes the frozen event format of ADR-0014 for a need no caller has.

## R3. The parts of an entity

- **Decision**: an entity state holds its `kind` and five parts, each compared on its own:
  - **presence**: present or removed;
  - **place**: unit, file path and **container path** (the qualified path without the entity's own name);
  - **name**: the entity's own name;
  - **signature**: the signature hash;
  - **body**: the body hash;
  plus the **structural hash** when it is known (R6). The qualified path is the container path joined with the name; it is split at its last top-level separator (for Rust `::` outside angle brackets, so `crate::m::<impl fmt::Display for X>::fmt` has the container `crate::m::<impl fmt::Display for X>`); each language added later (EPIC-004) states its separator. The container path is the parent of the qualified path, so a method's container is its type (`crate::Invoice::total`), not the implementation block (`crate::<impl Invoice>`).
- **Base values** come from the identity results of feature 003 (`AnchoredNode`: kind, name, qualified path, signature, body and structural hashes; `BlobAstMap`: unit and path). **Event values**: a creation sets presence, kind and place, and the name from its qualified path; a move sets the place; a rename sets the name from the qualified path it records; a signature or body change sets that hash; a removal sets presence to removed; every event with a result hash sets the structural hash.
- **Rationale**: these are the parts the identity layer already tracks and the deltas already name (feature 003), so the CRDT adds no hash and no format; splitting place and name is what lets a move and a rename of the same entity commute (US2, scenario 2).
- **Alternatives considered**: the whole entity as one register (every move against every edit would conflict, against ADR-0009); the qualified path as one part (a move and a rename would conflict); container ids instead of container paths (events record locations, not container ids).

## R4. The fold of a side

- **Decision**: a side is a list of events. Its tree state is the base state with the events applied: events deduplicated by id (`gedo_crdt::event_id`), ids resolved through the identity merges of the base and of every side (the merged id becomes an alias of the kept one, ADR-0007), then applied in the total order (clock, id) of ADR-0014. An entity created and removed on the same side is **neutral** (ADR-0016) and leaves no trace. An event about an entity that is neither in the base nor created earlier on its side is an error naming the event (FR-014). For each part, the fold keeps the id of the last event that set it on that side, for the conflicts (R8).
- **Rationale**: deduplication and the total order make the side state independent of arrival order and duplicates (US4); resolving aliases on every side before folding makes an identity merge seen by one side apply to all.
- **Alternatives considered**: applying events in arrival order (depends on the transport); dropping unknown events (would hide a partial fetch).

## R5. The convergence of a part

- **Decision**: for each entity present in the base or created on a side, and each part: the values of the sides that changed the part (differs from the base) are collected as a set; an empty set keeps the base value; one value is taken; two or more distinct values give a **conflict** on that part, and the part keeps its base value in the converged state. Equal values from several sides (the same change made twice, a graft, a cherry-pick) are one value. With one side, the result is that side's state.
- **Rationale**: FR-004, FR-009, FR-011; agreement by value, not by event, is what makes a graft applied on both sides converge even when its events were reconstructed with different ids.
- **Alternatives considered**: keeping a side's value in the converged state for a conflicting part (the state would depend on which side is "ours", breaking symmetry).

## R6. Which parts commute and which conflict

- **Decision**:
  - **Place and name** commute with every other part. A move on one side and a rename on another give the moved place with the new name; the qualified path is rebuilt from the converged container path and name.
  - **Signature and body** commute: one side's new signature and another side's new body converge into both (clarification of 2026-10-30; the post-merge contract check, FEAT-076, reports calls that no longer fit). When every changed part of an entity comes from one side (or from sides that agree), the converged structural hash is that side's; when parts come from different sides it is **unknown**, because only the merge driver can render and hash the combined entity.
  - **Presence**: a removal by one side and any change of another part by another side is a conflict on presence (clarification); removals by several sides agree.
  - **Container**: when a container is removed on one side and another side adds or moves a member into it (its converged container path names the removed container), the container's presence conflicts and the member is listed with it.
- **Rationale**: ADR-0009 (members as a set, move and edit commute, never hide a conflict); a removal against an edit has no safe automatic answer.

## R7. Entities added on several sides

- **Decision**: two entities created on different sides with different ids, the same kind and the same qualified path are compared by structural hash: equal hashes make them one entity, with the two identities linked and the lower id kept (ADR-0007, as the links of ADR-0016); different hashes give an **added on both sides** conflict naming both entities, and neither is in the converged state. Entities created with the same id (a grafted creation, a cherry-pick) are one entity by construction. Implementation blocks are left out of this rule: several blocks for one type are legal and share one qualified path.
- **Rationale**: identities derive from content when an entity first appears (ADR-0007), so the same new function written twice usually has the same id already; the hash comparison covers the rest without a new rule.

## R8. The result and its conflicts

- **Decision**: a `Convergence` holds the converged `TreeState` (entities by id, with a per-container member index for FR-013), the identity links of R7, and the conflicts sorted by entity then part. A `Conflict` names the entity, the part, the base value, and each distinct value with the sorted positions of the sides that hold it and the ids of the events that set it on each side (the last event of R4); the authors and labels of those events are read by the caller through the event log. Sides are named by their position in the input, so permuting the sides permutes the positions and nothing else.
- **Rationale**: FR-009, FR-010, FR-013; event ids are enough for the radar and the inbox to show who collided, without copying events into the result.

## R9. The use case: converging commits

- **Decision**: `EventLog::converge(base, sides, identities)`, where each side is a tip commit with optional live events on top (FR-012). For each side, the commits reachable from its tip and not from the base are listed with their generations (the walk of `EventLog::generation`) and processed in generation order; each one's events come from `events_of_commit` (live events where tapes are attached, the remainder and reconstructed events otherwise). The base tree state is built from the identity results of the base commit for the files changed on any side (a comparison of each side commit's file table with its first parent's), and for the rest of the base on demand when an event names an entity outside them. A commit that is not indexed is an error naming it (`IndexError::NotIndexed`).
- **Rationale**: the cost scales with the change, not with the repository (SC-005); `events_of_commit` already gives live events back after rebases and squashes (feature 005), so conflicts carry the original authors and labels.
- **Alternatives considered**: the full base state always (1,000 maps read for a merge of one file); the tip's identity results as the side state without events (loses attribution and cannot take live events on top).

## R10. Performance

- **Decision**: the fold and the convergence are linear in the number of events and touched entities (`BTreeMap` by id; untouched base entities are shared, not copied). Two benchmarks: `tree` in `gedo-crdt` (two sides of 10,000 events each over a base of 10,000 entities, SC-004) and `converge` in `gedo-app` (two branches of 100 commits, SC-005), both reported by the non-blocking bench job; the reference machine run is asked of the user at the release candidate.

## R11. Tests

- **Property-based** (`proptest`, at least 256 cases per property): generated bases and sides of creations, moves, renames, edits, removals, identity merges and shared events; properties: arrival order, duplicates, events copied between sides, permutation of sides (results equal up to side positions), convergence with itself and with an empty side, associativity of converging sides one at a time against all at once when no conflict arises, and no lost change (every part changed by one side only reaches the result).
- **Scenario tests**: one test per acceptance scenario of US1 to US4, with the expected state and conflicts.
- **Use case**: scripted repositories (two branches from a common commit; published and unpublished; a graft; live events on top; a side not indexed), with an `insta` snapshot of the convergence, identical on Windows and Linux.

## R12. Recorded decision

- **Decision**: **ADR-0017** "Tree CRDT as a three-way convergence of entity parts" records R2, R3, R6 and R7, and amends ADR-0009 (its merge rules, made precise: the parts, removal against change, entities added on both sides). The constitution is regenerated (accepted ADRs are sources).
