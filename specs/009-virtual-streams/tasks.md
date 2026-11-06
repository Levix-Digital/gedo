---

description: "Tasks for virtual streams (v0.3.0, sixth feature)"
---

# Tasks: Virtual Streams

**Input**: Design documents from `/specs/009-virtual-streams/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/streams.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md). The model, the port and its adapters come first because every story needs them. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US5 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new modules, benchmark, example and test files

- [ ] T001 Create `crates/core/gedo-core/src/stream.rs`, `src/ports/stream.rs`, `src/memory/stream.rs`, `crates/core/gedo-crdt/src/streams.rs`, `crates/core/gedo-app/src/streams.rs` and `crates/adapters/gedo-store/src/streams.rs` (module docs, exported from each `lib.rs` and `mod.rs`)
- [ ] T002 [P] Add `[[bench]] streams` and `[[example]] streams` to `crates/core/gedo-app/Cargo.toml`; create `crates/core/gedo-app/tests/streams.rs` and `crates/adapters/gedo-store/tests/streams_gc.rs`

**Checkpoint**: the workspace builds; `cargo xtask arch-check` and `cargo deny check` pass.

---

## Phase 2: Foundational (model, port, adapters)

**Purpose**: streams can be stored and read on both adapters (research R2 to R4)

- [ ] T003 Implement `StreamName` (rule R3, `key()`), `Stream`, `WorktreeState`, `StreamRecordError` and the encode/decode of both records with their checks in `crates/core/gedo-core/src/stream.rs`; unit tests and a name property (generated valid and invalid names)
- [ ] T004 Define the `StreamStore` port in `crates/core/gedo-core/src/ports/stream.rs` and its contract tests in `src/contract_tests.rs` (create once, replace, remove with log and pin, names, one log writer, pins, state)
- [ ] T005 Implement `MemoryStreamStore` (logs as `MemoryWal`) in `crates/core/gedo-core/src/memory/stream.rs`; contract tests pass on it
- [ ] T006 Add the working tree id and the per-working-tree Gedo directory to `crates/adapters/gedo-store/src/repository.rs`; implement `FileStreamStore` (directories by key, exclusive create, atomic replace, `FileWal` per stream, state file, pins under `refs/gedo-local/` through gitoxide) in `crates/adapters/gedo-store/src/streams.rs`; run the contract tests on it in `tests/contract.rs`

**Checkpoint**: contract tests pass on both adapters.

---

## Phase 3: User Story 1 - Labeled streams found again (Priority: P1) 🎯 MVP

**Goal**: create, get and list streams by label and agent (research R5, R8)

**Independent Test**: `cargo test -p gedo-app --test streams create_ list_`

- [ ] T007 [US1] Implement `StreamFilter` (`matches`, `from_texts`) in `crates/core/gedo-crdt/src/streams.rs`; unit tests
- [ ] T008 [US1] Implement `Streams::new`, `with_cache`, `create`, `get`, `list`, `StreamError` and the `stream created` info event in `crates/core/gedo-app/src/streams.rs`
- [ ] T009 [US1] Tests per acceptance scenario of US1 in `crates/core/gedo-app/tests/streams.rs` (round trip across reopened stores, filters, every refusal stores nothing, concurrent creation of one name)

**Checkpoint**: US1 acceptance scenarios pass.

---

## Phase 4: User Story 2 - Inherited author and labels (Priority: P1)

**Goal**: events recorded in a stream carry its author kind and labels (research R6)

**Independent Test**: `cargo test -p gedo-app --test streams record_`

- [ ] T010 [US2] Implement `stream_context` and `recording_context` in `crates/core/gedo-crdt/src/streams.rs`; unit tests and the property that a stream's context carries its agent and labels
- [ ] T011 [US2] Implement `Streams::active`, `recording_context`, `record` (through `EventLog` over the stream's log, on the stream's base) and `pending` in `crates/core/gedo-app/src/streams.rs`
- [ ] T012 [US2] Tests per acceptance scenario of US2 in `tests/streams.rs` (authors, labels, clocks; outside behavior unchanged; environment ignored inside; two streams and the repository log kept apart)

**Checkpoint**: US2 acceptance scenarios pass.

---

## Phase 5: User Story 3 - Switching work (Priority: P1)

**Goal**: switch saves the current work and returns the target's (research R7)

**Independent Test**: `cargo test -p gedo-app --test streams switch_`

- [ ] T013 [US3] Implement `Streams::switch` with work commits and pins, and the `stream switched` info event, in `crates/core/gedo-app/src/streams.rs`
- [ ] T014 [US3] Tests per acceptance scenario of US3 in `tests/streams.rs` (stream to stream, outside to stream and back, unknown target, two working trees), and the property that every switch returns the tree last saved for its target
- [ ] T015 [P] [US3] `crates/adapters/gedo-store/tests/streams_gc.rs`: saved work, outside work and a stream base survive `git gc --prune=now`

**Checkpoint**: US3 acceptance scenarios pass.

---

## Phase 6: User Story 4 - Publishing a stream (Priority: P2)

**Goal**: the stream's commit with its events as tape, crash-safe (research R9)

**Independent Test**: `cargo test -p gedo-app --test streams publish_`

- [ ] T016 [US4] Implement `Streams::publish` (completing a publish in progress first, in every use case that opens a stream's log), `StreamPublish` and the `stream published` and `stream publish completed` events in `crates/core/gedo-app/src/streams.rs`
- [ ] T017 [US4] Tests per acceptance scenario of US4 in `tests/streams.rs` (commit, tape and `events_of_commit`; interruptions injected after the commit, the record and the tape; nothing to publish; a stream without a base publishes a root commit); published commit survives `git gc` in `streams_gc.rs`

**Checkpoint**: US4 acceptance scenarios pass.

---

## Phase 7: User Story 5 - Converge and discard (Priority: P3)

**Goal**: a stream as a convergence side; discard that keeps work by default (research R11)

**Independent Test**: `cargo test -p gedo-app --test streams converge_ discard_`

- [ ] T018 [US5] Implement `Streams::side`, `discard`, `Discarded`, the missing-active-stream warning and the `stream discarded` event in `crates/core/gedo-app/src/streams.rs`
- [ ] T019 [US5] Tests per acceptance scenario of US5 in `tests/streams.rs` (converge without and with conflicts, discard refused and forced, a discarded active stream seen from another working tree, the pin removed); `insta` snapshot of the scenario outcomes

**Checkpoint**: US5 acceptance scenarios pass.

---

## Phase 8: Hardening and docs

- [ ] T020 [P] `crates/core/gedo-app/benches/streams.rs` (SC-003) and the bench job in `.github/workflows/ci.yml`
- [ ] T021 [P] `crates/core/gedo-app/examples/streams.rs` (quickstart step 4); stream info events asserted in `tests/event_tracing.rs`
- [ ] T022 [P] ADR-0020 (`architecture-docs` new_adr), `docs/architecture/events.md`, `store.md`, `surfaces.md`, `TECDOC.md` (build_index, check_docs)
- [ ] T023 Regenerate the constitution (`constitution generate`), run `check_design`, `check_readme`, `manual_tests validate`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`; fill the Results of the plan envelope

**Checkpoint**: every check passes; ready for the pull request.

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 → Phases 4 and 5 (either order) → Phase 6 → Phase 7 → Phase 8.
- T002 parallel with T001 after the files exist; T005 and T006 after T004; T015 parallel with T014; T020 to T022 in parallel.
