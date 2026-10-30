---

description: "Tasks for the tree CRDT (v0.3.0, third feature)"
---

# Tasks: Tree CRDT

**Input**: Design documents from `/specs/006-tree-crdt/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/convergence.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md). The state and the fold (foundational) come first because every story uses them. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US5 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new modules, benchmarks and test files

- [ ] T001 Create `crates/core/gedo-crdt/src/tree.rs` (module docs, exported from `lib.rs`) and `crates/core/gedo-app/src/events/converge.rs` (an `impl` block of `EventLog`, declared in `events/mod.rs`)
- [ ] T002 [P] Add `criterion` as a dev-dependency and a `[[bench]] tree` to `crates/core/gedo-crdt/Cargo.toml`; create `crates/core/gedo-crdt/tests/tree.rs` with generators of bases and sides (proptest strategies for creations, moves, renames, signature and body changes, removals, identity merges)

**Checkpoint**: the workspace builds; `cargo xtask arch-check` passes.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the state and the fold every story uses (research R3, R4)

**⚠️ CRITICAL**: no user story starts before this phase is complete

- [ ] T003 Implement `Part`, `Place`, `EntityState`, `TreeState` (`from_maps`, `entity`, `members`, aliases) in `crates/core/gedo-crdt/src/tree.rs`, with the split of a qualified path at its last top-level separator (research R3); unit tests (impl paths with `::` inside angle brackets, file-level entities, aliases)
- [ ] T004 Implement `fold` (research R4): deduplication by event id, alias resolution, total order, application per operation, `set_by` per part, neutral entities, `TreeError::UnknownEntity` and `InconsistentRename`; unit tests per operation and per error
- [ ] T005 [P] Property-based tests of the fold in `crates/core/gedo-crdt/tests/tree.rs`: identical result for any order of the events and with duplicated events

**Checkpoint**: state and fold complete and tested (milestone M2).

---

## Phase 3: User Story 1 - Members as a set (Priority: P1)

**Goal**: independent changes to the same structure never conflict.

**Independent Test**: two sides adding and removing different members of the same containers converge without conflict.

- [ ] T006 [US1] Implement `converge` in `crates/core/gedo-crdt/src/tree.rs` (research R5): per entity and part, changed values per side, agreement, conflicts keeping the base value, `Convergence` sorted output; members by place
- [ ] T007 [US1] Implement entities added on several sides (research R7): same kind and qualified path, equal structural hash linked with the lower id kept, different hashes in an added-on-both-sides conflict
- [ ] T008 [P] [US1] Scenario tests for US1 acceptance scenarios 1 to 4 in `crates/core/gedo-crdt/tests/tree.rs`

**Checkpoint**: members converge as a set (milestone M3).

---

## Phase 4: User Story 2 - Move and edit commute (Priority: P1)

**Goal**: changes to different parts of one entity by different sides all reach the result.

**Independent Test**: every pair of changes on different parts, in both orders, gives the expected entity.

- [ ] T009 [US2] Implement the commuting rules of research R6 in `crates/core/gedo-crdt/src/tree.rs`: place and name rebuilt into the qualified path, signature and body from different sides, the converged structural hash (known from one side, unknown when combined)
- [ ] T010 [P] [US2] Scenario tests for US2 acceptance scenarios 1 to 4, and a property test "no lost change" (every part changed by one side only reaches the result)

**Checkpoint**: commuting changes converge (milestone M4).

---

## Phase 5: User Story 3 - Conflicts reported (Priority: P1)

**Goal**: incompatible changes are reported with their sides and events, never hidden.

**Independent Test**: each kind of incompatible change gives the expected conflict and keeps unrelated changes.

- [ ] T011 [US3] Implement presence conflicts (removal against change) and removed containers against new or moved members (research R6), with `others` listing the members
- [ ] T012 [US3] Fill each conflict's values, sides and events from `set_by` (research R8)
- [ ] T013 [P] [US3] Scenario tests for US3 acceptance scenarios 1 to 5

**Checkpoint**: conflicts complete (milestone M5).

---

## Phase 6: User Story 4 - Determinism and idempotence (Priority: P1)

**Goal**: the same result in any arrival order, with every event applied once.

**Independent Test**: property-based tests over generated bases and sides.

- [ ] T014 [US4] Property-based tests in `crates/core/gedo-crdt/tests/tree.rs` (research R11): events shared between sides, permuted sides (results equal up to side positions), convergence with itself and with an empty side, one side at a time against all at once without conflicts, three or more sides; at least 256 cases each
- [ ] T015 [P] [US4] Encoding snapshot (`insta`) of the convergence of a fixed scenario, compared on Windows and Linux in CI
- [ ] T016 [P] [US4] Benchmark `crates/core/gedo-crdt/benches/tree.rs`: two sides of 10,000 events over 10,000 entities (SC-004)

**Checkpoint**: the CRDT properties hold (milestone M6).

---

## Phase 7: User Story 5 - Converge commits (Priority: P2)

**Goal**: the merge driver and the shadow merge can converge commits of a real repository.

**Independent Test**: scripted branches from a common commit converge to the expected state and conflicts.

- [ ] T017 [US5] Add `IdentityIndex::changed_files(commit)` in `crates/core/gedo-app/src/index/mod.rs` (the paths whose blob differs from the first parent), with a unit test
- [ ] T018 [US5] Implement `EventLog::converge` in `crates/core/gedo-app/src/events/converge.rs` (research R9): commits of each side after the base in generation order, `events_of_commit`, live events, base state from the changed files with the rest loaded on demand, `EventError::Tree`, `ConvergeReport`, the logs of the contract
- [ ] T019 [P] [US5] Tests in `crates/core/gedo-app/tests/converge.rs`: two branches (additions to the same `impl`, move against edit, removal against edit), published against reconstructed, a graft, live events on top, a side or base not indexed; `insta` snapshot; `tests/event_tracing.rs` asserts "sides converged"
- [ ] T020 [P] [US5] Benchmark `crates/core/gedo-app/benches/converge.rs` (two branches of 100 commits, SC-005) and example `crates/core/gedo-app/examples/converge.rs`

**Checkpoint**: commits converge (milestone M7).

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T021 ADR-0017 "Tree CRDT as a three-way convergence of entity parts" (amends ADR-0009), with `architecture-docs` scripts
- [ ] T022 Update `docs/architecture/events.md`, `docs/architecture/core.md` and `TECDOC.md`; run `build_index` and `check_docs`
- [ ] T023 `ci.yml` bench job runs the `tree` and `converge` benchmarks
- [ ] T024 Regenerate the constitution (`constitution generate`) and check it
- [ ] T025 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask arch-check`, `cargo deny check`, `check_design`, `manual_tests validate`, `check_readme`; fix what they report

**Checkpoint**: hardening and docs complete (milestone M8).

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phases 3 to 6 (US1 before US2 and US3, which extend `converge`; US4 after US1 to US3) → Phase 7 → Phase 8.
- Tasks marked [P] touch different files and can run beside the task before them.

## Implementation Strategy

MVP is US1 to US4 (the pure CRDT); US5 makes it callable on commits. Each phase ends at its checkpoint with every test green.
