---

description: "Tasks for the event model (v0.3.0, first feature)"
---

# Tasks: Event Model

**Input**: Design documents from `/specs/004-event-model/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/events.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md). US5 (clocks, P2) comes before US3 and US4 because recording and reconstruction need clocks. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US6 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new crate

- [x] T001 Add `crates/core/gedo-crdt` to the workspace members and `gedo-crdt` to `[workspace.dependencies]` in `Cargo.toml`; create the crate (`Cargo.toml` depending on `gedo-core`, `blake3`, `postcard`, `serde`, `thiserror`; dev-dependencies `proptest`, `insta`; `src/lib.rs` with crate docs); add `gedo-crdt` to `gedo-app`'s dependencies
- [x] T002 Verify the setup: `cargo build --workspace`, `cargo deny check` and `cargo xtask arch-check` pass, and `gedo-crdt` is checked as a core crate

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the event model and the two port additions every story uses

**⚠️ CRITICAL**: no user story starts before this phase is complete

- [x] T003 [P] Create `crates/core/gedo-core/src/event.rs` (data-model): `EventId`, `TapeId` (16 bytes, UUID display, 32-hex entry names), `Clock` (ordered), `AuthorFingerprint`, `AgentId`, `AuthorKind`, `Author`, `LabelKey`, `LabelValue`, `Labels` (limits, `for_recording`), `SemanticOp` with `target()`, `Origin`, `Event` with `check()` (target and `result_hash` invariants), `Tape` with `check()` (1 to 65,536 unique ids); serde derives and doc comments; export from `lib.rs`; unit tests of the validations
- [x] T004 [P] Add `author_email` to `CommitInfo` in `crates/core/gedo-core/src/ports/object.rs`; fill it in `crates/adapters/gedo-store/src/objects.rs` and in `crates/core/gedo-core/src/memory/objects.rs` (with a way to write test commits under a chosen author); extend the object store contract tests in `contract_tests.rs`
- [x] T005 [P] Add `Collection::entries_with_prefix` in `crates/core/gedo-core/src/collection.rs` (reads one fan-out directory) with contract tests on both adapters

**Checkpoint**: model and ports compile and pass on both platforms.

---

## Phase 3: User Story 1 - Content-derived ids (Priority: P1)

**Goal**: equal content, equal id; any change, a different id; stable encoding.

**Independent Test**: build events from scripted deltas, derive ids, round-trip and snapshot the encoded bytes.

- [x] T006 [US1] Implement `event_id` and `tape_id` in `crates/core/gedo-crdt/src/id.rs` (research R2, R8) and `Event::id`/`Tape::id` through them
- [x] T007 [P] [US1] Property tests in `crates/core/gedo-crdt/tests/properties.rs`: ids equal for equal content, different when any field changes, UUID version 8 bits; snapshot of encoded sample events and their ids

**Checkpoint**: ids and encoding fixed (milestone M2).

---

## Phase 4: User Story 2 - Author kind and labels (Priority: P1)

**Goal**: person or agent authors, opaque labels, text forms and filters.

**Independent Test**: build contexts from values and texts, record, read back and filter.

- [x] T008 [P] [US2] Implement `fingerprint` in `crates/core/gedo-crdt/src/author.rs` (research R4) with tests for trimming and case
- [x] T009 [P] [US2] Implement `Labels::parse`, the escapes, `LabelError` with positions and `RecordingContext` (`person`, `agent`, `from_texts`) in `crates/core/gedo-crdt/src/labels.rs` (research R6); unit and property tests (round trip of formatted labels, every refusal)
- [x] T010 [P] [US2] Implement `EventFilter::matches` in `crates/core/gedo-crdt/src/filter.rs` (research R12) with tests

**Checkpoint**: authors and labels (milestone M3).

---

## Phase 5: User Story 5 - Clocks consistent with history (Priority: P2)

**Goal**: generations, live and reconstructed clocks, one total order.

**Independent Test**: property tests over generated commit graphs.

- [x] T011 [US5] Implement `generation`, `live_clock` and `sort_events` in `crates/core/gedo-crdt/src/clock.rs` (research R3)
- [x] T012 [P] [US5] Property tests: generations increase along every parent edge; live clocks above their base's; sort independent of input order and total
- [x] T013 [US5] Implement generations in `EventLog::generation` in `crates/core/gedo-app/src/events/mod.rs` (kept in memory), with tests on generated histories (roots, merges, several roots)

**Checkpoint**: clocks (milestone M4).

---

## Phase 6: User Story 3 - Record and publish (Priority: P1)

**Goal**: live events through the WAL into tapes, events and the index, crash-safe.

**Independent Test**: record, publish, inject crashes, join two machines.

- [x] T014 [US3] Implement `WalRecord` and the checked encoding and decoding of event, tape and index records in `crates/core/gedo-app/src/events/records.rs` (data-model; id and invariant checks)
- [x] T015 [US3] Implement `EventLog::open`, `record` and `publish` in `crates/core/gedo-app/src/events/mod.rs` (research R11), with `EventError` and the `tracing` events of research R14
- [x] T016 [US3] Implement `tapes_of`, `tape` and `event` (published reads) in `crates/core/gedo-app/src/events/mod.rs`
- [x] T017 [US3] Tests in `crates/core/gedo-app/tests/events.rs`: publish, empty log, events appended during a publish, oversize refusal keeps the log, crash injection at every step with re-run equality (SC-003), two stores publishing then joining chains, the same event published twice stored once, a missing tape reported

**Checkpoint**: live path (milestone M5).

---

## Phase 7: User Story 4 - Reconstruction (Priority: P1)

**Goal**: the same events for commits made without Gedo on every machine.

**Independent Test**: reconstruct generated and real histories in several orders and compare with snapshots.

- [x] T018 [US4] Implement `reconstructed_events` in `crates/core/gedo-crdt/src/reconstruct.rs` (research R10: deltas then merges, ids as recorded, commit author, no labels)
- [x] T019 [US4] Implement `EventLog::events_of_commit` (published or reconstructed, `NotIndexed`) and `EventLog::filter` in `crates/core/gedo-app/src/events/mod.rs`
- [x] T020 [US4] Tests in `crates/core/gedo-app/tests/event_history.rs`: generated histories reconstructed in 3 orders give identical events; ids not resolved through later aliases; a commit with no Rust change; a published commit is not reconstructed; snapshot over selected commits of this repository (SC-001)

**Checkpoint**: reconstruction (milestone M6).

---

## Phase 8: User Story 6 - Safe reading and performance (Priority: P2)

- [x] T021 [P] [US6] Tests of malformed, truncated, oversized, wrong-version and id-mismatch records in `crates/core/gedo-app/src/events/records.rs`
- [x] T022 [P] [US6] Add `fuzz/fuzz_targets/decode_event_record.rs` (events, tapes, index entries, WAL records) and register it in `.github/workflows/fuzz.yml`
- [x] T023 [P] [US6] Add `crates/core/gedo-app/benches/events.rs` (record, publish 1,000 events, publish into a chain of 100,000 events, reconstruct one commit, filter 10,000 events) and run it from the bench job in `.github/workflows/ci.yml`
- [x] T024 [P] [US6] Add `crates/core/gedo-app/examples/events.rs` (quickstart step 3)

---

## Phase 9: Polish & Cross-Cutting Concerns

- [x] T025 Documentation: `docs/architecture/events.md`, `core.md`, `store.md`, `TECDOC.md`, TECH_SPEC data table and `uuid` row; new `docs/adr/0014-event-format-content-derived-ids-history-clocks-and-three-co.md`; run `build_index`, `check_docs`, `check_readme`, `constitution.py generate` and `check`
- [x] T026 Walk through `quickstart.md` steps 1 to 5 and fix any drift between the guide and the code
- [ ] T027 Run the benchmarks on Linux and on the Windows reference machine (with the user's go) and record p50, p95 and p99 in `docs/plans/gedo_event_model.md` (SC-004 to SC-006); run the fuzz target for 10 minutes (SC-007)

---

## Dependencies & Execution Order

- **Setup (T001–T002)** → **Foundational (T003–T005)** → stories.
- **US1 (T006–T007)** first; **US2 (T008–T010)** and **US5 (T011–T013)** after US1, in parallel; **US3 (T014–T017)** needs US1, US2 and US5; **US4 (T018–T020)** needs US1, US5 and the reads of T016; **US6 (T021–T024)** after US3 and US4.
- **Polish (T025–T027)** after all stories.

### Parallel Opportunities

- Foundational: T003, T004 and T005 in parallel.
- US2's T008 to T010 alongside US5's T011 and T012.
- US6's T021 to T024 in parallel.

## Implementation Strategy

1. Setup and Foundational, then **US1** (ids) and validate it alone.
2. US2 and US5 complete the event content; US3 delivers the live path; US4 the reconstructed one.
3. US6 hardens and measures.
4. Each checkpoint maps to a plan milestone: M1 Setup and event model, M2 Event ids, M3 Authors and labels, M4 Clocks, M5 Record and publish, M6 Reconstruction, M7 Hardening and acceptance.
