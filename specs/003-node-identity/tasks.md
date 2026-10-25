---

description: "Tasks for node identity (v0.2.0, second feature)"
---

# Tasks: Node Identity

**Input**: Design documents from `/specs/003-node-identity/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/identity.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md), in priority order. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US6 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new crate and the model change

- [ ] T001 Add `crates/core/gedo-app` to the workspace members and `gedo-app` to `[workspace.dependencies]` in `Cargo.toml`; create the crate (`Cargo.toml` depending on `gedo-core`, `gedo-ast`, `blake3`, `postcard`, `serde`, `thiserror`, `tracing`; dev-dependencies `gedo-core` with `memory`, `gedo-parse`, `gedo-store`, `proptest`, `insta`, `criterion`, `tempfile`; `src/lib.rs` with crate docs)
- [ ] T002 Verify the setup: `cargo build --workspace`, `cargo deny check` and `cargo xtask arch-check` pass, and `gedo-app` is accepted as a core crate

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the identity model and the unit of an entity, which every story uses

**⚠️ CRITICAL**: no user story starts before this phase is complete

- [ ] T003 [P] Add `unit` to `ParsedFile` and `SyntaxMap` in `crates/core/gedo-core/src/syntax.rs`, copy it in `gedo_ast::normalize`, and update `FixedParser` in `crates/core/gedo-core/src/memory/parser.rs`
- [ ] T004 [P] Add `unit_of(path)` to `crates/adapters/gedo-parse/src/rust/paths.rs` (research R4), fill `ParsedFile.unit`, add path-rule tests and accept the conformance snapshots with the new field (hashes unchanged)
- [ ] T005 [P] Create `crates/core/gedo-core/src/identity.rs`: `AstNodeId` (16 bytes, ordered, UUID display), `MatchFormat`, `AnchoredNode`, `BlobAstMap`, `SemanticDelta` with `Location`, `IdentityMerged`, with serde derives and doc comments; export from `lib.rs`
- [ ] T006 Unit tests for `AstNodeId` display and ordering and for `BlobAstMap` invariants (unique ids, containers before children) in `crates/core/gedo-core/src/identity.rs`

**Checkpoint**: model compiles on both platforms; snapshots updated only for `unit`.

---

## Phase 3: User Story 1 - An entity keeps its identity through edits, renames and moves (Priority: P1) 🎯 MVP

**Goal**: three-tier matching over a change set (research R3, R5, R6)

**Independent Test**: matching tests on before and after versions with known outcomes

- [ ] T007 [P] [US1] Write `crates/core/gedo-ast/tests/matching.rs` with failing cases: unchanged and reformatted entities (tier 1), body and signature edits in place (2a), move to another module, to another file and file rename with identical content (2b), rename with and without body edit (2c), unrelated removal and addition (3), ties, copies, splits and merges of entities
- [ ] T008 [US1] Implement `ChangeSet`, `MatchOutcome` and the tiers in `crates/core/gedo-ast/src/matching.rs`: ordering keys, tier 1, tier 2a, tier 2b with uniqueness, container resolution
- [ ] T009 [US1] Implement the similarity score in fixed-point thousandths (signature Dice without the own name, body, scope), `SIMILARITY_THRESHOLD_THOUSANDTHS = 800`, greedy assignment with the tie order and `MAX_SCORED_PAIRS` with candidate restriction in `crates/core/gedo-ast/src/matching.rs`
- [ ] T010 [US1] Implement `derive_id` (BLAKE3 derive-key `gedo node id v1`, version and variant bits, collision counter against live ids) in `crates/core/gedo-ast/src/identity.rs` and use it for tier 3
- [ ] T011 [US1] Make T007 pass; add a test that a change set of 5,000 entities per side stays within the pair bound

**Checkpoint**: US1 acceptance scenarios 1 to 6 pass.

---

## Phase 4: User Story 4 - Changes are described as semantic deltas (Priority: P1)

**Goal**: delta classification and order (research R12) and syntax errors (research R8)

**Independent Test**: expected deltas for each matching case

- [ ] T012 [P] [US4] Extend `crates/core/gedo-ast/tests/matching.rs` with the expected deltas of every case of T007, renamed plus body modified on one id, no delta for an unchanged file, and a damaged entity (carried old nodes, no tombstone, no `Created` until a valid version)
- [ ] T013 [US4] Implement `crates/core/gedo-ast/src/delta.rs` (classification, order by variant then id) and carried nodes in `matching.rs`
- [ ] T014 [US4] Make T012 pass

**Checkpoint**: US4 acceptance scenarios 1 to 4 pass.

---

## Phase 5: User Story 2 - The same history gives the same ids everywhere (Priority: P1)

**Goal**: determinism of matching and derivation

**Independent Test**: property tests and platform snapshot

- [ ] T015 [P] [US2] Write `crates/core/gedo-ast/tests/identity_properties.rs`: shuffled files and entities give the same encoded outcome; a version matched against itself keeps all ids with no delta; derived ids are unique over generated sets; resurrection gives the old id; a matched entity keeps its earlier id
- [ ] T016 [US2] Add an `insta` snapshot of the encoded outcomes of the matching cases in `crates/core/gedo-ast/tests/matching.rs`, checked on Windows and Linux
- [ ] T017 [US2] Fix any order dependence found; document `MatchFormat::V1` rules in the module docs

**Checkpoint**: US2 scenarios 1, 4, 5 and 6 pass at the algorithm level (2 and 3 close with US5).

---

## Phase 6: User Story 3 - Two identities for one entity are reconciled the same way everywhere (Priority: P1)

**Goal**: alias reconciliation (research R11)

**Independent Test**: alias properties and merge histories

- [ ] T018 [P] [US3] Write tests in `crates/core/gedo-ast/src/alias.rs`: resolution to the lowest id of the component, chains, invariance under permutation and grouping of merges, combination of two tables
- [ ] T019 [US3] Implement `AliasTable` (union-find with path compression) in `crates/core/gedo-ast/src/alias.rs`

**Checkpoint**: US3 scenarios 2 and 3 pass; scenario 1 closes with US5 (merge commits).

---

## Phase 7: User Story 5 - Existing history and teammates' commits are indexed (Priority: P2)

**Goal**: `gedo-app::index` (research R10)

**Independent Test**: generated histories on the in-memory store, the Gedo repository's history

- [ ] T020 [P] [US5] Implement `crates/core/gedo-app/src/index/walk.rs` (ancestry from tips to every root, skipping indexed commits, topological order with ties by id) with unit tests including several roots
- [ ] T021 [P] [US5] Implement `crates/core/gedo-app/src/index/diff.rs` (tree diff against the first parent, equal subtrees skipped, Rust files only through `Parser::language_of`) with unit tests
- [ ] T022 [P] [US5] Implement `crates/core/gedo-app/src/index/records.rs` (`CommitRecord`, `Checkpoint`, map digests, `RecordEnvelope` format 1, size limits) with round-trip and malformed-input tests
- [ ] T023 [US5] Implement `IdentityIndex` in `crates/core/gedo-app/src/index/mod.rs`: `index_history` (parse changed blobs once per run on worker threads, match against the first parent, reconcile against other parents at merges, one cache transaction per commit, checkpoints every 64 first-parent commits), `map_at`, `changes_of`, `resolve`, `IndexError`, `IndexReport`, `tracing` events
- [ ] T024 [US5] Write `crates/core/gedo-app/tests/history.rs`: linear, branching, merge, octopus and multi-root histories on `MemoryObjectStore`; indexed in at least 3 orders of tips and of branches; interrupted after random commits and resumed; re-run writes nothing; cleared cache rebuilt identical; independent identical creations get one id; independent different creations merged later give one `IdentityMerged` with the lowest id kept (US3 scenario 1)
- [ ] T025 [US5] Write `crates/core/gedo-app/tests/gedo_history.rs`: index the repository's own history through `gedo-store` and compare the identities of selected commits with a committed `insta` snapshot (skipped with a message on a shallow clone)
- [ ] T026 [US5] Add `crates/core/gedo-app/examples/index_history.rs` (quickstart step 3 and 4)

**Checkpoint**: US5 scenarios 1 to 4 and US2 scenarios 2 and 3 pass.

---

## Phase 8: User Story 6 - The identity metrics are measured on a reference corpus (Priority: P2)

**Goal**: FEAT-014 (research R9)

**Independent Test**: the corpus report and its gates

- [ ] T027 [US6] Commit the corpus source files under `crates/core/gedo-app/tests/fixtures/identity/` (the conformance fixtures and a pinned snapshot of workspace sources) and write the seeded edit generator in `crates/core/gedo-app/tests/identity_corpus.rs` for every category of research R9 with expected outcomes
- [ ] T028 [US6] Compute accuracy per category and overall and false continuations, print the table, and fail below 95% overall, 90% per category or above 1% false continuations
- [ ] T029 [US6] Tune the weights and threshold of research R5 on the corpus; record the final values and the table in `research.md` R5 and in the plan envelope
- [ ] T030 [P] [US6] Add benchmarks `crates/core/gedo-ast/benches/match_change.rs` (2,000-line file) and `crates/core/gedo-app/benches/index_history.rs` (generated history and, when `GEDO_BENCH_REPO` is set, a real repository)

**Checkpoint**: the corpus gates pass on Windows and Linux.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T031 [P] Add `fuzz/fuzz_targets/decode_identity_record.rs` and register it in `.github/workflows/fuzz.yml`
- [ ] T032 [P] Update `.github/workflows/ci.yml`: `fetch-depth: 0` in the test job; bench job runs `match_change` and `index_history` with ripgrep cloned at a pinned tag and commit
- [ ] T033 [P] Documentation: `docs/architecture/identity.md`, `core.md`, `store.md`, `TECDOC.md`, TECH_SPEC data table; new `docs/adr/0012-identity-results-are-local-derived-data-computed-per-commit.md`; run `build_index.py`, `check_docs.py`, `check_readme.py` and `constitution.py generate` and `check`
- [ ] T034 Walk through `quickstart.md` steps 1 to 5 and fix any drift between the guide and the code
- [ ] T035 Run the benchmarks on Windows and Linux and record p50, p95 and p99 of `match_change` and the indexing times in `docs/plans/gedo_node_identity.md` (SC-004, SC-005)

---

## Dependencies & Execution Order

- **Setup (T001–T002)** → **Foundational (T003–T006)** → stories.
- **US1 (T007–T011)** first; **US4 (T012–T014)** and **US2 (T015–T017)** depend on US1; **US3 (T018–T019)** is independent of US1 and can run in parallel with it; **US5 (T020–T026)** depends on US1, US4 and US3; **US6 (T027–T030)** depends on US1 and US4.
- **Polish (T031–T035)** after all stories.

### Parallel Opportunities

- Foundational: T003, T004 and T005 in parallel.
- US3 (T018, T019) alongside US1.
- US5: T020, T021 and T022 in parallel before T023.
- US6's T030 alongside T027 to T029; Polish T031 to T033 in parallel.

## Implementation Strategy

1. Setup and Foundational, then **US1** (MVP slice: matching keeps identities) and validate it alone.
2. US4 and US2 make the output complete and deterministic; US3 adds reconciliation.
3. US5 applies everything to Git histories; US6 measures accuracy and tunes the score.
4. Each checkpoint maps to a plan milestone: M1 Setup and identity model, M2 Matching, M3 Deltas, M4 Determinism and reconciliation, M5 History indexing, M6 Identity corpus, M7 Documentation and acceptance.
