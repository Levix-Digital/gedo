---

description: "Tasks for semantic lanes and split publish (v0.3.0, fourth feature)"
---

# Tasks: Semantic Lanes and Split Publish

**Input**: Design documents from `/specs/007-semantic-lanes/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/split.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md). The lanes (US1) come first because the builder and the use case need them. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US3 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new modules, dependency, benchmark and test files

- [ ] T001 Add `similar` to `[workspace.dependencies]` and to `crates/core/gedo-crdt/Cargo.toml`; create `crates/core/gedo-crdt/src/lanes.rs` and `src/split.rs` (module docs, exported from `lib.rs`) and `crates/core/gedo-app/src/events/split.rs` (an `impl` block of `EventLog`, declared in `events/mod.rs`)
- [ ] T002 [P] Create `crates/core/gedo-crdt/tests/lanes.rs` with helpers that parse Rust sources with `gedo-parse` (dev-dependency) and anchor them with `gedo-ast`; add `[[bench]] split` to `crates/core/gedo-app/Cargo.toml`

**Checkpoint**: the workspace builds; `cargo xtask arch-check` and `cargo deny check` pass.

---

## Phase 2: User Story 1 - Lanes (Priority: P1) 🎯 MVP

**Goal**: every change of a publish has exactly one lane (research R2 to R4)

**Independent Test**: `cargo test -p gedo-crdt --test lanes lanes_`

- [ ] T003 [US1] Implement `Lane`, `FollowUps` (from `Renamed` and `Moved` deltas), `LanedChange`, `LanePlan` (`lane_of`, `has`, `parts`) in `crates/core/gedo-crdt/src/lanes.rs`
- [ ] T004 [US1] Implement the token diff with follow-up equality and the path follow-up test (research R3) as `is_follow_up` in `lanes.rs`; unit tests: renamed call, moved path, `use` path, a local variable named like the old name, a real edit next to a rename
- [ ] T005 [US1] Implement `classify` (R2, R4: module splits) and `event_lane` (R7) in `lanes.rs`
- [ ] T006 [US1] Tests per acceptance scenario of US1 in `crates/core/gedo-crdt/tests/lanes.rs` (each delta kind, rename with body change, follow-up body, determinism)

**Checkpoint**: US1 acceptance scenarios pass.

---

## Phase 3: Refactor builder (US2, US3 foundation)

**Goal**: the refactor version of every changed file (research R5) and its message (R8)

- [ ] T007 [US2] Implement units and blocks (moved entities as one unit keyed by id) and `build` with the recursive rebuild of moved entities in `crates/core/gedo-crdt/src/split.rs`; `BuildError` for a moved entity whose base version is missing or blocks that overlap
- [ ] T008 [US2] Implement `refactor_message` (R8) in `split.rs`; unit tests (order, `and N more`, body lines)
- [ ] T009 [US2] Builder scenario tests in `tests/lanes.rs`: rename with callers in other files, move within a file, move across files, module split, mixed work, created and removed entities, formatting-only changes; each refactor text re-parsed and classified against the base gives only refactor changes
- [ ] T010 [US2] Property test (256 cases) in `tests/lanes.rs`: generated files with renames, moves and feature edits; the refactor text keeps every feature change as in the base and every refactor change as in the final version

**Checkpoint**: builder tests pass.

---

## Phase 4: User Story 2 - Split publish (Priority: P1)

**Goal**: `EventLog::publish_split` writes, verifies and publishes the two commits (research R6, R7)

**Independent Test**: `cargo test -p gedo-app --test split split_`

- [ ] T011 [US2] Extract the storing steps of `publish` (events, tapes, index entries, cache) into a private helper that stores several tapes and truncates the log once, in `crates/core/gedo-app/src/events/mod.rs`; existing tests unchanged
- [ ] T012 [US2] Implement `SplitRequest`, `SplitOutcome`, `PublishedCommit`, `NotSplit` and `publish_split` (single commit, classification, build, refactor and feature commits, indexing, verification, event lanes, two tapes) in `crates/core/gedo-app/src/events/split.rs`, with the `split published` info event
- [ ] T013 [US2] Scripted-repository tests per acceptance scenario of US2 in `crates/core/gedo-app/tests/split.rs` (both lanes, one lane, nothing to publish, tapes and `events_of_commit` per commit, determinism and idempotence), `insta` snapshot of the outcome

**Checkpoint**: US2 acceptance scenarios pass.

---

## Phase 5: User Story 3 - Safe fallback (Priority: P1)

**Goal**: a split that cannot be verified is published as one commit with the reason

- [ ] T014 [US3] Tests in `tests/split.rs`: refactor version with a syntax error, a mismatch found by re-indexing (a forced builder slip through a crafted case), a build error; each gives `Single` with its reason and the final tree; a base not indexed is an error and keeps the log
- [ ] T015 [US3] Property test in `tests/split.rs` (at least 32 cases, repositories are slow): for generated work, the last commit's tree is always the final tree and a split's parts equal the lanes

**Checkpoint**: US3 acceptance scenarios pass.

---

## Phase 6: Hardening and docs

- [ ] T016 [P] `crates/core/gedo-app/benches/split.rs` (100 files, 1,000 events, SC-003) and the bench job in `.github/workflows/ci.yml`
- [ ] T017 [P] `crates/core/gedo-app/examples/split.rs` (quickstart step 4); `split published` asserted in `tests/event_tracing.rs`
- [ ] T018 [P] ADR-0018 (`architecture-docs` new_adr), `docs/architecture/events.md`, `docs/architecture/surfaces.md`, `TECDOC.md` (build_index, check_docs)
- [ ] T019 Regenerate the constitution (`constitution generate`), run `check_design`, `check_readme`, `manual_tests validate`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`; fill the Results of the plan envelope

**Checkpoint**: every check passes; ready for the pull request.

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6.
- T002 parallel with T001 after the files exist; T016 to T018 in parallel.
