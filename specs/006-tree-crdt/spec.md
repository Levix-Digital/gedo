# Feature Specification: Tree CRDT

**Feature Branch**: `feat/006-tree-crdt`

**Created**: 2026-10-30

**Status**: Draft

**Input**: User description: "Tree CRDT with deterministic convergence (release v0.3.0, third feature, backlog item FEAT-066): set semantics for members of a structure, commutative move and edit, idempotent events; same result in any arrival order. These are the merge rules of ADR-0009, shared later by the semantic merge driver (FEAT-070), the shadow merge (FEAT-074), the sequencer (FEAT-069) and virtual streams (FEAT-068)."

## Context

Features 003 to 005 give every code entity a stable identity, record each change of an entity as an **event** with a content-derived id and a clock consistent with the history, and keep those events attached to commits through rebases and squashes. What is still missing is the rule that **combines concurrent work**: two developers (or two agents, or a developer and a grafted change) start from the same version and change the same files in parallel. Git merges their text line by line; Gedo's promise (VISION, ADR-0009) is to merge them by meaning, per entity: two methods added to the same `impl` never conflict, a function moved on one side and edited on the other ends up moved and edited, a change grafted to both sides is applied once, and the result is the same whoever merges and in whatever order the changes arrive.

This feature delivers that rule as a **tree CRDT**: the state of the code as a tree of entities (each entity with its presence, place, name, signature and body), the events of each side folded into that state, and a **convergence** of several sides over a common base that is deterministic, symmetric and idempotent. Where two sides change the same part of the same entity in different ways, the convergence never picks a winner silently: it reports a **conflict** with the competing values and the events that produced them (ADR-0009, "never hide a conflict"), and the layers above (the merge driver falls back to Git's textual merge for that node; the radar and the inbox show who collided).

Like the earlier features, this one has no end-user command. Its users are the later layers (merge driver, shadow merge, sequencer, streams, relay) and the Levix developers who build them; the capabilities are exercised through tests and benchmarks. Rendering a converged state back to source text, and placing members in the file, belong to the merge driver (FEAT-070).

## Clarifications

### Session 2026-10-30

- Q: One side changes an entity's signature and the other changes its body; do the two converge? → A: Both converge: the entity takes the signature of one side and the body of the other; the post-merge contract check (FEAT-076) reports a call that no longer fits.
- Q: One side removes an entity and the other changes it (or adds a member inside it): who wins? → A: Neither: it is a conflict, reported with both sides (ADR-0009, never hide a conflict).
- Q: Two sides create an entity with the same kind and qualified path: one entity or two? → A: One entity when their content is the same (equal structural hash): the two identities are linked, the lower id kept (ADR-0007, like the links of ADR-0016); a conflict ("added on both sides") when the content differs.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Independent changes to the same structure never conflict (Priority: P1)

Two developers start from the same commit. One adds a method `total` to `impl Invoice`; the other adds a method `tax` to the same `impl`, adds a field to another struct and removes an unused function. Converging their events over the common base gives a state with both methods, the new field and the function removed, and no conflict. Members of a structure (fields, methods, imports, variants, items of a module) behave as a **set**: each side's additions and removals apply, whatever their position in the file.

**Why this priority**: independent additions to the same structure are the most frequent textual conflict in a team (two people appending to the same `impl` or the same import list), and the first promise of the VISION.

**Independent Test**: build a base state and two event sets that add and remove different members of the same containers, converge them, and check the resulting members and the absence of conflicts.

**Acceptance Scenarios**:

1. **Given** two sides that each add a different member to the same container, **When** they converge, **Then** the container has both new members and no conflict is reported.
2. **Given** one side that removes a member and another side that adds a different member to the same container, **When** they converge, **Then** the removal and the addition both apply.
3. **Given** both sides that remove the same member, **When** they converge, **Then** it is removed once and no conflict is reported.
4. **Given** both sides that add an entity with the same kind, qualified path and content, **When** they converge, **Then** there is one entity, and when their identities differ, the two are linked with the lower id kept.

---

### User Story 2 - A move and an edit of the same entity commute (Priority: P1)

One developer moves a function to another module (or renames it); another edits its body. Converging the two sides gives the function in its new place, with its new name, and with the new body; the same result is reached whichever side is taken first. A change of an entity's place, its name, its signature or its body, made on one side only, always reaches the result.

**Why this priority**: refactors (moves and renames) running in parallel with feature work are the conflicts Git handles worst, because the text moved; this is what makes semantic lanes (FEAT-067) and grafts (FEAT-078) safe.

**Independent Test**: for every pair of changes on different parts of an entity (place, name, signature, body), build two sides with one change each, converge them in both orders, and compare the result with the expected entity.

**Acceptance Scenarios**:

1. **Given** a function moved to another file on one side and its body edited on the other, **When** they converge, **Then** the function is in the new file with the new body, and no conflict is reported.
2. **Given** a function renamed on one side and moved to another module on the other, **When** they converge, **Then** it has the new name in the new module.
3. **Given** a change made by one side only, **When** the sides converge in either order, **Then** the result has that change.
4. **Given** both sides that make the same change to the same entity (the same new body, the same new place), **When** they converge, **Then** the change applies once and no conflict is reported.

---

### User Story 3 - Incompatible changes are reported, never hidden (Priority: P1)

Two developers change the body of the same function differently, or rename it to different names, or move it to different places, or one removes it while the other edits it. Converging the two sides reports a conflict for that entity and that part, with each side's value and the events that produced it (their ids, authors and labels), and keeps every other change of the two sides. The converged state is still fully determined: the conflicting part has the base value until the conflict is resolved, and the conflict says so.

**Why this priority**: a wrong silent merge is worse than a visible conflict (ADR-0009); the merge driver needs to know exactly which nodes to hand back to Git's textual merge, and the radar needs to know who collided.

**Independent Test**: build pairs of sides with incompatible changes of each kind, converge them, and check the conflicts reported, the values and events in each, and that every unrelated change still applies.

**Acceptance Scenarios**:

1. **Given** two different new bodies of the same function, **When** the sides converge, **Then** one conflict is reported for its body, with both values and the events of each side, and the function keeps its base body in the converged state.
2. **Given** one side that removes an entity and another that changes it, **When** they converge, **Then** a conflict is reported for that entity, and the entity is kept with its base content in the converged state.
3. **Given** one side that removes a container and another that adds a member to it, **When** they converge, **Then** a conflict is reported for the container and the new member.
4. **Given** two sides that create different entities with the same kind and qualified path, **When** they converge, **Then** a conflict "added on both sides" is reported with both entities.
5. **Given** a conflict on one entity, **When** the sides converge, **Then** every change of the two sides to other entities applies as in Stories 1 and 2.

---

### User Story 4 - The same result in any arrival order, with every event applied once (Priority: P1)

The same events reach different machines in different orders and in different groupings: a change grafted to two branches, a tape attached to two commits after a cherry-pick, a fetch that brings some events twice, three sides merged in a different order. Every machine that converges the same base and the same sides reaches the same state and the same conflicts, byte for byte; an event present on several sides or listed twice applies once.

**Why this priority**: this is what makes it a CRDT. The merge driver, the shadow merge, the GitHub Action and the relay run on different machines and must agree; a graft must never apply a change twice.

**Independent Test**: property-based tests over generated bases and sides: shuffle the events of each side, duplicate events, copy events between sides, permute the sides and split the sides into consecutive convergences, and compare the results.

**Acceptance Scenarios**:

1. **Given** the same base and sides, **When** the events of each side arrive in any order, **Then** the converged state and the conflicts are identical.
2. **Given** an event listed twice in a side, or present in several sides (a graft, a cherry-pick), **When** the sides converge, **Then** it applies once and does not conflict with itself.
3. **Given** the same sides in any order, **When** they converge, **Then** the state is identical and the conflicts are identical, each naming its sides by their position in the input.
4. **Given** three or more sides, **When** they converge at once, **Then** each part of each entity takes the one value the sides agree on, or reports a conflict listing every distinct value with its sides.
5. **Given** a convergence run on Windows and on Linux, **When** the results are encoded, **Then** the bytes are identical.

---

### User Story 5 - Converge the work of commits and of the working tree (Priority: P2)

A later layer asks Gedo to converge two or more commits over their common base (the merge driver for a `git merge`, the shadow merge for the developer's branch against a teammate's), optionally with the live events not yet published on top of a side (the working tree, a virtual stream). Gedo reads the base state from the identity results of the base commit, the events of each side from the commits between the base and its tip (live events where tapes are attached, reconstructed events otherwise), folds and converges them, and returns the converged state and the conflicts.

**Why this priority**: it turns the pure rule into something the merge driver (FEAT-070) and the shadow merge (FEAT-074) can call on a real repository, and it is what the benchmark measures.

**Independent Test**: on scripted repositories (two branches from a common commit, with and without published events, with and without a graft, with live events on top of one branch), converge the tips and compare with the expected state and conflicts.

**Acceptance Scenarios**:

1. **Given** two branches from a common commit, **When** their tips are converged, **Then** the result is the convergence of the events of each branch over the base state.
2. **Given** a branch whose commits were published with Gedo and one made without Gedo, **When** they are converged, **Then** the live events and the reconstructed events give the same state as two reconstructed branches would, and conflicts name the live events with their authors and labels.
3. **Given** live events on top of a side, **When** the sides are converged, **Then** they take part like the events of that side's commits.
4. **Given** a base or a side whose identities are not indexed, **When** convergence is asked, **Then** it fails with an error that names the commit.

---

### Edge Cases

- A side with no events (one branch unchanged since the base): the result is the other sides' convergence; with only one side, it is that side's state.
- An entity created on one side and changed later on the same side: one side value (its last state); no conflict with itself.
- An entity created and removed on the same side (neutral, ADR-0016): it does not appear in the result and takes part in no conflict.
- A change made and then reverted on the same side: the side's value equals the base, so the side did not change that part.
- Identity merges (ADR-0007) in the events or the base: every id is resolved to the kept id before folding, so the two ids of one entity are one entity on every side.
- A member moved from one container to another on one side while the other side removes the destination container: a conflict for the destination container.
- A rename on one side and a move on the other where the moved qualified path does not end with the old name (the entity's container was renamed too): the result keeps the new container and the new name; when they cannot be combined, a conflict for the entity's place.
- Events in a side that refer to an entity unknown to the base and never created on that side (a partial fetch, a corrupted tape): a reported error naming the event, never a silent drop.
- Sides of thousands of commits (a long-lived branch): the convergence reads each commit's events once and stays within SC-004.
- The order of members inside a container: not part of the converged state; the merge driver keeps each side's order of its own members (FEAT-070).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CRDT MUST represent the state of the code as a set of entities keyed by identity, each with its kind and five parts: **presence** (present or removed), **place** (unit, file and container), **name**, **signature** and **body**, plus its structural hash when it is known.
- **FR-002**: The state of a side MUST be the base state with the side's events folded in their total order (clock, then id, ADR-0014): a creation sets the entity present with its kind and place; a move sets its place; a rename sets its name; a signature or body change sets that part; a removal sets it removed; an identity merge makes the merged id an alias of the kept one.
- **FR-003**: Events MUST be deduplicated by id before folding, within a side and across sides: an event present twice, or on several sides, MUST apply once.
- **FR-004**: Convergence MUST take a base state and one or more sides and give, for every part of every entity, the base value when no side changed it, the side value when the sides that changed it agree on one value, and a **conflict** otherwise; a side changes a part when its value differs from the base value.
- **FR-005**: Members of a container MUST behave as a set: an addition or removal by one side MUST apply whatever the other sides did to other members of the same container; members MUST be keyed by identity, never by position.
- **FR-006**: Changes to different parts of the same entity by different sides MUST commute: place and name with any other part, signature and body with each other (clarification); the structural hash of an entity whose parts come from different sides is unknown until the merge driver renders it.
- **FR-007**: A removal by one side and a change of any part (or a new member inside the entity) by another MUST be a conflict; a removal by several sides MUST NOT.
- **FR-008**: Two entities created by different sides with the same kind and qualified path MUST be one entity when their structural hashes are equal (their identities linked, the lower id kept, ADR-0007) and a conflict "added on both sides" otherwise.
- **FR-009**: Each conflict MUST name the entity, the part, the base value, each distinct value with the sides that hold it and the ids of the events that set it; the part MUST keep its base value in the converged state (an entity added on both sides keeps neither), so the state is always fully determined.
- **FR-010**: The result MUST depend only on the base and the set of events of each side: the same input MUST give the same state and conflicts whatever the order of the events, the duplicates, the order of the sides (conflicts name sides by input position) and the platform.
- **FR-011**: Convergence MUST accept any number of sides at once; with one side, it MUST return that side's state with no conflict.
- **FR-012**: A use case of the core MUST converge commits: given a base commit and the tips of the sides, it MUST build the base state from the identity results of the base, the events of each side from its commits after the base (through `events_of_commit`, so live events where tapes are attached and reconstructed events otherwise), optionally followed by live events given by the caller, and return the converged state and conflicts.
- **FR-013**: The converged state MUST be readable per entity and per container (the members of a container), so the merge driver (FEAT-070) can render the nodes of one file and hand conflicting nodes to Git's textual merge.
- **FR-014**: Errors (a commit whose identities are not indexed, a missing tape or event, an event about an entity neither in the base nor created on its side) MUST carry context and MUST NOT be swallowed.
- **FR-015**: The convergence rules MUST be pure core logic without I/O in `gedo-crdt` (ADR-0001), covered by property-based tests of determinism, symmetry, idempotence and commutativity; nothing new is stored and no shared format changes.
- **FR-016**: The use case MUST log, at info level, the base and the number of sides, events and conflicts, and at debug level the commits read per side; the pure rules do not log.

### Key Entities

- **Entity state**: one entity with its kind and its five parts (presence, place, name, signature, body) and, when known, its structural hash.
- **Tree state**: the entity states of a version of the code, keyed by identity, with the members of each container derived from the places.
- **Side**: a set of events made after a common base (the commits of a branch, live events, a stream).
- **Convergence**: the tree state that combines a base and several sides, with the list of conflicts.
- **Conflict**: an entity and a part on which sides disagree, with the base value, each distinct value, the sides that hold it and the events that set it; or two different entities added on both sides at the same qualified path.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the scripted concurrent-change scenarios (additions to the same structure, removals, moves, renames, edits, removal against change, additions on both sides, grafts and cherry-picks, three sides), 100% of the results equal the expected state and conflicts, and no case that changes the same part of an entity differently converges without a conflict.
- **SC-002**: In 100% of the property-based runs (at least 256 cases per property on every pull request), the result is identical under any order of events, duplicated events, events shared between sides, any order of sides, and on Windows and Linux.
- **SC-003**: Converging a side with itself, or with an empty side, gives that side's state with no conflict in 100% of the property-based runs.
- **SC-004**: Converging two sides of 10,000 events each over a base of 10,000 entities takes under 50 ms at the 95th percentile on the reference machine, so it fits inside the 200 ms budget of the merge driver on one file (technical specification).
- **SC-005**: Converging two branches of 100 commits each from their tips, with their identities indexed, takes under 500 ms at the 95th percentile on the reference machine.

## Assumptions

- No end-user command is delivered. The merge driver (FEAT-070), the shadow merge (FEAT-074), the sequencer (FEAT-069), virtual streams (FEAT-068) and the relay call these rules later; this feature is exercised through tests and benchmarks.
- Rendering the converged state back to source, the order of members in a file and the textual fallback for conflicting nodes belong to the merge driver (FEAT-070).
- The common base of the sides is given by the caller (the merge base Git computes); several merge bases (criss-cross merges) are handled by the merge driver as Git does.
- Recording conflicts for the inbox (v0.8.0) and showing them in `gedo radar` (FEAT-074) are later features; this feature returns them.
- The parts of an entity are those the identity layer already tracks (feature 003): structural, signature and body hashes, name, qualified path, file and unit; no new hash is computed.
- Nothing is written: the convergence is derived data, computed on demand, and adds no shared collection and no change to the event format (ADR-0014).
- The reference machine of the technical specification is used for SC-004 and SC-005; CI benchmarks are reported but do not block pull requests, while determinism checks do.
