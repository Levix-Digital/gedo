# Feature Specification: Node Identity

**Feature Branch**: `feat/003-node-identity`

**Created**: 2026-10-25

**Status**: Draft

**Input**: User description: "Node identity (release v0.2.0, second of two features, backlog items FEAT-011 to FEAT-015): three-tier matching of the code entities of two versions of the code (exact match, signature and scope heuristics with a tuned 80% threshold, spawn or tombstone fallback); identities derived deterministically from content and history, with a versioned matching algorithm and alias reconciliation when two identities are found for one entity (ADR-0007); the semantic deltas that describe what changed (created, moved, renamed, signature changed, body changed, tombstoned); a reference identity corpus with benchmarks for accuracy (95% target) and determinism across machines; and the indexing of history made without Gedo, so existing repositories and teammates' commits get the same ids on every machine. Builds on the syntax model of feature 002."

## Context

Feature 002 turns one version of a Rust file into its **code entities**, each with a kind, a qualified path and structural, signature and body hashes that ignore formatting and comments. This feature, the second and last of v0.2.0 (Node identity), gives each entity an **identity** (`AstNodeId`) that it keeps for its whole life: through edits of its body, changes of its signature, renames, moves and formatter runs. It also describes each change between two versions as a **semantic delta**, the raw material of the event log of v0.3.0.

Identities must be **deterministic** (ADR-0007): the same history produces the same ids on every machine, whether or not Gedo was running when the code was written, so a fresh clone, a teammate without Gedo and a server-side merge all agree without talking to each other. When two ids are nevertheless found for one entity (the same entity written independently on two branches, for example), a fixed rule keeps one and records the other as an **alias**.

Like v0.1.0 and feature 002, this feature has no end-user command (`gedo init` and the daemon arrive in v0.4.0). Its users are the **event layer** (v0.3.0), which turns deltas into events, the **merge driver, trace and graft** (v0.4.0), which follow entities by id, and the **Levix developers** who build them. The vision's identity metrics are proven here: at least 95% of identities preserved on the reference corpus, and identical ids for the same history on every machine and in every indexing order.

## Clarifications

### Session 2026-10-25

- Q: Is a move from one file to another file in the same change recognized as a move? → A: Yes; matching works on the whole change set (all files of a commit), so an entity that leaves one file can be matched to one that appears in another, and it keeps its identity with a "moved" delta.
- Q: Is the whole history always indexed from the first commit, or only from a shared starting point? → A: The whole history, from every root commit; it is the only fully deterministic choice. The cost of the first indexing is measured by the benchmarks, and results shared through the Gedo data of a remote are reused instead of recomputed.
- Q: How is identity accuracy measured? → A: On scripted edits of real Rust code (renames, moves, signature changes, formatter runs, copies, splits, ...) whose correct outcome is known; real histories are used for determinism across machines and orders, not for accuracy.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An entity keeps its identity through edits, renames and moves (Priority: P1)

A developer edits the code: changes a function's body, adds a parameter, renames a method, moves a function to another module, reformats the file. Given the previous and the new version, Gedo recognizes each entity that still exists and keeps its identity, and gives new identities only to entities that are really new.

**Why this priority**: trace, graft, the merge driver and the threads all follow entities by identity; an identity lost on a rename splits an entity's history in two.

**Independent Test**: apply scripted edits (body edits, signature changes, renames, moves, formatter runs, reorders) to real Rust files whose correct outcome is known, match the before and after versions, and compare the kept and new identities with the expected ones.

**Acceptance Scenarios**:

1. **Given** an entity whose content did not change, **When** the file is reformatted, lines are added above it or other entities change, **Then** it keeps its identity (exact match).
2. **Given** a function whose body was edited, **When** the versions are matched, **Then** it keeps its identity and a "body modified" delta is reported.
3. **Given** a method whose signature changed (a parameter added, a return type changed, visibility changed), **When** the versions are matched, **Then** it keeps its identity and a "signature mutated" delta is reported.
4. **Given** a function renamed in place with its body unchanged or slightly edited, **When** the versions are matched, **Then** it keeps its identity and a "renamed" delta is reported.
5. **Given** a function moved to another container or another module, **When** the versions are matched, **Then** it keeps its identity and a "moved" delta is reported. This includes moves to another file in the same change.
6. **Given** an entity removed and an unrelated entity added, **When** the versions are matched, **Then** the removed one is reported as tombstoned and the added one as created with a new identity.

---

### User Story 2 - The same history gives the same ids everywhere (Priority: P1)

