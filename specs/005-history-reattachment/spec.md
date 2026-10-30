# Feature Specification: History Re-attachment

**Feature Branch**: `feat/005-history-reattachment`

**Created**: 2026-10-30

**Status**: Draft

**Input**: User description: "Re-attachment after rebase, amend and squash (release v0.3.0, second feature, backlog items FEAT-065 and IMP-005): rebuild the commit index after local and server-side rewrites by semantic diff and event matching, with a fast path for local rebases (ADR-0008); events no longer reachable from any indexed commit are kept for 90 days before `gedo gc` (FEAT-087) may remove them; when a fetch or merge brings new Gedo data, fill the local event cache so the first filter after a fresh clone is not slow (IMP-005)."

## Context

Feature 004 stores live events in **tapes**, one per publish, and links commits to tapes through the **commit index** (`commit-tapes`). Tapes and events do not name commits, so they survive any rewrite; only the index is keyed by commit id (ADR-0008, ADR-0014). Commit ids change all the time: a local `git rebase` or `git commit --amend`, and above all the server-side squash and rebase merges of GitHub and GitLab, which Gedo never sees happen. After such a rewrite the index points to commits that are no longer in the history, the new commits have no tape, and `events_of_commit` falls back to reconstruction: the events lose their original ids, authors, agent identifiers and labels, and the reasoning of the branch is gone. Teams squash by default, so this is the normal case.

This feature **re-attaches** Gedo data to rewritten commits. For a commit without an index entry, Gedo derives the commit's semantic changes, matches them against the stored events of tapes that are no longer attached to the history, and adds index entries for the tapes it finds, so the commit's events are the original live events again. Nothing stored is changed: re-attachment only adds index entries, which join by union like every other Gedo data (ADR-0014). Local rewrites, where Git tells which commit replaced which, take a fast path.

Events whose tapes are attached to no commit of the history are **detached**. They are kept for a grace period of 90 days, so a squash that Gedo has not re-attached yet (or a branch deleted by mistake) never loses history; this feature reports them with their age, and `gedo gc` (FEAT-087, v0.4.0) removes them after the period.

Finally, **filling the event cache** (IMP-005): when new Gedo data arrives (a fetch or a merge of `refs/gedo/*`), the events it brings are read once and kept in the local event cache, so the first filter after a fresh clone does not read every event object (1.5 to 1.7 s at p95 on the Windows reference machine for 10,000 events today).

Like the earlier features, this one has no end-user command: the `post-rewrite` and fetch hooks that call it arrive with the hooks in v0.4.0 (FEAT-090, FEAT-091) and the daemon's background fetch (FEAT-073). Its users are those later layers, the hosting integration and the Levix developers who build them; the capabilities are exercised through tests and benchmarks.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A local rebase or amend keeps the live events of every commit (Priority: P1)

A developer publishes work with Gedo on a branch, then rebases the branch on the updated main (or amends the last commit). Git reports which old commit became which new commit. For each pair, Gedo attaches the old commit's tapes to the new commit when the new commit makes the same semantic changes; afterwards, the events of every rewritten commit are the original live events, with their ids, authors and labels.

**Why this priority**: rebasing a branch before merging is the most frequent rewrite, and it is the one Gedo can handle exactly, because Git names the pairs.

**Independent Test**: publish events for commits of a branch, rebase the branch (with and without conflicts resolved by hand, and with an amend that changes the message only), give the old and new pairs to re-attachment, and check the events of each new commit.

**Acceptance Scenarios**:

1. **Given** a published commit rewritten by a rebase that changes no code of its own (only the base), **When** the pair is re-attached, **Then** the new commit has the old commit's tapes and its events are the published events, unchanged.
2. **Given** an amend that changes only the commit message, **When** the pair is re-attached, **Then** the result is the same as in scenario 1.
3. **Given** a pair where the new commit makes changes the old one did not (a conflict resolved by hand, an amend that edits code), **When** it is re-attached, **Then** the matching changes keep their live events and the extra changes appear as reconstructed events of the new commit.
4. **Given** the same pairs re-attached twice, or on two machines, **When** the data is joined, **Then** the index holds each entry once and nothing conflicts.

---

