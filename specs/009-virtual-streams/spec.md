# Feature Specification: Virtual Streams

**Feature Branch**: `feat/009-virtual-streams`

**Created**: 2026-11-06

**Status**: Draft

**Input**: User description: "Virtual streams (release v0.3.0, sixth feature, backlog item FEAT-068): switch or isolate work in memory without git stash or checkout. A stream carries the same opaque labels as events, set at creation, and an optional agent author, so a tool can open one stream per plan milestone or per agent and find it again by label. Every event recorded inside a stream inherits its author and labels; outside a stream the author is the person of the Git identity, or the agent and labels named by GEDO_AGENT and GEDO_LABELS. The prefix gedo. is reserved for Gedo's own keys (ADR-0013)."

## Context

Features 004 to 008 record each change of a code entity as an **event** with an author kind and opaque labels, publish events as tapes indexed by commit, keep them attached through rewrites, converge concurrent work per entity and split mixed work into lanes. Today all live work of a repository goes into one write-ahead log, under one recording context. A developer who wants to put a change aside and start another one uses `git stash` or a second branch and `git checkout`, and loses the events of the work put aside or mixes them with the next one. A tool that runs several coding agents, or the dev-cycle protocol that runs one milestone after another, has no way to tell Gedo "this work belongs to milestone 2 of plan X, done by agent `planner`" once, and find that work again later.

A **virtual stream** is a named unit of work kept by Gedo on the developer's machine: its base commit, its author (the person of the Git identity, or an agent), its labels, its own pending events and its saved work (the files of the stream, as a tree in the object store). Events recorded inside a stream inherit its author and labels. Switching streams saves the current work into the stream being left and gives back the work of the stream entered, so work is put aside and resumed without stash, checkout or a branch per change. Publishing a stream writes its commit on its base, with the stream's events as the commit's tape.

Like the earlier features of v0.3.0, this one has no end-user command yet. `gedo stream <name>`, `gedo stream list --label key=value --json` and writing a stream's work into the working tree arrive with the CLI in v0.4.0 (FEAT-088 and the CLI items); hidden checkouts for agent streams are FEAT-130 (v0.5.0). The users of this feature are those commands and the Levix developers who build them; it is exercised through tests and benchmarks.

## Clarifications

### Session 2026-11-06