Two developers index the same repository on different machines (Windows and Linux), in different orders, one with Gedo running while the code was written and one from a fresh clone of commits made without Gedo. Every entity gets exactly the same identity on both machines.

**Why this priority**: Gedo has no server (ADR-0005); identities only agree between machines if they are a pure function of the code and its history (ADR-0007). A difference here splits entities at every merge.

**Independent Test**: index the same histories on Windows and Linux in CI, and in several orders on one machine, and compare the resulting identities with a committed snapshot byte for byte.

**Acceptance Scenarios**:

1. **Given** a new entity, **When** it is first seen, **Then** its identity is derived only from its kind, its crate, its qualified path (with its occurrence) and its structural hash, so any machine derives the same one.
2. **Given** a repository history, **When** it is indexed on Windows and on Linux, **Then** every entity of every indexed version has the same identity on both.
3. **Given** a history with several branches, **When** it is indexed branch by branch in different orders, **Then** the identities are the same.
4. **Given** two developers who independently create the same entity with the same content, **When** each indexes their own work, **Then** both get the same identity.
5. **Given** an entity deleted and later re-created with the same path and content, **When** it reappears, **Then** it gets its old identity back.
6. **Given** a published identity, **When** a newer version of the matching algorithm is released, **Then** the identity is kept; the algorithm version that produced each result is recorded with it.

---

### User Story 3 - Two identities for one entity are reconciled the same way everywhere (Priority: P1)

The same function is created on two branches with different bodies, so it gets two identities; later the branches are merged and the function exists once. Gedo sees that both identities name one entity, keeps the lower one and records the other as an alias, and every machine reaches the same result.

**Why this priority**: without reconciliation, merges and re-indexed histories would leave entities with several identities, and lookups by the old identity would fail.

**Independent Test**: build histories where one entity receives two identities (independent creation on two branches, then a merge), index them in different orders and on both platforms, and check that the kept identity and the alias are the same everywhere and that the alias resolves to the kept identity.

**Acceptance Scenarios**:

1. **Given** a merge where two identities match one entity, **When** it is indexed, **Then** the lower identity is kept, the other becomes an alias of it, and an "identity merged" record is produced.
2. **Given** an alias, **When** any component looks up the aliased identity, **Then** it resolves to the kept identity, also through chains of aliases.
3. **Given** the same reconciliation found on two machines, **When** their alias records are combined, **Then** they agree and nothing conflicts.

---

### User Story 4 - Changes are described as semantic deltas (Priority: P1)

For each pair of versions, Gedo reports what happened to each entity as an ordered list of deltas: created, moved, renamed, signature mutated, body modified, tombstoned. The event layer (v0.3.0) turns them into events, and the semantic diff (v0.4.0) shows them to people.

**Why this priority**: deltas are the input of the event log; the release goal of v0.3.0 depends on them.

**Independent Test**: for each scripted edit of the corpus, compare the deltas produced with the expected deltas.

**Acceptance Scenarios**:

1. **Given** an unchanged file, **When** its versions are matched, **Then** no delta is produced.
2. **Given** an entity both renamed and with its body edited, **When** the versions are matched, **Then** both a "renamed" and a "body modified" delta are produced for the same identity.
3. **Given** the same pair of versions, **When** deltas are produced twice or on another machine, **Then** they are identical, in the same order.
4. **Given** a version with a syntax error inside one entity, **When** it is matched, **Then** entities outside the damaged region produce the deltas they would produce without the error, and the damaged entity is not reported as tombstoned.

---

### User Story 5 - Existing history and teammates' commits are indexed (Priority: P2)

A team adopts Gedo on a repository with years of history, and some teammates keep committing without Gedo. Gedo indexes those commits from the Git history alone and reaches the identities that live indexing would have produced. The whole history is indexed, from every root commit, so the result never depends on where indexing started.

**Why this priority**: most of the code Gedo will see was written without it; without history indexing, identities of old entities would depend on when Gedo was installed.

**Independent Test**: index a real repository's history in one run, then index the same history commit by commit as live work would, and compare the identities.

**Acceptance Scenarios**:

1. **Given** a repository never indexed, **When** its history is indexed, **Then** every Rust entity in every indexed commit has an identity, and renames and moves across commits keep identities.
2. **Given** commits fetched from a teammate who does not use Gedo, **When** they are indexed, **Then** the resulting identities are the ones the teammate would have obtained with Gedo running.
3. **Given** an indexing interrupted part way, **When** it is resumed, **Then** the result is the same as an uninterrupted run.
4. **Given** results already indexed on this machine, **When** indexing runs again, **Then** only commits without results are indexed; **Given** the local results are deleted or corrupted, **When** indexing runs again, **Then** it rebuilds them identical.

