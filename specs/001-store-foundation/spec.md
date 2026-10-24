# Feature Specification: Store Foundation

**Feature Branch**: `feat/001-store-foundation`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "Store foundation (release v0.1.0, backlog items FEAT-001, FEAT-002, FEAT-003, FEAT-004, FEAT-005, FEAT-006, FEAT-007, FEAT-119): the workspace skeleton and the persistence foundation of Gedo: object, reference, write-ahead log and symbol cache ports with in-memory adapters; Git object store access without spawning git or touching the working tree; atomic compare-and-swap references under refs/gedo/*; a crash-safe local write-ahead log; a rebuildable local symbol cache; refspec setup and validation that the data travels through plain push and fetch to GitHub, GitLab and plain remotes; measurement of how many references hosts and fetch tolerate; CI on Windows and Linux for every pull request."

## Context

This is the first release of the bottom-up roadmap (v0.1.0). It has no end-user commands yet: its users are the **higher layers of Gedo** (identity, events, merge, CLI and daemon, built in later releases) and the **Levix developers** who build them. It must prove the riskiest storage assumptions of the product baseline before anything is built on top: that Gedo data can live inside a standard Git repository, travel with standard Git commands and be kept by the hosts, without ever touching source files or `refs/heads/*`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Store and read Gedo data inside a Git repository (Priority: P1)

A higher layer of Gedo stores an opaque piece of data (for example a syntax map or an event tape) in the repository's object store, points a Gedo reference at it, and later reads it back, while the developer's working tree, index and branches stay exactly as they were.

**Why this priority**: every later capability (identity, events, merge, trace) persists through this; nothing else in Gedo can be built without it.

**Independent Test**: in a scratch repository with uncommitted changes, write data and a reference under `refs/gedo/`, read them back, and compare the working tree, index and every `refs/heads/*` value before and after.

**Acceptance Scenarios**:

1. **Given** a repository with uncommitted edits, **When** Gedo stores data and creates a reference under `refs/gedo/`, **Then** the data reads back byte for byte and the working tree, index and `refs/heads/*` are unchanged.
2. **Given** stored data, **When** a standard Git client inspects the repository (`git status`, `git log`, `git fsck`), **Then** it reports a healthy repository and no difference visible to the developer.
3. **Given** no `git` executable on the machine, **When** Gedo stores and reads data, **Then** both succeed (Gedo does not depend on spawning Git for storage).

---

### User Story 2 - Concurrent writers never lose an update (Priority: P1)

The CLI and the background daemon (both in later releases) update the same Gedo reference at the same time. Each update states which value it expects to replace; only one wins, the other is told that the reference moved and can retry.

**Why this priority**: the daemon and the CLI run side by side on every developer machine; a lost update silently corrupts the overlay.

**Independent Test**: many concurrent writers repeatedly update the same reference with compare-and-swap; at the end, the number of successful updates equals the number of distinct values the reference went through, and the reference log shows no gap.

**Acceptance Scenarios**:

1. **Given** a reference at value A, **When** two writers both try to replace A with different values, **Then** exactly one succeeds and the other receives a "reference moved" result carrying the current value.
2. **Given** a writer that expects the reference not to exist, **When** it already exists, **Then** the update is refused without changing it.
3. **Given** a crash in the middle of an update, **When** the repository is opened again, **Then** the reference holds either the old or the new value, never a partial one.

---

### User Story 3 - Local work survives crashes (Priority: P1)

Micro-events produced while a developer edits (from later releases) are appended to a local log inside `.git/gedo/`. If the machine or the process dies, everything recorded up to the last durability point is still there when Gedo starts again, and a half-written last record is detected and dropped instead of corrupting the log.

**Why this priority**: "never lose work" is part of the product promise, and the log is the source of truth for unpublished work.

**Independent Test**: append records, cut the log file at every possible byte offset of the last record (simulating a torn write), reopen, and check that all complete records are returned and the torn one is dropped and reported.

