---

description: "Tasks for the bounded detached-tape walk (v0.3.0, IMP-006)"
---

# Tasks: Bounded Detached-Tape Walk

**Input**: Design documents from `/specs/008-bounded-detached-walk/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/generations.md, quickstart.md

**Tests**: required (constitution IV.2). Tests are written with the code they cover and pass before a story is closed.

**Organization**: the kept generations (US2) come first, because the bound (US1) needs a cheap generation to save anything across calls.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [x] T001 Add `[[bench]] detached` to `crates/core/gedo-app/Cargo.toml` and `crates/core/gedo-app/benches/detached.rs` (deep history by `git fast-import`, research R1); record the numbers before the change in the plan envelope
- [x] T002 [P] Create `crates/core/gedo-app/src/events/generations.rs` (declared in `events/mod.rs`) and `crates/core/gedo-app/tests/generations.rs`; add `GENERATIONS_CACHE_NAMESPACE` to `events/records.rs`

**Checkpoint**: the workspace builds; `cargo xtask arch-check` passes.

---

## Phase 2: User Story 2 - Generations kept in the cache (Priority: P1)

**Goal**: each generation computed once per machine (research R4)

**Independent Test**: `cargo test -p gedo-app --test generations`

- [x] T003 [US2] Implement the record encoding and decoding (format 1, 8 bytes little-endian) and the batched write (1,000 per transaction) in `events/generations.rs`
- [x] T004 [US2] Make `EventLog::generation` read the cache before reading a commit's parents, check each cached value against its parents (R4), and write every computed generation at the end of the call in `events/mod.rs`
- [x] T005 [US2] Tests: reuse by a new event log (commits read counted), cleared cache, refused records, a failing cache, same clocks with and without a cache, in `tests/generations.rs`

**Checkpoint**: US2 acceptance scenarios pass.

---

## Phase 3: User Story 1 - Bounded search (Priority: P1) 🎯

**Goal**: the detached-tape search stops at the bound (research R3)

**Independent Test**: `cargo test -p gedo-app --test generations`

- [x] T006 [US1] Replace the walk of `detached_set` by the max-heap search with the pending set; leave missing indexed commits out as unreachable; log the debug event, in `events/reattach.rs`
- [x] T007 [US1] Scenario tests (deep linear history, merges, several tips, missing commit, all reachable, no index, `reattach_new` unchanged) with a counting object store, in `tests/generations.rs`
- [x] T008 [US1] Property (256 cases): random DAGs, bounded search equals a reference unbounded walk, cached generations equal uncached ones, in `tests/generations.rs`

**Checkpoint**: US1 acceptance scenarios pass; the property passes.

---

## Phase 4: Hardening and docs

- [x] T009 Run the `detached` benchmark at 10,000 and 100,000 commits (first call and cached calls) and record the results in the plan envelope; add the bench to the CI bench job (at its default depth of 100,000 commits)
- [x] T010 [P] Assert the `generation record refused` warning in `tests/event_tracing.rs`
- [x] T011 [P] ADR-0019; update `docs/architecture/events.md` and `docs/architecture/store.md`; regenerate the constitution and the TECDOC index
- [x] T012 Phase 6 checkers (`check_docs`, `check_readme`, `constitution check`, `manual_tests validate`), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`

**Checkpoint**: everything passes; the plan envelope has its results.

---

## Dependencies

- Phase 1 before everything; US2 (Phase 2) before US1 (Phase 3); Phase 4 last.