---

### User Story 6 - The identity metrics are measured on a reference corpus (Priority: P2)

The project keeps a reference identity corpus of renames, moves, signature changes, body edits and formatter runs with their expected identities, and benchmarks that report identity accuracy, determinism across machines and matching time on every pull request. Accuracy is measured on scripted edits of real code whose outcome is known; real histories measure determinism.

**Why this priority**: the 95% accuracy target and the 80% similarity threshold are only meaningful when measured; the corpus is also the regression net for every later change of the matching.

**Independent Test**: run the corpus in CI on Windows and Linux and read the accuracy and determinism report; degrade the matching on purpose and see the accuracy check fail.

**Acceptance Scenarios**:

1. **Given** the corpus, **When** it runs in CI, **Then** it reports identity accuracy per category of edit and overall, and fails below the target.
2. **Given** the corpus, **When** it runs on Windows and Linux, **Then** the identities produced are identical on both.
3. **Given** a change of the similarity threshold, **When** the corpus runs, **Then** the effect on accuracy is visible per category.

---

### Edge Cases

- Several entities with the same kind and qualified path in one file (code under mutually exclusive `#[cfg]`, several `impl` blocks of one type): distinguished by occurrence, and their identities stay distinct and stable.
- Two entities with identical content in different files or crates of a workspace: different identities, because the crate and path take part in the derivation.
- An entity duplicated (copied) within a change: the original keeps its identity; the copy is created.
- One entity split into two, or two merged into one: at most one of them keeps the identity, the rest are created or tombstoned.
- Two candidates equally similar to one old entity: the choice follows a fixed, documented order, never the order of discovery.
- A file renamed or moved (its module path changes) with unchanged content: every entity keeps its identity and is reported as moved.
- A file deleted, a file added, a file that is not Rust, or a binary file: entities tombstoned or created; other languages are ignored.
- A version with syntax errors, an empty or truncated file: the damaged entities are neither matched as new nor tombstoned on the strength of the error alone.
- Very large changes (a file rewritten, thousands of entities): matching stays within its time budget and never degrades into comparing every pair without limit.
- Merge commits, octopus merges, root commits and histories with several roots.
- Identities already stored in an older id form (version 4 or 7) remain valid when read (ADR-0007).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The core MUST define the identity of a code entity (`AstNodeId`, 16 bytes) and the per-file result that anchors identities to the entities of one file version: for each entity its identity, kind, qualified path, byte range and hashes, with the source blob, the language, the hash format and the matching algorithm version.
- **FR-002**: Matching MUST work on a change set (the files changed between two commits, or one file) and compare the entities of the previous and the new versions across all its files and decide for each new entity whether it continues an earlier one, in three tiers in this order: (1) exact match on qualified path and structural hash; (2) signature and scope heuristics among the remaining entities of the same kind, keeping the identity when the similarity reaches a named, tuned threshold (initially 80%); (3) otherwise a new identity, and every unmatched earlier entity is tombstoned.
- **FR-003**: Matching MUST be deterministic: the same inputs give the same result on every platform, build, thread count and input order; ties MUST be broken by a fixed, documented rule.
- **FR-004**: A new identity MUST be derived only from the entity's kind, its crate, its qualified path (with its occurrence) and its structural hash, under a domain-separated, versioned derivation (ADR-0007); a matched entity MUST keep its earlier identity.
- **FR-005**: The matching algorithm MUST be versioned; the version MUST be stored with every result, and identities already produced MUST never be recomputed by a newer version.
- **FR-006**: Matching MUST produce the semantic deltas of the change, in a deterministic order: created, moved, renamed, signature mutated, body modified, tombstoned; one entity MAY produce several deltas (renamed and body modified, for example).
- **FR-007**: When two identities are found for what the matching considers one entity, the lower identity MUST be kept and the other recorded as its alias in an "identity merged" record; alias records MUST combine without conflict across machines, and lookups MUST resolve aliases, including chains.
- **FR-008**: An entity flagged with a syntax error MUST NOT be tombstoned or re-created because of the error alone; matching MUST treat it so that the next valid version keeps the identity.
- **FR-009**: Gedo MUST index Rust code from Git history alone (commits, trees and blobs), always the whole history from every root commit, without Gedo having run when the commits were made, and reach the same identities as live indexing of the same history.
- **FR-010**: Indexing MUST be resumable and idempotent: it MUST reuse the results already computed locally and index only commits without results; results rebuilt from scratch MUST equal the previous ones.
- **FR-011**: The per-commit results and the identity merges MUST be kept as local derived data (rebuildable from the Git history, never pushed) in versioned formats that are decoded safely (size limits, format versions, no panic on malformed input), and the decoders MUST be fuzzed. Shared identity data travels with the events of v0.3.0 (research R2).
- **FR-012**: A reference identity corpus of scripted edits on real Rust code, with known outcomes, MUST cover at least: body edits, signature changes, renames, moves within a file and between modules, formatter runs, reorders, copies, splits, merges, deletions and additions, each case with its expected identities and deltas.
- **FR-013**: Benchmarks MUST report identity accuracy per category and overall (on the scripted corpus), determinism across Windows and Linux and across indexing orders (on real histories), and matching time, and CI MUST fail when accuracy drops below the target or when identities differ between platforms.
- **FR-014**: Errors (unreadable history, malformed stored data, unsupported format version) MUST carry context and MUST NOT be swallowed; matching itself never fails on valid syntax maps.
- **FR-015**: The matching, derivation and reconciliation MUST be pure core logic without I/O (ADR-0001), covered by property-based tests, including indexing the same history in different orders and on different branches.