**Acceptance Scenarios**:

1. **Given** a log with complete records, **When** it is reopened, **Then** all records are returned in order with their checksums verified.
2. **Given** a log whose last record was cut mid-write, **When** it is reopened, **Then** all earlier records are returned, the torn record is dropped, and the event is logged for diagnostics.
3. **Given** a record whose content was altered on disk, **When** the log is read, **Then** the corruption is detected and reported with its position instead of returning wrong data.
4. **Given** an acknowledged durability point, **When** the power is lost right after, **Then** every record before that point is present on restart. [NEEDS CLARIFICATION: how often must records become durable — every single append, or in small groups within a bounded window?]

---

### User Story 4 - The local cache can always be rebuilt (Priority: P2)

Gedo keeps a local index (`.git/gedo/symbol_cache.db`) for fast lookups. If it is deleted, corrupted or written by an incompatible version, Gedo detects it and rebuilds it from the data in the repository, with the same lookup results as before.

**Why this priority**: the cache only speeds things up; correctness must never depend on it, but a broken cache must never block the developer.

**Independent Test**: populate data, build the cache, then delete it, corrupt it and change its schema version in three separate runs; each time the cache is rebuilt and answers the same lookups.

**Acceptance Scenarios**:

1. **Given** a missing cache, **When** a lookup is requested, **Then** the cache is rebuilt from the repository and the lookup answers correctly.
2. **Given** a cache with an older or newer schema version, **When** Gedo opens it, **Then** it is discarded and rebuilt, and the rebuild is reported in diagnostics.
3. **Given** a corrupted cache file, **When** Gedo opens it, **Then** it is discarded and rebuilt instead of failing.

---

### User Story 5 - Gedo data travels with plain Git push and fetch (Priority: P1)

A developer pushes and fetches with their usual Git commands. After the repository is configured once, Gedo references and their objects travel to and from the remote alongside the branches, and a teammate who clones without Gedo sees a normal repository.

**Why this priority**: serverless operation (ADR-0005) depends entirely on hosts accepting, keeping and serving custom references; if they do not, the architecture must change before anything else is built.

**Independent Test**: configure a repository, create Gedo data, push to each kind of remote, clone the remote into an empty directory, fetch, and compare the Gedo references and objects; repeat after the host's garbage collection has had a chance to run.

**Acceptance Scenarios**:

1. **Given** a configured repository, **When** the developer runs a plain `git push` and a teammate a plain `git fetch`, **Then** the teammate has the same `refs/gedo/*` references and their objects.
2. **Given** a plain clone without the Gedo configuration, **When** the teammate works normally, **Then** nothing about Gedo appears in their branches, working tree or command output.
3. **Given** data pushed to the host, **When** the host's maintenance runs and the data is fetched again later, **Then** the objects referenced only by `refs/gedo/*` are still there.
4. **Given** the configuration step, **When** it is run twice, **Then** the remote configuration is not duplicated and existing settings are not altered.

Remotes covered: GitHub, GitLab, a plain file remote and a plain SSH remote. [NEEDS CLARIFICATION: where are the GitHub and GitLab test repositories, and should the host checks run in CI on every pull request or as a manual, on-demand validation?]

---

### User Story 6 - Know the reference limits before building on them (Priority: P2)

Before the identity and event layers create one reference per blob or per commit, the team measures how push, fetch and the hosts behave as the number of Gedo references grows, and decides from the numbers whether references must be consolidated into a few index trees.

**Why this priority**: a layout that collapses at a few thousand references would force a data migration later; measuring now is cheap.

**Independent Test**: generate repositories with growing numbers of Gedo references, measure push, fetch and clone times and sizes against each remote, and record the results and the layout decision.

**Acceptance Scenarios**:

