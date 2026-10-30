# Feature Specification: Semantic Lanes and Split Publish

**Feature Branch**: `feat/007-semantic-lanes`

**Created**: 2026-10-30

**Status**: Draft

**Input**: User description: "Semantic lanes and split publish (release v0.3.0, fourth feature, backlog item FEAT-067): classify events into refactor and feature lanes; gedo publish --split-lanes creates a refactor commit then a feature commit."

## Context

Features 003 to 006 give every code entity a stable identity, record each change of an entity as an **event**, keep those events attached to commits through rewrites, and converge concurrent work per entity. A developer's work between two commits usually mixes two kinds of change: **refactors** that keep behavior (a function renamed and every call updated, a type moved to another module) and **feature** work that changes it (a new function, a changed body or signature, a removal). Mixed in one commit, the refactor hides the feature in review: a rename touches fifty files and the three real changes are lost in the diff. The VISION promises "clean commits without effort": Gedo knows which changes are which, so it can publish them as two commits, the refactor first and the feature on top, without the developer staging hunks by hand.

This feature delivers that in two parts. **Lanes**: a pure, deterministic classification of the changes of a publish into the refactor lane and the feature lane. **Split publish**: a use case of the core that, given the base commit and the final tree of a publish, builds a refactor commit (the base plus only the refactor changes) and a feature commit on top of it (whose tree is exactly the final tree), and publishes the events of each lane as the tape of its commit.

Like the earlier features of v0.3.0, it has no end-user command yet: the `gedo publish --split-lanes` flag is wired by the command itself (FEAT-083, v0.4.0), which also moves the branch to the new commits. The users of this feature are that command and the Levix developers who build it; it is exercised through tests and benchmarks.

## Clarifications

### Session 2026-10-30

- Q: What does the refactor lane hold: moves and renames only, or also the updates of references they require (calls, imports, paths)? → A: Moves and renames together with the reference updates they require, so the refactor commit stands on its own.
- Q: What happens when a refactor commit cannot be built safely? → A: One commit with everything is published, as a plain publish, and the reason is reported.
- Q: Where does the refactor commit's message come from? → A: It is generated from its changes (for example `refactor: rename Invoice::sum to total; move tax to billing/tax.rs`), and the caller may give its own; the feature commit takes the caller's message.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Changes are classified into the refactor and feature lanes (Priority: P1)

A developer renames `Invoice::sum` to `Invoice::total`, updates its three callers, moves `tax` from `billing.rs` to `billing/tax.rs`, then adds a method `discount` and changes the body of `total`. Asked for the lanes of this work, Gedo answers: the rename, the move and the caller updates are **refactor**; the new method and the new body of `total` are **feature**. The same changes give the same lanes on every machine.

**Why this priority**: the lanes are the input of the split publish, of stack proposals by lane (FEAT-113) and of the review views that hide refactors; they must be exact and deterministic before any commit is built on them.

**Independent Test**: build the base and final identity results of scripted changes (each kind of delta, alone and mixed on the same entity) and check the lane of every change.

**Acceptance Scenarios**:

1. **Given** an entity moved or renamed with no other change, **When** the changes are classified, **Then** the change is in the refactor lane.
2. **Given** an entity created, removed, or with a changed signature or body, **When** the changes are classified, **Then** the change is in the feature lane.
3. **Given** an entity renamed and with a changed body, **When** the changes are classified, **Then** the rename is refactor and the body change is feature.
4. **Given** a body whose only change is a reference updated to follow a rename or a move of the same publish, **When** the changes are classified, **Then** that body change is in the refactor lane.
5. **Given** the same changes classified twice, or on Windows and Linux, **When** the results are compared, **Then** they are identical.

---

### User Story 2 - A split publish gives a refactor commit and a feature commit (Priority: P1)

The developer publishes the same work with lanes split. Gedo writes a **refactor commit** on the base whose tree is the base plus the refactor changes only, and a **feature commit** on the refactor commit whose tree is exactly the final tree. Reviewing the refactor commit shows only the rename, the move and the updated callers; reviewing the feature commit shows only the new method and the new body. The events of each lane are published as the tape of its commit, so `events_of_commit` on each commit returns its own lane's events with their authors and labels.

