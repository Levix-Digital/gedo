# Feature Specification: Bounded Detached-Tape Walk

**Feature Branch**: `feat/008-bounded-detached-walk`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: "Bound the history walk that finds detached tapes (release v0.3.0, backlog item IMP-006): EventLog::detached and reattach_new walk the history from the tips until every indexed commit is found; when some tape is really detached they walk the whole history on every call. Keep commit generations as local derived data (or use the commit-graph) and stop the walk below the lowest generation of the indexed commits still unseen; measure on a large repository first."

## Context

Feature 005 re-attaches tapes to commits that arrive without rewrite pairs (a fetch, a server-side squash) and reports the tapes that stay detached, for the grace period before `gedo gc`. Both find the detached tapes the same way: they walk the history from the tips until every commit of the commit index is seen. When one tape is really detached (the normal case after a squash merge on the server), its commits are never seen, so the walk goes on to the root commit of the repository, on every call.

Measured on 2026-10-31 (Linux development container, release build, Git store adapter, a linear history without Gedo data under 200 commits with published tapes, one tape detached from a side branch): finding the detached tapes takes p50 58 ms with 10,000 commits and p50 612 ms with 100,000. The cost is linear in the whole history, not in the part Gedo has data for, and it is paid on every fetch that re-attaches and every retention report. A repository that adopts Gedo late in a long history (a kernel, a monorepo) pays for all of its past.

The commit generation (ADR-0014: 1 for a root, one more than the highest parent) bounds the walk: no commit can reach a commit of a higher or equal generation other than itself, so once every commit still to visit has a lower generation than every indexed commit not yet seen, none of those can be reached and the walk can stop. Today generations are kept in memory by the event log only for the life of one call, so computing one costs a walk to the root as well. This feature keeps them as local derived data, so each is computed once per machine.

## Clarifications

### Session 2026-10-31

- Q: Where are the commit generations kept: in the local symbol cache, or read from Git's commit-graph file when it exists? → A: The symbol cache, in a new namespace of local derived data; no port change.
- Q: Is the one-time cost of the first call (one walk of the whole history to compute the generations) acceptable? → A: Yes: it is paid once per machine, within SC-002.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Finding detached tapes costs the part of the history Gedo has data for (Priority: P1)

A team adopts Gedo on a repository with 100,000 commits of history. After a server squash merge, a fetch re-attaches what it can and one tape stays detached. Every later fetch and every retention report finds that detached tape again, walking only from the tips down to the oldest commit with Gedo data that is not yet seen, never into the 100,000 commits below it.

**Why this priority**: this is the improvement: the cost of re-attachment and retention no longer grows with the age of the repository.

**Independent Test**: on a scripted deep history with published tapes and one detached tape, count the commits the walk reads and time it, before and after the generations are known.

**Acceptance Scenarios**:

1. **Given** a deep history with Gedo data only on its recent commits and one detached tape, **When** the detached tapes are listed a second time (generations known), **Then** the walk reads no commit of a generation below the lowest generation of the indexed commits it has not yet seen.
2. **Given** the same history, **When** `reattach_new` is called for a fetched commit, **Then** it finds the same detached candidates as before this change and attaches the same tapes.
3. **Given** a tape whose only commit is gone from the object database (removed by `git gc`), **When** the detached tapes are listed, **Then** the tape is reported detached, as before.

---

### User Story 2 - Generations are computed once per machine (Priority: P1)

The first call after this change computes the generation of every commit it needs, walking the history once, and keeps them in the local cache. Later calls, in other processes, read them from the cache and only compute the generations of new commits, walking from them down to commits whose generation is known. Recording an event on top of a commit and publishing a tape, which also need generations (ADR-0014 clocks), become as cheap.

**Why this priority**: without a kept generation the bound costs a walk to the root itself, so it saves nothing across processes.

