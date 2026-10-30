# Implementation Plan: Tree CRDT

**Branch**: `feat/006-tree-crdt` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/006-tree-crdt/spec.md`

## Summary

Converge concurrent work deterministically. `gedo-crdt` gains a pure `tree` module: the state of the code as entities with five parts (presence, place, name, signature, body, research R3), the fold of a side's events into that state with duplicates removed, aliases resolved and the total order of ADR-0014 (R4), and a three-way convergence per part of a base and any number of sides (R2, R5): a part changed by one side, or by sides that agree, takes that value; distinct values give a conflict that keeps the base value and names the sides and events. Moves, renames and edits of different parts commute; removal against change conflicts; members of a container are a set because membership is each member's place (R6); entities added on several sides at the same qualified path are one entity (identities linked) when their content is equal and a conflict otherwise (R7). `gedo-app::events` gains `converge(base, sides)`, which builds the base state from the identity results of the files the sides changed and folds the events of each side's commits (live where tapes are attached) plus optional live events (R9). Nothing is stored; no format changes. ADR-0017 records the model and amends ADR-0009.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: no new dependency. `gedo-crdt` adds `criterion` as a dev-dependency (already in the workspace) for its benchmark.

**Storage**: none written. Reads the collections of ADR-0014 through `events_of_commit` and the identity results through `IdentityIndex`.

**Testing**: `cargo test` (property-based tests of the fold and the convergence in `gedo-crdt`; scenario tests per acceptance scenario; scripted branches on real repositories in `gedo-app` with an `insta` snapshot of the convergence), `criterion` benchmarks.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as before.

**Project Type**: Rust library crates; one example binary.

**Performance Goals**: two sides of 10,000 events over 10,000 entities under 50 ms p95 (SC-004); two branches of 100 commits from their tips under 500 ms p95 (SC-005).

**Constraints**: pure logic without I/O in `gedo-crdt` (ADR-0001); no write of any kind; no `git` subprocess; no `unsafe` code; `BTreeMap` and sorted outputs only, so results are identical on every platform.

**Scale/Scope**: bases of up to 100,000 entities (the reference repository), sides of up to tens of thousands of events, two to a handful of sides.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | Rules in `gedo-crdt::tree` (state, fold, convergence); orchestration in `gedo-app::events::converge`; `TreeError` and `EventError` with context; unknown entities never dropped | Pass |
| II. Simplicity first | No new port, crate, collection or format; parts are the hashes and names feature 003 already tracks; one rule (three-way per part) for every caller | Pass |
| III. Hexagonal | Only core crates change; adapters untouched; `cargo xtask arch-check` covers `crates/core/` | Pass |
| IV.2 Evidence | Property-based tests, scenario tests per acceptance scenario, determinism snapshot on both platforms, benchmarks (R10, R11) | Pass |
| IV.3 Documentation describes today | `events.md`, `core.md`, TECDOC edited in place; ADR-0017 | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security, data and rollback | Shared data read through the checked decoders of feature 004; nothing written | Pass |
| IV.9 No AI attribution | Commits by the repository identity; texts checked before every push | Pass |
| TECH_SPEC: merge driver on one file of 2,000 lines under 200 ms (p95) | SC-004 keeps the convergence under 50 ms of it, benchmark `tree` | Pass |
| TECH_SPEC: merge safety, any doubt falls back to Git's textual merge | Conflicts are reported per part, never resolved silently (R5, R6) | Pass |
| ADR-0007: lower id kept on reconciliation | Links of entities added on both sides (R7) | Pass |
| ADR-0009: members as a set, move and edit commute, an event on both sides applied once, never hide a conflict | R2, R5, R6; removal against change and added-on-both-sides made precise | Pass, ADR-0017 amends it |
| ADR-0012: identity results are local derived data | Links derived on demand, never stored | Pass |
| ADR-0014: events and their total order; format frozen | Fold in (clock, id) order; no field added | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/006-tree-crdt/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/convergence.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
.github/workflows/ci.yml            # bench job adds the tree and converge benchmarks
crates/core/
├── gedo-crdt/
│   ├── Cargo.toml                  # criterion dev-dependency, [[bench]] tree
│   ├── src/lib.rs                  # exports tree
│   ├── src/tree.rs                 # parts, entity and tree state, fold, convergence, conflicts
│   ├── tests/tree.rs               # property-based and scenario tests
│   └── benches/tree.rs
└── gedo-app/
    ├── src/events/mod.rs           # declares converge, EventError::Tree
    ├── src/events/converge.rs      # Side, ConvergeReport, EventLog::converge
    ├── src/index/mod.rs            # changed_files(commit) for the base state of R9
    ├── tests/converge.rs           # scripted branches, graft, live events, snapshot
    ├── tests/event_tracing.rs      # asserts "sides converged"
    ├── benches/converge.rs
    └── examples/converge.rs
docs/adr/0017-*.md
```