1. **Given** reference counts from 100 up to 100,000, **When** push, fetch and clone are measured on each remote, **Then** the results are recorded in a reproducible report.
2. **Given** the measurements, **When** fetching the reference set of the reference repository (about 1,000 files with history) costs more than the thresholds below, **Then** a consolidated layout is designed and recorded in an ADR before v0.2.0 starts.

---

### User Story 7 - Every change is checked on Windows and Linux (Priority: P1)

Every pull request is built and tested on Windows and Linux; architecture rules, dependency licenses and security advisories are checked; performance benchmarks are run and reported.

**Why this priority**: Windows is a first-class platform from the start (line endings, paths, locking) and the rules of the constitution must be enforced mechanically.

**Independent Test**: open a pull request that breaks a test on one platform only, and another that imports an adapter library from the core; both are blocked.

**Acceptance Scenarios**:

1. **Given** a pull request, **When** it is opened or updated, **Then** build and tests run on Windows and Linux and the result is reported on the pull request.
2. **Given** code in the core that imports a forbidden adapter or infrastructure library, **When** CI runs, **Then** the pull request is blocked by the architecture check.
3. **Given** a new dependency with an incompatible license or a known advisory, **When** CI runs, **Then** the pull request is blocked.
4. **Given** the benchmarks, **When** CI runs, **Then** their results are published with the run.

---

### Edge Cases

- The repository is a bare repository, a worktree, or uses a non-default object directory or `core.hooksPath`: storage must still target the right `.git` location.
- The repository uses the SHA-256 object format: Gedo refuses clearly (SHA-256 support is FEAT-008, release v0.13.0) instead of writing wrong data.
- Two Gedo processes start at the same time and both try to create `.git/gedo/` and the log: one wins, the other uses what exists.
- The disk is full during an append or a cache rebuild: the error is reported, no partial record is left as valid, and earlier data stays intact.
- On Windows, another process (an antivirus, an IDE) holds the log or cache file briefly: Gedo retries within a bounded time and then reports a clear error.
- A reference name contains characters invalid on Windows or case-insensitive file systems: names are produced so that they are valid and distinct on every supported platform.
- A remote rejects custom references or deletes them: the validation reports it explicitly per host.
- Records written by a future format version: reading reports "unsupported format version" instead of misreading them.
- Very large data (tens of megabytes): stored and read without loading unnecessary copies; an upper size limit is enforced for data read from the remote (untrusted input).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The project MUST be organized so that the core (domain model, ports, later algorithms and use cases) depends on no adapter or infrastructure library, as defined in ADR-0001, and this rule MUST be checked automatically.
- **FR-002**: The core MUST define ports for object storage, reference storage, the write-ahead log and the symbol cache, each with an in-memory implementation that behaves like the real one for the core's tests.
- **FR-003**: Gedo MUST read and write blobs and trees in the repository's object store without spawning the `git` executable and without modifying the working tree, the index or `refs/heads/*`.
- **FR-004**: Gedo MUST create, read, list and delete references only under `refs/gedo/`, and MUST refuse any request to write outside that namespace.
- **FR-005**: Every reference update MUST be a compare-and-swap: it succeeds only when the reference still holds the expected value (or does not exist, when that is expected), and otherwise returns the current value without changing anything.
- **FR-006**: Reference updates MUST be atomic and recorded in Git's reference log, so a crash leaves either the old or the new value.
- **FR-007**: The write-ahead log MUST live in `.git/gedo/` (never pushed), be append-only, carry a checksum and a format version per record, and on opening MUST return every complete record in order, drop a torn last record, and report any other corruption with its position.
- **FR-008**: Records acknowledged as durable MUST survive a crash or power loss.
- **FR-009**: Only one process at a time MUST append to the log; a second writer MUST wait or receive a clear "log busy" result.
- **FR-010**: The symbol cache MUST live in `.git/gedo/`, store its schema version, and be discarded and rebuilt from the repository data when it is missing, corrupted or of another schema version.
- **FR-011**: All Gedo records (stored data, log records, cache entries) MUST carry a format version, and reading a record of an unknown version MUST fail clearly instead of misreading it.
- **FR-012**: Data read from the object store under `refs/gedo/` MUST be treated as untrusted: decoding MUST enforce size limits and MUST NOT crash the process on malformed input.
- **FR-013**: Gedo MUST be able to configure a repository's remotes so that plain `git push` and `git fetch` carry `refs/gedo/*`, idempotently and without changing other remote settings.
- **FR-014**: The validation of FR-013 MUST cover GitHub, GitLab, a plain file remote and a plain SSH remote, including retention after host maintenance, and its results MUST be recorded.
- **FR-015**: Push, fetch and clone times and transferred sizes MUST be measured for reference counts from 100 to 100,000 on the covered remotes, and the reference layout decision MUST be recorded in an ADR.
- **FR-016**: Every pull request MUST be built and tested on Windows and Linux, with the architecture check, the license and advisory checks and the benchmarks, and MUST be blocked when the build, a test, the architecture check or the license and advisory checks fail.
- **FR-017**: Failures MUST be reported with context (operation, path or reference, cause) and logged; no failure may be swallowed or replaced by a silent fallback.
- **FR-018**: Repositories in the SHA-256 object format MUST be detected and refused with a clear message until SHA-256 support is delivered.

