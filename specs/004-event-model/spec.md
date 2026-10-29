# Feature Specification: Event Model

**Feature Branch**: `feat/004-event-model`

**Created**: 2026-10-29

**Status**: Draft

**Input**: User description: "Event model (release v0.3.0, first feature, backlog items FEAT-063 and FEAT-064): semantic events with Lamport clocks and content-addressed ids, deterministic ids for events reconstructed from commits made without Gedo; each event carries its operation, its target entity, the author's fingerprint, the kind of author (a person or a coding agent, with the agent's identifier) and a map of opaque labels (key and value) that tools built on Gedo set and filter (for example the dev-cycle protocol's dc.plan and dc.milestone), all in the stored format from the start because formats freeze at v1.0.0; events grouped in tapes, one per publish, with an index from commits to tapes, stored as content-addressed data robust to history rewrites (ADR-0008, ADR-0011, ADR-0013). Re-attachment after rebase, amend and squash (FEAT-065) is a later feature."

## Context

Feature 003 describes each change of the code as **semantic deltas** of entities with stable identities (created, moved, renamed, signature mutated, body modified, tombstoned) and records **identity merges** at merge commits. This feature, the first of v0.3.0 (Event log and convergence), turns those changes into **events**: immutable, content-addressed records of who changed which entity, how, and when in logical time. Events are the shared history that every later layer reads: re-attachment after rewrites (FEAT-065), the tree CRDT (FEAT-066), lanes (FEAT-067), virtual streams (FEAT-068), the sequencer (FEAT-069), and in v0.4.0 `gedo trace`, `gedo graft` and the merge driver.

Events are recorded **live** while someone works (first in the local write-ahead log, then packed into a **tape** when the work is published as a commit) or **reconstructed** from the Git history for commits made without Gedo. Reconstruction is deterministic: every machine derives the same events, with the same ids, from the same commit (ADR-0007). A **commit index** says which tapes belong to which commit; tapes and events do not depend on commit ids, so a later rewrite only needs a new index entry (ADR-0008).

Every event also carries an **author kind** (a person, or a coding agent with its identifier) and **opaque labels** that tools built on Gedo set and filter without Gedo knowing what they mean (ADR-0013). The Levix dev-cycle protocol uses them to tie events to its plans, milestones and gates (`dc.plan`, `dc.milestone`), and the parallel agents of v0.5.0 to tell each agent's work apart. Both are part of the stored format now, because stored formats freeze at v1.0.0 (FEAT-127) and adding them later would split the history into two formats.

Like the earlier features, this one has no end-user command (`gedo publish`, `gedo stream` and the `--json` output arrive with the CLI in v0.4.0). Its users are the later layers listed above and the Levix developers who build them; the capabilities are exercised through tests and benchmarks.

## Clarifications

### Session 2026-10-29

- Q: Are the events reconstructed for commits made without Gedo stored and pushed with the shared data, or derived on demand on each machine? → A: [NEEDS CLARIFICATION: recommended: derived on demand and kept only as local derived data, like the identity results (ADR-0012); only live events and their tapes are shared.]
- Q: When work is published, does the tape keep every event recorded live (each save), or only one consolidated event per change of the commit? → A: [NEEDS CLARIFICATION: recommended: every recorded event, in recording order, so `gedo playback` (v0.4.0+) can show how a delivery evolved; size is bounded by the limits and measured by the benchmarks.]
- Q: What is the person's identity that the author fingerprint hashes? → A: [NEEDS CLARIFICATION: recommended: the Git author e-mail, trimmed and lowercased, so a change of display name or of letter case does not split one person into two; the identity itself is never stored (technical specification, Security and Privacy).]

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A change becomes an event with a stable, content-derived id (Priority: P1)

A developer edits code: renames a method, changes a function's body, moves a struct. Each semantic delta that results, and each identity merge, becomes an event that records the target entity, the operation with its details, the entity's structural hash after the change, a logical clock, the author's fingerprint, the author kind and the labels in effect. The event's id is derived from its content, so the same event always has the same id, wherever and however often it is written.