### User Story 2 - A squash merged on the server keeps the branch's history (Priority: P1)

A developer's branch has several commits published with Gedo. The pull request is squash-merged on GitHub, so main has one new commit made without Gedo and the branch's commits are gone. After the next fetch, Gedo finds the stored events that the squash commit accounts for, attaches their tapes to it, and the squash commit's events are the branch's live events: `gedo trace` and `gedo playback` (v0.4.0) keep the reasoning of the branch.

**Why this priority**: squash merges are the default in most teams and the reason ADR-0008 exists; without this, every pull request erases the history Gedo recorded.

**Independent Test**: publish events for the commits of a branch, build a squash commit of the branch on top of main (as the server does), run re-attachment for the new commit with no pairs, and check that the tapes are attached and that unrelated detached tapes are not.

**Acceptance Scenarios**:

1. **Given** a squash commit whose changes are exactly the net effect of the published commits of a branch, **When** it is re-attached, **Then** every tape of those commits is attached to it, and its events are their live events in clock order.
2. **Given** a branch where an entity was changed several times (created, then edited twice), **When** its squash is re-attached, **Then** the events of the intermediate changes are attached with the final one, so the history of the entity is complete.
3. **Given** a squash commit and detached tapes of an unrelated branch, **When** it is re-attached, **Then** the unrelated tapes stay detached.
4. **Given** a squash commit made from commits never published with Gedo, **When** it is re-attached, **Then** nothing is attached and its events are reconstructed as before.
5. **Given** a rebase merge on the server (every commit of the branch replayed on main with new ids), **When** the new commits are re-attached without pairs, **Then** each new commit gets the tapes of the commit it replays.

---

### User Story 3 - Re-attachment is deterministic and adds data only (Priority: P1)

Every machine that fetches the same history and the same Gedo data reaches the same index, whichever machine re-attached first and in whatever order the commits are processed. Re-attachment never changes or removes a stored event, tape or index entry; a commit that already has an index entry is left as it is.

**Why this priority**: the index is shared data that joins by union; a non-deterministic result would make two machines disagree on the history of the same commit.

**Independent Test**: run re-attachment on the same scripted histories in different orders and on Windows and Linux, compare the resulting index entries byte for byte, and check with property-based tests that the stored events and tapes are untouched.

**Acceptance Scenarios**:

1. **Given** the same history and Gedo data, **When** re-attachment runs on two machines or in different orders, **Then** the index entries it adds are identical.
2. **Given** a commit that already has an index entry, **When** re-attachment runs for it, **Then** nothing is added for it.
3. **Given** any run of re-attachment, **When** it finishes, **Then** every event and tape stored before it is unchanged, and the only new data are index entries.
4. **Given** a crash during re-attachment, **When** it runs again, **Then** the result equals an uninterrupted run.

---

### User Story 4 - Detached events are kept and reported for 90 days (Priority: P2)

After a rewrite that has not been re-attached (a branch deleted, a squash not fetched yet, history rewritten away), some tapes are attached to no commit of the history. Gedo lists them with the date since which they are detached; a tape is only eligible for removal once it has been detached for the grace period (90 days by default), and it stops being detached the moment a re-attachment links it again.

**Why this priority**: the grace period is what makes a squash safe before it is re-attached; `gedo gc` (FEAT-087) needs this report to remove only what is safe to remove.

**Independent Test**: publish tapes, rewrite their commits away, advance the clock of the test, and check the report before and after 90 days and after a later re-attachment.

**Acceptance Scenarios**:

1. **Given** tapes whose commits are no longer reachable from any ref of the repository, **When** detached data is reported, **Then** each such tape is listed with its events and the date since which it is detached.
2. **Given** a tape detached for less than the grace period, **When** the report is asked which tapes may be removed, **Then** it is not listed; after the period, it is.
3. **Given** a detached tape later attached by re-attachment, **When** the report runs, **Then** it is no longer listed.
4. **Given** a grace period set to another number of days, **When** the report runs, **Then** that period is used.

---

### User Story 5 - The first filter after a fetch is fast (Priority: P2)

A developer clones a repository that already has 10,000 Gedo events, or fetches a teammate's new events. The events brought in are read once and kept in the local event cache as part of handling the new data, so the first filter afterwards takes the same time as any later one.