**Structure Decision**: the layout of ADR-0001; the event layer crate `gedo-crdt` and the event use cases of features 004 and 005 grow, no new crate.

## Complexity Tracking

No violation to justify.

## Dev Cycle Protocol

### Backlog items and release

FEAT-066 (`docs/BACKLOG.md`), release **v0.3.0 Event log and convergence** (`docs/ROADMAP.md`), third feature. Plan envelope: `docs/plans/gedo_tree_crdt.md`. Next in the release: FEAT-067 (lanes), FEAT-068 (streams), FEAT-069 (sequencer). Not in scope: rendering to source and the textual fallback (FEAT-070), the radar (FEAT-074), recording conflicts in the inbox (v0.8.0), IMP-006 (still to be triaged by the user).

### Tests (quality-assurance)

- **Rules (US1 to US4)**: property-based tests in `gedo-crdt/tests/tree.rs` (R11): arrival order, duplicates, events shared between sides, permuted sides, convergence with itself and with an empty side, one side at a time against all at once without conflicts, no lost change; at least 256 cases each (SC-002, SC-003).
- **Scenarios (US1 to US4)**: one test per acceptance scenario with the expected state and conflicts: additions and removals in the same container, removal on both sides, the same entity added on both sides, move against edit, rename against move, a change on one side in both orders, the same change on both sides, two bodies, removal against change, removed container against new member, different entities added on both sides, unrelated changes beside a conflict, three sides, identity merges, neutral entities, unknown entity (SC-001).
- **Use case (US5)**: scripted repositories: two branches with and without published events, a graft (the same change on both), live events on top of one side, a side or base not indexed; `insta` snapshot of the convergence, identical on both platforms.
- **Benchmarks**: `tree` (SC-004) and `converge` (SC-005), reported by the non-blocking bench job; the reference machine run is asked of the user at the release candidate, as for features 004 and 005.
- **Manual tests**: none added; there is no end-user command yet. The quickstart steps are developer checks.
- **Acceptance**: every acceptance scenario passes in CI on both platforms, the determinism snapshot is identical on both.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/events.md`: the tree CRDT (parts, fold, convergence per part, what commutes and what conflicts, links, conflicts) and the `converge` use case; moved from "later" to what exists today.
- `docs/architecture/core.md`: the `tree` module and `converge`.
- `TECDOC.md`: data flow of a convergence.
- New **ADR-0017** "Tree CRDT as a three-way convergence of entity parts" (amends ADR-0009). Regenerate the constitution (accepted ADRs are sources; the regeneration also picks up the protocol's updated wording of principle 9).
- README: no change (no user-facing command yet).

### CI, release and observability (devops-release)

- `ci.yml`: the bench job runs `cargo bench -p gedo-crdt --bench tree` and `cargo bench -p gedo-app --bench converge`. Required checks unchanged.
- Version: the workspace stays `0.1.0`; v0.1.0 to v0.3.0 wait for the user's release gate.
- Observability profile `library`: the `sides converged` info event and the debug event of the contract; `event_tracing.rs` asserts the info event.

### Security, migration and rollback

- No secrets. Events and identity results read for the convergence come from other machines and keep the checked decoding of features 003 and 004 (size limits, versions, id checks); a malformed side gives an error, never a panic.
- No migration: nothing stored, no format changes.
- Rollback: revert the pull request; nothing persists.