**Why this priority**: every later layer of v0.3.0 and v0.4.0 consumes events; their id and content must be stable before anything is built on them.

**Independent Test**: build events from the deltas of scripted edits, encode and decode them, and check that ids are reproducible, that any change of content changes the id, and that the encoded form is the same on Windows and Linux.

**Acceptance Scenarios**:

1. **Given** a semantic delta of an entity, **When** it is turned into an event, **Then** the event names the entity's identity, the operation (created, moved, renamed, signature mutated, body modified, tombstoned) with the details of the delta, and the entity's structural hash after the change (none for a tombstone).
2. **Given** an identity merge found at a merge commit, **When** it is turned into an event, **Then** the event records the kept and the merged identities, so every machine can apply the same alias.
3. **Given** two events with the same content, **When** their ids are computed on any machine, **Then** the ids are equal; **Given** two events that differ in any field (operation, target, clock, author, labels), **Then** their ids differ.
4. **Given** an event, **When** it is encoded and decoded, **Then** the result is equal to the original and the encoded bytes are identical on every platform.

---

### User Story 2 - Events say who made them: a person or an agent, with opaque labels (Priority: P1)

A developer works directly, and in the same repository a coding agent works for a plan of the dev-cycle protocol. The developer's events carry the developer's fingerprint and the author kind "person". The agent's events carry the same person's fingerprint (the agent runs on that person's Git identity), the author kind "agent" with the agent's identifier, and the labels the agent's tool set (for example `dc.plan=gedo_event_model` and `dc.milestone=M2`). A tool later lists only the events of that plan, or only the agent's.

**Why this priority**: the dev-cycle integration contract (ADR-0013) and the parallel agents of v0.5.0 depend on these fields, and the stored format must carry them from the first version.

**Independent Test**: record events under different recording contexts (a person; an agent with labels; labels given in the recording context's text form), read them back, and filter them by author kind, agent identifier and labels.

**Acceptance Scenarios**:

1. **Given** a recording context with no agent, **When** an event is recorded, **Then** its author kind is "person" with the fingerprint of the Git identity.
2. **Given** a recording context with an agent identifier and labels, **When** an event is recorded, **Then** its author kind is "agent" with that identifier, and it carries exactly those labels.
3. **Given** labels in the text form used by tools (`key=value` pairs, the form of `GEDO_LABELS`), **When** they are parsed, **Then** they give the same labels as the structured form, and malformed text is refused with a message that names the problem.
4. **Given** a label whose key starts with the reserved prefix `gedo.`, or more than 16 labels, a key over 64 bytes or a value over 256 bytes, **When** a tool tries to record it, **Then** it is refused before anything is written.
5. **Given** stored events, **When** they are filtered by one or more `key=value` labels, by author kind or by agent identifier, **Then** exactly the matching events are returned, in their stored order.
6. **Given** labels unknown to Gedo, **When** they are stored and read back, **Then** they come back unchanged; Gedo never interprets them.

---

### User Story 3 - Live events are kept until they are published as a tape (Priority: P1)

While a developer works, each event is appended to the local write-ahead log as it happens. When the work is published as a commit, the events in the log are packed into a tape (in recording order), the events and the tape are stored in the shared Gedo data, the commit index records that the commit has that tape, and the published events leave the log. A crash at any point loses no event and never stores half a tape.

**Why this priority**: this is the path every live change takes; losing an event or storing a partial tape would corrupt the history that later layers read.

**Independent Test**: record events, publish them for a commit, and check the tape, the stored events, the index and the emptied log; repeat with simulated crashes before, during and after each step, and with a second publish of the same events.

**Acceptance Scenarios**:

1. **Given** events recorded in the log, **When** they are published for a commit, **Then** one tape lists their ids in recording order, every event is stored by its id, the index maps the commit to the tape, and the log no longer holds them.
2. **Given** a crash after some of the shared data was written but before the log was truncated, **When** publishing runs again, **Then** the result is the same as an uninterrupted publish and nothing is duplicated.
3. **Given** a crash while an event is being appended, **When** the log is opened again, **Then** the torn record is dropped and every earlier event is intact.
4. **Given** events recorded after the publish started, **When** the publish finishes, **Then** those later events stay in the log for the next publish.
5. **Given** a log with no events, **When** a publish runs, **Then** no tape is created and nothing is written.
6. **Given** two publishes on two machines that write to the same Gedo data and then exchange it, **When** the data is joined, **Then** both tapes, all events and both index entries are present and nothing conflicts.

---

### User Story 4 - Commits made without Gedo get the same events everywhere (Priority: P1)

A teammate commits without Gedo, or the team adopts Gedo on an existing repository. For a commit that has no tape in the index, Gedo reconstructs its events from the commit's semantic deltas and identity merges: the author is the commit's author (a person), there are no labels, and the clock is derived from the commit's position in the history. Every machine reconstructs exactly the same events with the same ids.

**Why this priority**: most commits Gedo sees were made without it; without deterministic reconstruction the same change would get different ids on each machine and later grafts and merges would apply it twice.

**Independent Test**: reconstruct the events of real and scripted histories on Windows and Linux, in different indexing orders, and compare the events and ids byte for byte with a committed snapshot.

**Acceptance Scenarios**:

1. **Given** a commit without a tape, **When** its events are requested, **Then** they are reconstructed from its semantic deltas against the first parent and its identity merges, in the order of the deltas, each marked as reconstructed.
2. **Given** the same history, **When** it is reconstructed on two machines or in different orders, **Then** the events and their ids are identical.
3. **Given** a commit that has a tape from a live publish, **When** its events are requested, **Then** the tape's events are returned and nothing is reconstructed.
4. **Given** a commit with no Rust change, **When** its events are requested, **Then** the result is empty.

---

### User Story 5 - Logical clocks order events consistently with the history (Priority: P2)

Every event has a logical clock. An event of a commit is always later than every event of the commit's ancestors, whether the events were recorded live or reconstructed, and events of one publish keep their recording order. Two events that neither happened before the other are ordered by a fixed rule, so every machine sorts any set of events the same way.

**Why this priority**: the CRDT and the sequencer (FEAT-066, FEAT-069) depend on an order that respects causality and is the same everywhere; it must be fixed in the stored format now.

**Independent Test**: build histories with branches and merges, record live events on top of some commits and reconstruct others, and check with property-based tests that clocks never go backwards along the history and that sorting is total and the same in any input order.

**Acceptance Scenarios**:

1. **Given** a commit and any of its ancestors, **When** their events are compared, **Then** every event of the ancestor is earlier.
2. **Given** events recorded live on top of a commit, **When** they are compared with the events of that commit and its ancestors, **Then** they are later; among themselves, they keep their recording order.
3. **Given** events from two branches, **When** they are sorted, **Then** the order is total, deterministic and independent of the order in which they were given.

---

### User Story 6 - Stored events and tapes are read safely and quickly (Priority: P2)

Events and tapes arrive from other machines through Git remotes. Reading them enforces size limits and format versions and never crashes on malformed or hostile data. Recording an event and publishing a tape are fast enough for the interactive budgets of the technical specification.

**Why this priority**: everything under `refs/gedo/*` is untrusted input (technical specification), and the WAL append and publish budgets are part of the MVP's promise.

**Independent Test**: fuzz the decoders of events and tapes, feed them truncated, oversized and wrong-version data, and run the benchmarks for recording and publishing.

**Acceptance Scenarios**:

1. **Given** a malformed, truncated or oversized event or tape, **When** it is read, **Then** a clear error names the record and nothing panics.
2. **Given** an event or tape with a newer format version than this Gedo reads, **When** it is read, **Then** it is refused with the found and the supported versions.
3. **Given** a stored event whose content does not match its id, **When** it is read, **Then** it is reported as corrupted.

---

### Edge Cases

- An entity both renamed and with its body edited in one change: two events for the same identity, in the order of the deltas.
- A merge commit: its reconstructed events come from the deltas against the first parent and the identity merges found at it; its clock is later than both parents'.
- Root commits and histories with several roots.
- The same live event published twice (a retried publish, or the same tape written on two machines): stored once, because ids are content-derived.
- Two identical changes made independently by two people: different ids, because the authors differ; the same person making the same change on two machines on top of the same commit: the same id (idempotent).
- A publish with thousands of events (a large refactor): one tape, within the size limits; above the limits, the publish is refused with a clear error and the log is kept.
- An agent identifier or a label with characters outside the allowed set, an empty key, or an empty value: refused before anything is written.
- An event read from a remote whose labels use the reserved `gedo.` prefix: accepted when read (Gedo may set such labels itself), refused only when a tool tries to record one.
- A commit index entry pointing to a tape that is missing locally (the remote data was only partly fetched): reported as missing, never silently treated as "no events".
- A WAL holding events recorded on top of a different commit than the one being published (the developer switched branches): the events keep their clocks; publishing does not rewrite them.
- A commit in a repository without a configured Git identity: reconstruction still works (the commit's own author is used); a live record or publish refuses with the existing missing-identity error.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The core MUST define an event with: its id, a logical clock, the target entity's identity, the operation, the entity's structural hash after the operation (absent for a tombstone and an identity merge), the author's fingerprint, the author kind (person, or agent with an identifier), the labels, and whether it was recorded live or reconstructed.
- **FR-002**: The operation MUST cover every semantic delta of feature 003 (created, moved, renamed, signature mutated, body modified, tombstoned, with their details) and the identity merge (kept and merged identities); new operations MUST be addable later without changing the meaning of stored ones.
- **FR-003**: An event's id MUST be derived only from the event's content, under a domain-separated, versioned derivation (ADR-0007), so that equal content gives equal ids on every machine and any difference in content gives a different id.
- **FR-004**: Clocks MUST respect the history: an event of a commit is later than every event of its ancestors, live events recorded on top of a commit are later than that commit's events, and events of one publish keep their recording order. Every set of events MUST have one total order, the same on every machine and for any input order, with ties broken by a fixed rule.
- **FR-005**: The author fingerprint MUST be a one-way hash of the person's Git identity; the identity itself MUST NOT be stored in events. An agent's events MUST carry both the fingerprint of the Git identity the agent runs on and the agent's identifier.
- **FR-006**: Labels MUST be a map of at most 16 entries, keys of 1 to 64 bytes and values of 1 to 256 bytes of UTF-8 (ADR-0013), with a documented character set for keys and agent identifiers; keys with the prefix `gedo.` MUST be refused when a tool records them. Gedo MUST store and return labels unchanged and never interpret them.
- **FR-007**: A recording context MUST state the author kind, the agent identifier and the labels of the events recorded under it; it MUST be buildable from the text forms that tools use (`GEDO_AGENT` and `GEDO_LABELS`, as `key=value` pairs), and invalid text MUST be refused with a message naming the problem. Reading the environment itself belongs to the CLI (v0.4.0).
- **FR-008**: Recording a live event MUST append it to the local write-ahead log; nothing is shared until it is published.
- **FR-009**: Publishing for a commit MUST store every event of the log by its id, store one tape listing them in recording order, add a commit index entry from the commit to the tape, and only then remove the published events from the log. Publishing MUST be idempotent and crash-safe: re-running it after a crash at any point gives the same result as an uninterrupted run.
- **FR-010**: Tapes MUST NOT depend on commit ids; the commit index MUST be the only link between commits and tapes, and a commit MAY have several tapes.
- **FR-011**: Events, tapes and index entries MUST be stored as content-addressed shared Gedo data (ADR-0008, ADR-0011), so that data written on several machines joins by union without conflict and travels through plain push and fetch.
- **FR-012**: For a commit without a tape in the index, the events MUST be reconstructed deterministically from the commit's semantic deltas against its first parent and its identity merges, with the commit author as a person, no labels, and the reconstructed marker; equal histories MUST give identical reconstructed events on every machine and in every indexing order.
- **FR-013**: Reading MUST answer: the tapes of a commit (from the index, or the reconstructed events), the events of a tape in order, one event by id, and the events matching a filter on labels, author kind and agent identifier.
- **FR-014**: Every stored record (event, tape, index entry) MUST be versioned and decoded safely: size limits, format versions, a check that an event's content matches its id, no panic on malformed input; the decoders MUST be fuzzed.
- **FR-015**: Errors (a missing tape, a corrupted record, an unsupported format, a full or busy log, a missing identity) MUST carry context and MUST NOT be swallowed.
- **FR-016**: The event model, the clock rules, the id derivation and the label rules MUST be pure core logic without I/O (ADR-0001), covered by property-based tests; storage goes through the existing ports.

### Key Entities

- **Event**: one immutable change of one entity (or one identity merge), with its id, clock, target, operation, resulting structural hash, author fingerprint, author kind, labels and origin (live or reconstructed).
- **Semantic operation**: what the event did: a semantic delta of feature 003 or an identity merge.
- **Logical clock**: the event's position in logical time, consistent with the commit history and with the recording order.
- **Author**: the fingerprint of the person's Git identity, and the author kind: a person, or an agent with its identifier.
- **Labels**: an opaque map of keys to values set by tools built on Gedo; the `gedo.` prefix is Gedo's.
- **Recording context**: the author kind, agent and labels that apply to the events recorded while it is in effect.
- **Tape**: the ordered event ids of one publish.
- **Commit index entry**: the link from one commit to one tape.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The same histories, reconstructed on Windows and Linux and in at least 3 different orders, give identical events and ids, byte for byte, on every pull request.
- **SC-002**: In 100% of the property-based runs, event ids are equal for equal content and different for different content, clocks never go backwards along the history, and sorting is total and independent of input order.
- **SC-003**: In 100% of the property-based crash runs (a crash before, during or after each step of a publish), re-running the publish gives the same stored data as an uninterrupted run, with no event lost or duplicated.
- **SC-004**: Recording one event takes under 1 ms at the 95th percentile, and publishing a tape of 1,000 events takes under 1 second, on the reference machine (technical specification).
- **SC-005**: Reconstructing the events of one fetched commit takes under 100 ms at the 95th percentile once the commit's identities are indexed, in line with the re-attachment budget of the technical specification.
- **SC-006**: Filtering 100,000 stored events by one label returns in under 500 ms on the reference machine.
- **SC-007**: Property-based tests and at least 10 minutes of coverage-guided fuzzing of the event and tape decoders per release candidate find no crash, hang or wrong result.

## Assumptions

- No end-user command is delivered; `gedo publish`, `gedo stream`, `--label` filters on the command line and the JSON schemas arrive in v0.4.0 (FEAT-088) and later. The capabilities are exercised through tests and benchmarks.
- Live events are produced by the ingest of saved files, which arrives with the daemon in v0.4.0; this feature records events given to it and is tested with the deltas of feature 003.
- Re-attachment of tapes to rewritten commits (rebase, amend, squash) and the 90-day retention are FEAT-065; virtual streams, their labels and the inheritance of author and labels from a stream are FEAT-068. This feature fixes the event fields they use.
- Labels and author kind follow ADR-0013 (limits, reserved prefix, opaque values). Agent identifiers use the same character set as label keys.
- Events of a tape are kept as recorded; consolidating or splitting them (lanes) is FEAT-067.
- Formats are versioned and every later release reads this first version (technical specification, Formats); the layout of the shared data (which collections hold events, tapes and the index) is decided in the plan and recorded in an ADR if it changes the technical specification.
- The reference machine of the technical specification is used for SC-004 to SC-006; CI benchmarks are reported but do not block pull requests, while determinism checks do.