### Key Entities

- **Object id**: the identity of a stored object in the repository (SHA-1 now; the model already allows SHA-256).
- **Stored object**: an opaque blob or a tree of named entries written by Gedo into the repository's object store.
- **Gedo reference**: a named pointer under `refs/gedo/` to a stored object, updated only by compare-and-swap; its history is kept in the reference log.
- **Log record**: one entry of the local write-ahead log, with a format version, a payload and a checksum; ordered; local to the machine.
- **Cache entry**: a derived lookup entry in the local symbol cache, tagged with the cache schema version.
- **Remote validation result**: per remote, whether references were pushed, fetched, cloned and retained, with timings and sizes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Storing and reading data never changes the developer's working tree, index or branches, verified on 100% of the storage test scenarios on Windows and Linux.
- **SC-002**: In a stress test with at least 8 concurrent writers and 10,000 updates of the same reference, zero updates are lost and every refusal reports the current value.
- **SC-003**: Appending one record to the local log takes under 1 ms at the 95th percentile on the reference machine (technical specification).
- **SC-004**: Cutting the log at every byte offset of its last record always yields all earlier records and never returns a corrupted one.
- **SC-005**: Rebuilding the cache after deletion, corruption or a schema change answers 100% of the reference lookups identically to before.
- **SC-006**: Gedo references survive a round trip (push, fresh clone with the configuration, fetch) and a retention check on each of the 4 covered remotes, or the failing remote is documented with its cause.
- **SC-007**: The reference-scaling report covers at least 5 reference counts between 100 and 100,000 on each covered remote, and the layout decision is recorded before v0.2.0 starts; consolidation is required if fetching 10,000 references adds more than 5 seconds or 50 MB over a fetch without them.
- **SC-008**: 100% of pull requests run the full check set on both platforms, and a deliberately broken test or architecture rule on either platform blocks the pull request.

## Assumptions

- No end-user command is delivered in this release; the storage capabilities are exercised through automated tests and validation tools, and the user-facing `gedo init` arrives in v0.4.0.
- "Concurrent writers" are processes on the same machine (CLI and daemon); multi-machine coordination happens through push and fetch.
- A single writer per log is enough: the daemon and the CLI coordinate through a lock, and the CLI writes through the daemon when it runs.
- Benchmark results in CI are published for trend tracking but do not block pull requests, because shared CI runners are too noisy for hard limits; the latency requirement is verified on the reference machine.
- The reference machine of the technical specification (4 cores, SSD) is used for the performance criteria.
- Only SHA-1 repositories are supported in this release (FR-018).
- The reference repository for measurements is about 100,000 lines in about 1,000 files, as in the technical specification.