**Independent Test**: compute the generations of a scripted history with a cache, open a new event log on the same cache and check that the same generations are returned while reading only the new commits; clear the cache and check the same values are computed again.

**Acceptance Scenarios**:

1. **Given** a history whose generations were computed with a cache, **When** a new event log on the same cache asks for the generation of a new commit on top, **Then** it reads only the new commits and returns the same value as a full computation.
2. **Given** a cache that is missing, cleared or refuses a record, **When** a generation is asked, **Then** it is computed from the history, with the same value; a cache failure is only a warning (ADR-0015).
3. **Given** a cached generation that disagrees with the generations of its commit's parents, **When** it is read, **Then** it is refused, computed again and a warning is logged.

---

### Edge Cases

- No indexed commit at all: nothing to find, no walk.
- Every indexed commit reachable: the walk stops as soon as the last one is seen, as today.
- Tips that are themselves indexed commits, or several tips sharing history: each commit is read once.
- Merge commits: the generation is one more than the highest parent; the bound holds through every parent.
- A tip or a commit on the walk missing from the object database: an error with context, as today; only indexed commits that are missing count as unreachable.
- Shallow clones: a parent missing at the shallow boundary is an error with context, as today (reading the history of a shallow clone is not supported yet).
- Very large histories (1,000,000 commits): the first call keeps every generation it computed in batched cache writes; later calls are bounded.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The search for detached tapes (used by `detached` and `reattach_new`) MUST visit commits from the highest generation down and MUST stop when every commit still to visit has a generation lower than every indexed commit not yet seen, or when every indexed commit is seen.
- **FR-002**: The result of the search MUST be the same as the unbounded walk's for every history: the same tapes detached, with the same commits.
- **FR-003**: An indexed commit missing from the object database MUST count as unreachable and MUST NOT stop the search with an error.
- **FR-004**: With a symbol cache (`EventLog::with_cache`), every commit generation the event log computes MUST be kept in the cache's `generations` namespace, written in batches, and read from it before any history walk; generations of commits read from the cache MUST bound the computation of new ones.
- **FR-005**: A cached generation MUST be used only when it decodes and equals one more than the highest generation of its commit's parents (1 for a root) as known in memory or in the cache; otherwise it MUST be refused, computed again from the history and a warning logged.
- **FR-006**: Cache failures MUST NOT fail a call: they are logged as warnings and the generation is computed from the history (ADR-0015).
- **FR-007**: Generations MUST stay as ADR-0014 defines them, so event clocks, event ids and every stored format stay unchanged.
- **FR-008**: The search MUST log at debug level the commits it read and whether it stopped on the bound, and `detached` and `reattach_new` keep their info events.

### Key Entities

- **Commit generation**: a commit's number in the commit graph (ADR-0014), immutable for the commit, kept per machine in the cache.
- **Bound**: the lowest generation among the indexed commits not yet seen by the walk.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a history of 100,000 commits without Gedo data under 200 commits with tapes and one detached tape, listing the detached tapes once generations are cached takes under 50 ms at the 95th percentile on the reference machine (today: p50 612 ms on the development container).
- **SC-002**: On the same history, the first call with an empty cache takes no more than twice the time of one call before this change.
- **SC-003**: On generated histories (linear, branching and merging, with tapes attached, detached, and on missing commits; at least 256 cases per property on every pull request), the bounded search returns exactly the tapes of the unbounded walk, and cached generations equal the generations computed without a cache.

## Assumptions

- No end-user command is delivered or changed; `gedo fetch`, `gedo gc` and the hooks (v0.4.0 and later) call these use cases.
- The generations namespace is local derived data, like the `identity` and `events` namespaces: never pushed, never rebuilt on open, recomputed when missing.
- Git's commit-graph file is not read: it would need a new port operation and Git does not always write it; it can be added later as a faster source of generations without changing this design.
- The reference machine of the technical specification is used for SC-001 and SC-002; CI benchmarks are reported but do not block pull requests.
