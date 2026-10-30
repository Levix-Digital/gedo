---

description: "Tasks for history re-attachment (v0.3.0, second feature)"
---

# Tasks: History Re-attachment

**Input**: Design documents from `/specs/005-history-reattachment/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/reattachment.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md). The matching rules (foundational) come first because US1, US2 and US3 all use them. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US5 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new modules and test support

- [x] T001 Create `crates/core/gedo-crdt/src/reattach.rs` (module docs, exported from `lib.rs`) and `crates/core/gedo-app/src/events/reattach.rs` (an `impl` block of `EventLog`, declared in `events/mod.rs`)
- [x] T002 [P] Add history builders for rewrites to `crates/core/gedo-app/tests/support/` (commit a branch, rebase it onto a new base, squash it onto a base as a server does, replay it as a rebase merge, amend, split, cherry-pick), on real repositories through `gedo-store`

**Checkpoint**: the workspace builds; `cargo xtask arch-check` passes.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the pure matching every story uses (research R3 to R7)

**⚠️ CRITICAL**: no user story starts before this phase is complete

- [x] T003 Implement `semantic_signature` in `crates/core/gedo-crdt/src/reattach.rs` (research R3): canonical order, domain-separated `BLAKE3`; unit tests (equal for equal changes, different for any changed delta or hash, independent of the base)
- [x] T004 Implement the fold of candidate events into entity states (research R4, R5): last result hash, kind and qualified path through creations, moves and renames, neutral detection (create then tombstone, change then revert); unit tests per case
- [x] T005 Implement `match_commit` (research R4 to R7): matches and superseded events per change, identity links for created entities (kind, qualified path, final hash; lowest id on ties; lower id kept), identity merges accounted for, the attachment rule with and without a pair, `Reattachment` sorted output
- [x] T006 [P] Property-based tests in `crates/core/gedo-crdt/tests/reattach.rs`: generated branches (creations, edits, renames, moves, tombstones, reverts) and their squash attach every tape; results independent of candidate order; a shuffled unrelated branch does not change the result except in the known limit of research R7 (measured and asserted bounded); links follow the lower-id rule

**Checkpoint**: matching rules complete and tested (milestone M2).

---

## Phase 3: User Story 1 - Local rebase and amend (Priority: P1)

**Goal**: Git's rewritten pairs keep their live events.

**Independent Test**: publish a branch, rebase or amend it, re-attach the pairs, compare the events of each new commit.

- [x] T007 [US1] Implement `EventLog::reattach_rewritten` in `crates/core/gedo-app/src/events/reattach.rs`: skip commits with index entries, fast path on equal signatures (old commit indexed), otherwise match against the old commit's tapes, write index entries (content-addressed, one collection write per call), `ReattachReport`, logs of the contract
- [x] T008 [US1] Change `CommitEvents::Published` to `{ tapes, remainder }` in `crates/core/gedo-app/src/events/mod.rs` and compute the remainder with `match_commit` (research R8); update `tests/event_history.rs`, the `events` example and bench
- [x] T009 [P] [US1] Tests in `crates/core/gedo-app/tests/reattach.rs`: rebase without and with a resolved conflict, amend of the message, amend with code (remainder holds the extra change), interactive squash, fixup and split, re-run idempotence, two machines joining their entries

**Checkpoint**: local rewrites re-attach (milestone M3).

---

## Phase 4: User Story 2 - Server squash and rebase merge (Priority: P1)

**Goal**: fetched commits get the tapes of the branch they replace.

**Independent Test**: publish a branch, squash it onto main as a server does, re-attach the new commit without pairs.

- [x] T010 [US2] Implement the detached set in `crates/core/gedo-app/src/events/reattach.rs` (research R9): list the commit index, walk the history from the tips once per call with the generation cut-off, group detached tapes' events by target
- [x] T011 [US2] Implement `EventLog::reattach_new`: commits in generation then id order, candidates are the detached tapes, attachment rule without a pair, index entries, report and logs
- [x] T012 [P] [US2] Tests in `crates/core/gedo-app/tests/reattach.rs`: server squash (entities created and edited on the branch, with the link in the remainder), revert on the branch, server rebase merge, cherry-pick, unrelated detached branch stays detached, commit never published reconstructs as before, a missing tape is an error

**Checkpoint**: squashes re-attach (milestone M4).

---

## Phase 5: User Story 3 - Deterministic, additive only (Priority: P1)

**Goal**: every machine reaches the same index; nothing stored changes.

**Independent Test**: re-attach scripted histories in different orders and on both platforms, compare the entries.

- [x] T013 [US3] Snapshot with `insta` the index entries of every scripted history of T009 and T012, processed in 3 orders, in `crates/core/gedo-app/tests/reattach.rs` (SC-002)
- [x] T014 [P] [US3] Property and crash tests: every event, tape and earlier index entry unchanged after a run; a crash injected before and after the index write, re-run equal to an uninterrupted run (SC-003)

**Checkpoint**: determinism fixed (milestone M5).

---

## Phase 6: User Story 4 - Retention report (Priority: P2)

**Goal**: detached tapes are listed with their date and eligibility.

**Independent Test**: rewrite tapes away, advance `now`, report; re-attach, report again.

- [x] T015 [US4] Implement the retention record in `crates/core/gedo-app/src/events/records.rs` (namespace `retention`, versioned, checked decoding) and `EventLog::detached` (first-seen date, grace period, eligibility; record rewritten in one transaction; a re-attachment that attaches a listed tape removes it; no cache: dated `now`)
- [x] T016 [P] [US4] Tests: before and after 90 days, a custom period, after a later re-attachment, without a cache, with a refused record (SC-006)

**Checkpoint**: retention reported (milestone M6).

---

## Phase 7: User Story 5 - Cache filled on new data (Priority: P2)

**Goal**: the first filter after a fetch is as fast as later ones (IMP-005).

**Independent Test**: fetch 10,000 events into an empty cache, fill, filter.

- [x] T017 [US5] Implement `EventLog::fill_cache` in `crates/core/gedo-app/src/events/reattach.rs` (research R11): per-id lookup, batched reads and one cache transaction per batch, `CacheFill`, warnings on cache failures
- [x] T018 [P] [US5] Tests: empty, partial, full and refusing caches; after filling, a filter reads no event object (counted through the memory object store)

**Checkpoint**: IMP-005 done (milestone M7).

---

## Phase 8: Polish & Cross-Cutting Concerns

- [x] T019 [P] Benchmark `crates/core/gedo-app/benches/reattach.rs` (fetched squash with 10,000 stored and 1,000 detached events, pair path, `fill_cache` and the first filter) and add it to the bench job of `.github/workflows/ci.yml` (SC-004, SC-005)
- [x] T020 [P] Example `crates/core/gedo-app/examples/reattach.rs` (quickstart step 3) and the tracing assertions in `tests/event_tracing.rs`
- [x] T021 ADR-0016 (amends ADR-0008) with the `architecture-docs` script; edit `docs/architecture/events.md`, `store.md`, `core.md` and `TECDOC.md` in place; regenerate the constitution
- [x] T022 Run the protocol checks: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo xtask arch-check`, the docs and constitution checkers; run the quickstart

**Checkpoint**: ready for the pull request (milestone M8).

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phases 3 and 4 (4 uses T008's remainder) → Phase 5 → Phases 6 and 7 (independent) → Phase 8.
- Within a phase, [P] tasks touch different files and can run together.

## Milestones (plan envelope)

M1 Setup (T001, T002), M2 Matching rules (T003 to T006), M3 Local rewrites (T007 to T009), M4 Server squash (T010 to T012), M5 Determinism (T013, T014), M6 Retention (T015, T016), M7 Cache filling (T017, T018), M8 Hardening and docs (T019 to T022).