**Why this priority**: this is the user-visible promise of the feature (VISION, "clean commits without effort").

**Independent Test**: on scripted repositories with mixed work, split-publish and check the two commits: the parent chain, that the feature tree equals the final tree byte for byte, that re-indexing the refactor commit against the base gives only refactor-lane changes and the feature commit against the refactor commit only feature-lane changes, and the tapes of each commit.

**Acceptance Scenarios**:

1. **Given** work with changes in both lanes, **When** it is split-published, **Then** two commits are written, the refactor commit with the base as parent and the feature commit with the refactor commit as parent, and the feature commit's tree is the final tree.
2. **Given** the two commits, **When** their changes are computed by the identity index, **Then** the refactor commit has exactly the refactor-lane changes and the feature commit exactly the feature-lane changes.
3. **Given** the two commits, **When** their events are read, **Then** each has one tape with the events of its lane, in recording order, and the log is truncated once after both are indexed.
4. **Given** work with changes in one lane only, **When** it is split-published, **Then** one commit is written, as a plain publish would.
5. **Given** work that is split-published twice from the same base, final tree, message, author and events, **When** the results are compared, **Then** the commits and tapes are identical (the split is deterministic and idempotent).

---

### User Story 3 - A split that cannot be built safely is never wrong (Priority: P1)

Some work cannot be cut into a refactor commit that stands on its own: a function renamed inside a body that was also rewritten, a refactor whose intermediate version would not parse, references that cannot be told apart from feature changes. Gedo verifies every refactor commit it builds before writing it (it parses, and re-indexed against the base it gives exactly the refactor lane); when the verification fails, it publishes one commit with everything, as a plain publish, and reports why.

**Why this priority**: a refactor commit that silently contains feature code, or a feature commit that does not lead to the developer's tree, would break the trust the split exists for; the verification makes the split safe by construction.

**Independent Test**: scripted cases that defeat the builder (overlapping changes, a rename shadowed by a local of the same name, a move into a file that is also rewritten), each checked to give the fallback outcome with its reason and never a wrong commit.

**Acceptance Scenarios**:

1. **Given** work whose refactor commit would contain a syntax error, **When** it is split-published, **Then** one commit with everything is published and the reason names the file.
2. **Given** work whose refactor commit, re-indexed, would not give exactly the refactor lane, **When** it is split-published, **Then** one commit with everything is published and the reason names the entity.
3. **Given** any work, **When** it is split-published, **Then** the last commit's tree is always the final tree.

---

### Edge Cases