### Key Entities

- **Node identity (`AstNodeId`)**: 16 bytes that name one code entity for its whole life; derived from content when the entity first appears, kept while the matching recognizes it.
- **Anchored node**: an entity of one file version with its identity, kind, qualified path, byte range and hashes.
- **Blob AST map**: the anchored nodes of one file version, with the source blob, language, hash format and matching algorithm version; immutable once stored.
- **Semantic delta**: one change of one entity between two versions: created, moved, renamed, signature mutated, body modified or tombstoned, with the identity it concerns and the before and after details it needs.
- **Identity alias**: a record that an identity is merged into a lower one; the alias table resolves any identity to the one kept.
- **Matching algorithm version**: the version of the rules that produced a result.
- **Reference identity corpus**: cases of edits on real code with their expected identities and deltas, and the accuracy report computed from them.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the reference identity corpus, at least 95% of the identities that should be preserved are preserved, overall, and no category of edit is below 90%.
- **SC-002**: On the corpus, fewer than 1% of the entities that should get a new identity wrongly keep an old one (false continuations).
- **SC-003**: The same histories indexed on Windows and Linux, and in at least 3 different orders, give identical identities, deltas and alias records, byte for byte, on every pull request.
- **SC-004**: Matching the entities of a 2,000-line file against its previous version takes under 5 ms at the 95th percentile on the reference machine, so parse (under 10 ms, feature 002), matching and the log append fit the 20 ms ingest budget of the technical specification.
- **SC-005**: Indexing the history of the reference repository (about 100,000 lines of Rust) completes in a time recorded by the benchmarks; indexing a single fetched commit takes under 100 ms at the 95th percentile, like re-attachment in the technical specification.
- **SC-006**: Re-indexing an already indexed history produces no new data, and an interrupted indexing resumed gives the same result as an uninterrupted one, in 100% of the property-based runs.
- **SC-007**: Property-based tests and at least 10 minutes of coverage-guided fuzzing of the stored-data decoders per release candidate find no crash, hang or wrong result.

## Assumptions

- No end-user command is delivered; `gedo init`, the daemon and `gedo trace` arrive in v0.4.0. The capabilities are exercised through tests, benchmarks and the corpus.
- Deltas and "identity merged" records are produced as core values and stored Gedo data; turning them into events with Lamport clocks and event ids is FEAT-063 (v0.3.0), and the reconciliation inside a real merge driver is v0.4.0. The reconciliation rule is complete here so that both can call it.
- Crates of a workspace are told apart by the file path (the crate's directory), without reading manifests, as decided in feature 002.
- Only Rust is indexed; files of other languages are skipped and do not stop indexing.
- The similarity threshold starts at 80% and is a named constant tuned on the corpus; changing it after release creates a new matching algorithm version.
- Identity results are a pure function of the history, so they are recomputed on each machine instead of shared; live identities (before a commit exists) travel with the events of v0.3.0 (ADR-0007).
- Indexing reads the Git object store through the existing store adapter; it never runs `git` as a subprocess (technical specification constraints).
- The reference machine of the technical specification is used for SC-004 and SC-005; CI benchmarks are reported but do not block pull requests, while accuracy and determinism checks do.