- Q: What does a stream isolate: only its context and events, or also the work (the files)? → A: Its context (author kind, labels), its own pending events and its work, kept by Gedo as a tree; switching saves the current work and gives back the other stream's, without stash or checkout. Writing the files of the working tree is the CLI's (v0.4.0).
- Q: Where are streams kept? → A: Only on the developer's machine, in the repository's Gedo directory; they are never pushed. What is shared is the published result (the tapes with their labels).
- Q: What does publishing a stream do? → A: Gedo writes the commit (the stream's work on its base) with the stream's events as its tape and moves no branch; the stream stays open on the new commit until it is discarded.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A tool opens a labeled stream and finds it again (Priority: P1)

The dev-cycle protocol starts milestone 2 of plan `gedo_virtual_streams` with the agent `implementer`. It creates the stream `dc/m2` on the current commit with the agent `implementer` and the labels `dc.plan=gedo_virtual_streams` and `dc.milestone=2`. Later, in another process, it lists the streams with the label `dc.plan=gedo_virtual_streams` and gets `dc/m2` back with its base, agent and labels exactly as given.

**Why this priority**: it is the contract promised to the dev-cycle protocol (ADR-0013): one stream per milestone or per agent, found again by label.

**Independent Test**: create streams with different agents and labels in a scripted repository, reopen the store, list them with label and agent filters and compare.

**Acceptance Scenarios**:

1. **Given** a repository, **When** a stream is created with a name, a base, an agent and labels, **Then** listing the streams in a new process returns it with the same name, base, agent and labels.
2. **Given** several streams, **When** they are listed with one or more `key=value` labels, **Then** only the streams carrying every one of those labels are returned, in name order; an agent filter keeps only the streams of that agent.
3. **Given** a label whose key starts with `gedo.`, more than 16 labels, an invalid key, value or agent, or a name already used, **When** a stream is created, **Then** it is refused with the reason and nothing is stored.

---

### User Story 2 - Events recorded inside a stream inherit its author and labels (Priority: P1)

Inside the stream `dc/m2`, Gedo records the events of the agent's changes. Every event carries the author kind `agent implementer` with the fingerprint of the Git identity it runs on, and the labels of the stream, without the agent passing them again. Outside any stream, events keep today's behavior: the person of the Git identity, or the agent and labels named by `GEDO_AGENT` and `GEDO_LABELS`.

**Why this priority**: inheritance is what makes the labels useful: the published events of a milestone can be filtered by its labels (FEAT-063 filters) without every caller repeating them.

**Independent Test**: record operations inside a stream and outside, read the events back and compare their authors, labels and clocks.

**Acceptance Scenarios**:

1. **Given** a stream with an agent and labels, **When** events are recorded inside it, **Then** each event's author kind is that agent, its fingerprint is the Git identity's, its labels are the stream's, and its clock is a live clock on top of the stream's base.
2. **Given** no active stream, **When** the recording context is asked with the texts of `GEDO_AGENT` and `GEDO_LABELS`, **Then** it is the same context as today (person, or that agent and labels).
3. **Given** an active stream and `GEDO_AGENT` or `GEDO_LABELS` set, **When** the recording context is asked, **Then** it is the stream's, and the environment texts are ignored.
4. **Given** two streams, **When** events are recorded in each, **Then** each stream's pending events are only its own, and the repository's own log is unchanged.

---

### User Story 3 - Work is switched without stash or checkout (Priority: P1)

A developer is changing `billing.rs` in the stream `retry-policy` when an urgent fix comes in. Switching to a new stream `hotfix` saves the current work (the tree of the working files, given by the caller) into `retry-policy` and gives back the work of `hotfix`, which for a new stream is its base. After the fix, switching back to `retry-policy` gives back exactly the saved work. The work outside any stream is saved and given back the same way. No branch, `HEAD`, index or working file is touched by Gedo; writing the returned tree to the working tree is the CLI's (v0.4.0).

**Why this priority**: this is the "switch or isolate work in memory" of the backlog item.

**Independent Test**: switch between streams and the outside work with different trees and check that each switch returns the tree last saved for the target, across processes.

**Acceptance Scenarios**:

1. **Given** the active stream `A` and a current work tree `T`, **When** the caller switches to `B`, **Then** `A`'s saved work becomes `T`, `B` becomes active, and the result is `B`'s saved work, or its base's tree when it has none.
2. **Given** no active stream, **When** the caller switches into a stream, **Then** the current work is saved as the outside work, and switching out of streams later returns it.
3. **Given** a switch to a stream that does not exist, **When** it is asked, **Then** it is refused and nothing changes.
4. **Given** two working trees of one repository (Git worktrees), **When** each switches streams, **Then** each has its own active stream; the streams themselves are shared by both.

---

### User Story 4 - A stream is published as a commit with its events (Priority: P2)

When the work of `dc/m2` is done, the tool publishes the stream with a message. Gedo writes the commit of the stream's work on its base and publishes the stream's pending events as that commit's tape, with the same guarantees as `publish` (ADR-0014). No branch moves; the caller decides where the commit goes. The stream stays open on the new commit, with no pending events, until it is discarded.

**Why this priority**: without it, a stream's events never reach the shared history; it builds on User Stories 1 to 3.

**Independent Test**: publish a stream with recorded events and saved work, read the commit and its tapes back, interrupt the publish at each step and publish again.

**Acceptance Scenarios**:

1. **Given** a stream with saved work and pending events, **When** it is published, **Then** a commit with that tree and the stream's base as its only parent exists, its tape lists the stream's events in recording order, and the stream's base is the new commit with no pending events.
2. **Given** a publish interrupted after the commit is written or after the tape is stored, **When** the stream is used again, **Then** the publish is completed with the same commit and tape, and no event is lost or published twice.
3. **Given** a stream with no saved work and no pending events, **When** it is published, **Then** nothing is written and the outcome says there was nothing to publish.

---

### User Story 5 - Streams converge with each other and are discarded (Priority: P3)

Before publishing, a tool asks Gedo whether two streams on the same base collide: each stream is a side (its base and its pending events) for the tree CRDT convergence of feature 006, which reports the conflicts. A stream that is no longer needed is discarded with its pending events and saved work; a stream that still has pending events or saved work is discarded only when the caller says so explicitly.

**Why this priority**: the radar and agent streams (FEAT-074, FEAT-130) build on it later; here it is the entry point and its tests.

**Independent Test**: converge two streams with compatible and conflicting events; discard streams with and without work.

**Acceptance Scenarios**:

1. **Given** two streams on the same base whose events change different parts of an entity, **When** they are converged, **Then** the result has no conflict; with incompatible changes, the conflicts name the entity and both streams.
2. **Given** a stream with pending events or saved work, **When** it is discarded without the explicit flag, **Then** it is refused with what would be lost; with the flag, the stream, its log, its record and its references are removed, and a working tree where it was active finds no active stream.

---

### Edge Cases

- A stream created on an empty repository (no base commit): it records events with the clocks of an empty history, publishes a root commit, and cannot be converged until it has a base.
- A stream name with `/` (`dc/m2`, `agent/metrics`), upper-case letters, spaces, `..`, an empty segment or more than 64 bytes: names follow one rule (see FR-002), and names that differ only by case cannot exist, so Windows and macOS file systems never mix two streams.
- Names that are reserved on Windows (`con`, `aux`, `nul`) work: the stored form of a name never depends on the file system's rules.
- Two processes create the same stream at the same time: exactly one succeeds; the other gets "already exists".
- Two processes record into the same stream: the stream's log has one writer at a time, as the repository's log (`WalBusy` after the wait).
- The active stream is discarded from another working tree: the next use from the first working tree finds no active stream and reports it, instead of failing.
- A stream's base commit is later rewritten or removed by `git gc` while the stream is open: publishing still writes on the old base (the caller rebases first); a base missing from the object database is an error with context.
- The saved work names a tree whose objects are gone (a repository copied without them): switching to the stream is an error with context; the stream record is kept.
- Very many streams (1,000): listing reads every record once.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A stream MUST have a name, an optional base commit, an author kind (a person, or an agent with an identifier under the rule of FEAT-063), labels (the rules and limits of FEAT-063: at most 16, keys and values as for events), an optional saved work tree and its own pending events. Name, author kind and labels MUST be set at creation and MUST NOT change afterwards.
- **FR-002**: A stream name MUST be 1 to 64 bytes of segments separated by `/`, each segment non-empty, of `[a-z0-9._-]`, starting with a letter or digit, and not `.` or `..`; any other name MUST be refused with the reason.
- **FR-003**: Creating a stream MUST refuse a name already used, labels with the reserved prefix `gedo.` and invalid agents or labels, and MUST store nothing when it refuses.
- **FR-004**: Streams MUST be listed in name order, optionally filtered by labels (every given `key=value` pair present) and by agent, and a stream MUST be readable by name, across processes, with every field as created.
- **FR-005**: Streams MUST be kept on the developer's machine only: in the repository's Gedo directory, shared by its working trees, never under `refs/gedo/*` and never pushed. The active stream MUST be kept per working tree.
- **FR-006**: Events recorded inside a stream MUST carry the stream's author kind, the fingerprint of the Git identity given at recording time, the stream's labels and a live clock on top of the stream's base (ADR-0014), and MUST go to the stream's own log, never to the repository's log or another stream's.
- **FR-007**: The recording context MUST be the active stream's when a stream is active, and otherwise the one named by the texts of `GEDO_AGENT` and `GEDO_LABELS` (today's behavior); inside a stream those texts MUST be ignored.
- **FR-008**: Switching MUST save the caller's current work tree into the stream being left (or as the outside work when no stream is active), set the target (a stream, or none) as the active stream of the working tree and return the target's saved work, or its base's tree when it has none (the empty tree for a stream without a base). Switching to a stream that does not exist MUST be refused without change. Gedo MUST NOT write the working tree, the index, `HEAD` or any branch.
- **FR-009**: Publishing a stream MUST write one commit (tree: the given work tree, or the stream's saved work; parent: the stream's base, none for a stream without a base; message: the caller's), publish the stream's pending events as the tape of that commit with every guarantee of `publish` (content-derived ids, idempotent steps, the log truncated only after the index entry is written), and then set the stream's base to the commit with no saved work. It MUST NOT move any branch or reference outside `refs/gedo/*`.
- **FR-010**: An interrupted publish MUST be completed by the next operation on the stream, with the same commit and tape: the commit to publish MUST be recorded in the stream before its tape is stored.
- **FR-011**: A stream MUST be usable as a side of the convergence of feature 006 (its base and its pending events), so two or more streams on a common base converge with the existing use case.
- **FR-012**: Discarding a stream MUST remove its record, its log and its references; a stream with pending events or saved work MUST be discarded only when the caller asks explicitly, and the refusal MUST say how many events and whether work would be lost. A working tree whose active stream was discarded MUST then find no active stream, with a warning, instead of failing.
- **FR-013**: The stream record MUST be a versioned stored format (a format byte), checked on read; a record that does not decode or breaks an invariant MUST be refused with its stream name, never silently dropped.
- **FR-014**: The stream rules (names, filters, inheritance of the recording context) MUST be pure core logic in `gedo-crdt` or `gedo-core` (ADR-0001); the use cases MUST be in `gedo-app` and reach storage only through ports, with an in-memory adapter for tests and a file adapter in `gedo-store`.
- **FR-015**: Errors (a name refused, a stream missing or already existing, a busy log, a store failure, a missing base or work tree) MUST carry context and MUST NOT be swallowed.
- **FR-016**: The use cases MUST log with `tracing`: stream created, switched, published (with the commit, the tape and the event count), discarded (info); events recorded in a stream (debug); a publish completed after an interruption, a stream record refused (warn).
- **FR-017**: A stream's saved work, its base and the outside work of each working tree MUST stay in the object database while they are in use: they MUST be kept reachable by local references that are never pushed, so `git gc` does not prune them, and those references MUST be removed when the stream is discarded or published.

### Key Entities

- **Stream**: name, base commit, author kind, labels, saved work tree, a publish in progress; its pending events are in its own log.
- **Stream filter**: labels that must all be present and an optional agent.
- **Working tree state**: the active stream of one working tree and its outside work.
- **Publish outcome**: the commit and tape written, or nothing to publish.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On scripted scenarios (create, list by label and agent, record inside and outside streams, switch between streams and the outside work, publish, interrupted publishes at every step, converge, discard), 100% of the outcomes equal the expected ones, across processes and with two working trees.
- **SC-002**: In property-based runs (at least 256 cases per property on every pull request), every event recorded in a stream carries exactly the stream's author kind and labels, every switch returns the tree last saved for its target, and every stream read back equals the stream created.
- **SC-003**: On the reference machine, creating, switching or reading a stream takes under 10 ms at the 95th percentile, listing 1,000 streams under 100 ms, and publishing a stream adds under 10 ms to `publish` for the same events (the budget of `gedo publish` stays 1 s).

## Assumptions

- No end-user command is delivered: `gedo stream <name>`, `gedo stream list --label ... --json` and the `stream` part of the JSON contract arrive with the CLI in v0.4.0 (FEAT-088); the CLI also builds the work tree from the working files and writes the returned tree back.
- Hidden checkouts for agent streams are FEAT-130 (v0.5.0); local intents between streams FEAT-129; the radar's use of streams FEAT-074; moving a stream onto a new base (rebase) and `gedo graft --into <stream>` later items.
- The event format does not change: streams only fill the author kind and labels events already carry (ADR-0013, ADR-0014). Formats freeze at v1.0.0, like every stored format.
- A stream's saved work and base are objects of the repository's object database; Gedo keeps them reachable through local references outside `refs/gedo/` (never pushed) while the stream exists, so `git gc` keeps them; after a discard, `git gc` may prune them.
- The reference machine of the technical specification is used for SC-003; CI benchmarks are reported but do not block pull requests.