- No change at all between the base and the final tree: nothing to publish, as for a plain publish.
- Changes with no semantic delta (formatting, comments, files of languages without a parser): they are not refactors Gedo can verify, so they go to the feature commit.
- A file renamed or moved with no change to its entities: every entity in it is moved, so the file move is in the refactor lane.
- A new file that only receives moved entities (a split of a module): refactor; a new file with new entities: feature.
- An entity moved and then changed: the move in the refactor commit, the change in the feature commit.
- An entity created and renamed in the same publish: one creation, feature.
- An identity merge in the events (ADR-0007): it follows the lane of the change that produced it and never splits an entity in two.
- Live events that no longer match the final tree (recorded on a save that was later undone): the lanes come from the changes between the base and the final tree; events that no change accounts for go to the feature tape.
- Merge commits as base (more than one parent): the split builds on the base given by the caller, which is the commit the work started from.
- Very large publishes (a rename touching thousands of files): within SC-003.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The core MUST classify each change between a base and a final version into exactly one of two lanes, **refactor** or **feature**, per part of each entity (presence, place, name, signature, body, as in ADR-0017).
- **FR-002**: A move (place) and a rename (name) MUST be refactor; a creation, a removal, a signature change and a body change MUST be feature, except as FR-003 says.
- **FR-003**: A body or signature change whose only difference is references updated to follow a move or rename of the same publish (a call, a path, a type name), and a change of text outside entities (an import) whose only difference is such a reference, MUST be refactor (clarification), so the refactor commit stands on its own.
- **FR-004**: The classification MUST be pure core logic without I/O in `gedo-crdt` (ADR-0001), and MUST depend only on the base and final identity results and source tokens: the same input MUST give the same lanes on every machine.
- **FR-005**: A use case of the core MUST split-publish: given a base commit, a final tree, a message and the events of the log, it MUST write a refactor commit (parent: the base) and a feature commit (parent: the refactor commit, tree: the final tree), and publish the events of each lane as the tape of its commit, with the same guarantees as `publish` (ADR-0014: content-derived ids, idempotent steps, the log truncated only after every index entry is written).
- **FR-006**: The refactor commit's tree MUST be the base tree with the refactor-lane changes applied and nothing else; files and text that only the feature lane changes MUST keep their base content in it.
- **FR-007**: Before writing the commits, the use case MUST verify the refactor tree: every file it changed parses without a syntax error, and its changes against the base, computed by the identity index, are exactly the refactor lane; the feature commit's changes against the refactor commit MUST be exactly the feature lane.
- **FR-008**: When the verification fails, the use case MUST NOT publish a split: it MUST write one commit with the final tree and every event, as FR-009 says, and report the reason with the file or entity concerned (clarification).
- **FR-009**: When only one lane has changes, the use case MUST write one commit with the final tree and publish all events on it.
- **FR-010**: The refactor commit's message MUST be generated from its changes, deterministically (`refactor: ` followed by the renames and moves, in a stable order, shortened with a count when long), unless the caller gives one; the feature commit (and a single commit) MUST take the caller's message (clarification).
- **FR-011**: Each event of the log MUST go to the tape of exactly one commit: an event whose change is in the refactor lane to the refactor commit, every other event to the feature commit.
- **FR-012**: The use case MUST NOT move any branch or reference outside `refs/gedo/*`; it returns the commits it wrote, and the caller (FEAT-083) moves the branch.
- **FR-013**: The same base, final tree, message, author, time and events MUST give the same commits and tapes (deterministic and idempotent).
- **FR-014**: Errors (a base whose identities are not indexed, a store failure, a refused tape) MUST carry context and MUST NOT be swallowed; a failure before the index entries are written MUST leave the log intact.
- **FR-015**: The use case MUST log at info level the base, the commits written, the number of changes and events per lane, and the outcome (split, single commit or not split with its reason); the pure rules do not log.

### Key Entities

- **Lane**: refactor or feature.
- **Lane plan**: the changes of a publish, each with its lane, and the events of the log assigned to a lane.
- **Split outcome**: the commits written (refactor and feature, or one), the tape of each, or the reason the split was not possible.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the scripted mixed-work scenarios (renames with callers, moves within and across files, module splits, renamed and changed entities, new and removed entities, formatting-only changes), 100% of the lanes equal the expected ones and 100% of the split publishes give a refactor commit with exactly the refactor lane and a feature commit whose tree equals the final tree.
- **SC-002**: On every scenario built to defeat the builder, 100% of the outcomes are the safe fallback with a reason, and none is a wrong split; in property-based runs (at least 256 cases per property on every pull request) the last commit's tree always equals the final tree.
- **SC-003**: Split-publishing a change of 100 files and 1,000 events takes under 1 s at the 95th percentile on the reference machine, the budget of `gedo publish` (technical specification).

## Assumptions

- No end-user command is delivered. `gedo publish --split-lanes` (FEAT-083, v0.4.0) calls this use case, moves the branch and prints the outcome; stack proposals by lane (FEAT-113) reuse the lanes later.
- Gedo does not compile code: the refactor commit is verified to parse and to carry exactly the refactor lane, not to build. A refactor that needs a change Gedo cannot see (a macro, a build script) is the developer's to check, as for any commit.
- The author, committer and time of the commits are the repository's identity, as for every commit Gedo writes (`ObjectStore::write_commit`).
- The final tree is given by the caller (FEAT-083 builds it from the working tree); this feature does not read the working tree.
- The lanes are derived data: nothing new is stored, the event format does not change (ADR-0014) and the lane of an event is not a label.
- The reference machine of the technical specification is used for SC-003; CI benchmarks are reported but do not block pull requests.