**Why this priority**: IMP-005 measured a first filter of 1.5 to 1.7 s at p95 on the Windows reference machine after a fresh clone, against 35 to 69 ms once the cache is warm.

**Independent Test**: build a repository with 10,000 published events, fetch it into an empty clone with an empty cache, run the cache filling for the new data, and measure the first filter.

**Acceptance Scenarios**:

1. **Given** new Gedo data brought by a fetch or a merge of `refs/gedo/*`, **When** the new data is handled, **Then** every new event is in the local event cache.
2. **Given** an event already in the cache, **When** new data is handled, **Then** it is not read again.
3. **Given** a cache that cannot be written (busy, full, refused), **When** new data is handled, **Then** the failure is logged and nothing else fails; filters still return the right events.

---

### Edge Cases

- A rewritten commit whose new version has no Rust change (all changes dropped in a rebase): nothing is attached; its old tapes stay detached.
- A rebase that splits one published commit into two, or squashes two published commits into one locally (`git rebase -i` with `squash`, `fixup`, `split`): pairs are many to one or one to many; the tapes go to the new commits whose changes match their events.
- A cherry-pick of a published commit onto another branch: the original commit is still in the history, so the tape is attached to both commits (the CRDT of FEAT-066 applies the shared events once).
- An entity created on the branch and edited later, squashed: the squash commit creates the entity directly with its final content; it is matched to the branch's events through its final structural hash and qualified path (see FR-006).
- A revert on the branch (a change and its inverse): the net effect of the squash has no change for that entity; the events are attached with the rest of their tapes when the tapes match otherwise.
- Two branches with identical changes (the same fix made twice): both sets of tapes match; both are attached, in the deterministic order of FR-008.
- A tape listed in the index whose events are missing locally (partial fetch): reported as missing, never treated as unmatched.
- A history of thousands of commits fetched at once (a first fetch after a long time): re-attachment is incremental and bounded per commit, within the budget of SC-004.
- A commit made by a server-side merge commit (not squash): its parents are the branch tip and main; the branch's commits are still reachable, so nothing is detached.
- The grace period changed after tapes were detached: the report uses the current period.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Re-attachment MUST accept a list of rewritten pairs (old commit, new commit), as Git's `post-rewrite` hook reports them, and MUST also run for new commits given without pairs (commits that arrived by fetch).
- **FR-002**: For a pair, Gedo MUST first compare the semantic signature of the old and new commits (their changes without their base); when they are equal, it MUST attach the old commit's tapes to the new commit without matching events one by one (the fast path of ADR-0008).
- **FR-003**: Otherwise, Gedo MUST derive the new commit's semantic changes against its first parent and match them against the events of candidate tapes: a change matches an event with the same kind of operation and target entity whose structural hash after the change is equal (ADR-0008, ADR-0014).
- **FR-004**: Candidate tapes MUST be the tapes of the old commits of the given pairs; for commits given without pairs, the detached tapes of the repository.
- **FR-005**: A candidate tape MUST be attached to the new commit when [NEEDS CLARIFICATION: is a tape attached when every one of its events is matched or superseded by a later matched event of the same entity in the candidates, or as soon as one of its events is matched?]; the events of the new commit MUST then be the events of its attached tapes, followed by the reconstructed events of its changes that no attached event accounts for.
- **FR-006**: An entity created on the rewritten branch has a different identity in a squash commit (its id derives from its content when it first appears, ADR-0007). Re-attachment MUST match such an entity by its kind, its qualified path and its final structural hash, and MUST [NEEDS CLARIFICATION: how is the identity of the squash commit's entity reconciled with the identity in the live events?].
- **FR-007**: Re-attachment MUST only add commit index entries; it MUST NOT change or remove any stored event, tape or index entry, and MUST NOT add entries for a commit that already has one.
- **FR-008**: The result MUST be deterministic: the same history and Gedo data MUST give the same index entries on every machine and for every processing order; ties between several matching tapes MUST be broken by a fixed rule.
- **FR-009**: Re-attachment MUST be idempotent and crash-safe, like publishing (feature 004).
- **FR-010**: A tape MUST be reported as detached when none of the commits it is indexed for is reachable from a ref of the repository outside `refs/gedo/*`; the report MUST list each detached tape, its events and the date since which it is detached, [NEEDS CLARIFICATION: where does the detachment date come from, so that every machine and `gedo gc` agree on it?].
- **FR-011**: The report MUST say which detached tapes are past the grace period (90 days by default, configurable) and are therefore eligible for removal by `gedo gc` (FEAT-087); removing data is not part of this feature.
- **FR-012**: When new Gedo data arrives (an update of the `events` collection from a fetch or a merge), Gedo MUST read every event that the new data adds and that the local event cache does not hold, and keep it in the cache; a cache failure MUST be logged and MUST NOT fail the operation (ADR-0015).
- **FR-013**: The entry points (re-attach pairs, re-attach new commits, report detached data, handle new event data) MUST be use cases of the core usable by the hooks (FEAT-090, FEAT-091), the daemon (FEAT-073) and the hosting integration without change.
- **FR-014**: Errors (a missing tape or event, a commit that cannot be read, identities that cannot be indexed) MUST carry context and MUST NOT be swallowed; a missing tape MUST NOT be treated as unmatched.
- **FR-015**: The matching rules MUST be pure core logic without I/O (ADR-0001), covered by property-based tests; storage goes through the existing ports.
- **FR-016**: Re-attachment MUST log, at info level, each commit it attached tapes to, the number of tapes and events, and whether the fast path was used; at debug level, the unmatched changes.

### Key Entities

- **Rewritten pair**: an old commit and the new commit that replaced it, as Git reports them after a rebase or an amend.
- **Semantic signature**: the semantic changes of a commit without its base (operations, targets, structural hashes after the change), used to compare an old and a new commit.
- **Candidate tape**: a tape that may belong to a commit being re-attached: a tape of the old commit of a pair, or a detached tape.
- **Match**: a change of the new commit and the event that accounts for it.
- **Detached tape**: a tape indexed for no commit reachable from the repository's refs, with the date since which it is detached.
- **Grace period**: how long a detached tape is kept before it may be removed (90 days by default).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the scripted rewrite scenarios (rebase, amend, interactive squash and fixup, split, server squash, server rebase merge, cherry-pick), 100% of the published events whose changes survive the rewrite are attached to the new commits, and no tape of an unrelated branch is attached.
- **SC-002**: The same histories and data, re-attached on Windows and Linux and in at least 3 different orders, give identical index entries, byte for byte, on every pull request.
- **SC-003**: In 100% of the property-based runs, re-attachment adds only index entries and leaves every stored event and tape unchanged, and re-running it after a crash at any point gives the same data as an uninterrupted run.
- **SC-004**: Re-attachment of one fetched commit takes under 100 ms at the 95th percentile once its identities are indexed, with 10,000 stored events of which 1,000 are detached, on the reference machine (technical specification).
- **SC-005**: After a fetch of 10,000 new events into a clone with an empty cache, the first filter by one label returns in under 500 ms on the reference machine, and filling the cache takes under 2 s.
- **SC-006**: Detached tapes are reported with the right eligibility before and after the grace period in 100% of the scripted retention scenarios.

## Assumptions

- No end-user command is delivered. The `post-rewrite` hook, the fetch path of the hooks (FEAT-090, FEAT-091), the daemon's background fetch (FEAT-073) and the hosting integration call these use cases later; this feature is exercised through tests and benchmarks.
- Removing detached data is `gedo gc` (FEAT-087, v0.4.0); this feature only reports what may be removed.
- Commits are re-attached only after their identities are indexed (feature 003); indexing is not part of the re-attachment budget.
- The CRDT (FEAT-066) decides how events present in several commits (a cherry-pick, two identical branches) are applied once; this feature only links tapes to commits.
- Only the `events`, `tapes` and `commit-tapes` collections of ADR-0014 are read; no new shared collection is added unless the plan shows it is needed, and then it is recorded in an ADR.
- The grace period default of 90 days comes from the technical specification (Retention); where it is configured is decided with the configuration of the CLI (v0.4.0) and is a parameter of the use case here.
- The reference machine of the technical specification is used for SC-004 and SC-005; CI benchmarks are reported but do not block pull requests, while determinism checks do.
