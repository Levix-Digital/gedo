# Implementation Plan: History Re-attachment

**Branch**: `feat/005-history-reattachment` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/005-history-reattachment/spec.md`

## Summary

Re-attach Gedo data to commits rewritten by a rebase, an amend or a server-side squash or rebase merge, report detached tapes for the 90-day retention, and fill the event cache when new events arrive. `gedo-crdt` gains a pure `reattach` module: the semantic signature of a commit for the fast path of local rewrites (research R3), the fold of candidate events into entity states and the matching of a commit's changes against them (R4, R5), identity links for entities created on the branch, derived as reconstructed identity merges with the lower-id rule (R6), and the attachment rule of the clarification (R7). `gedo-app::events` gains the use cases `reattach_rewritten` (Git's rewritten pairs), `reattach_new` (fetched commits, candidates are the detached tapes), `detached` (the retention report, with the first-seen date kept in the symbol cache, R9) and `fill_cache` (IMP-005, R11); `events_of_commit` returns the remainder of a commit whose tapes do not account for all its changes (R8). Re-attachment writes only commit index entries; no stored format changes. ADR-0016 refines ADR-0008.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: no new dependency. `gedo-crdt` uses `blake3` and `postcard` (already there) for the semantic signature.

**Storage**: shared: commit index entries of feature 004 only (`commit-tapes`, unchanged format). Local: a `retention` record in the symbol cache (R9); the `events` cache namespace (ADR-0015).

**Testing**: `cargo test` (property-based tests of the matching in `gedo-crdt`; scripted rewrite histories on real repositories, crash injection, joined index entries, retention and cache tests in `gedo-app`), `insta` snapshots of the index entries, `criterion` benchmarks.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as before.

**Project Type**: Rust library crates; one example binary.

**Performance Goals**: re-attach one fetched commit under 100 ms p95 with 10,000 stored events of which 1,000 are detached (SC-004); fill the cache with 10,000 events under 2 s and the first filter after it under 500 ms (SC-005).

**Constraints**: pure logic without I/O in `gedo-crdt` (ADR-0001); only additive shared writes, content-addressed and idempotent; no `git` subprocess; no `unsafe` code; the `RefStore` port stays limited to `refs/gedo/*` (tips are inputs).

**Scale/Scope**: histories of thousands of commits, 10,000 to 100,000 stored events, up to 1,000 detached tapes per repository.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | Rules in `gedo-crdt::reattach` (signature, fold, match, attach); orchestration in `gedo-app::events::reattach`; `EventError` with context, `Missing` never treated as unmatched | Pass |
| II. Simplicity first | No new port, crate, collection or format; links reuse identity merges; one cache record for retention | Pass |
| III. Hexagonal | Only core crates change; adapters untouched; `cargo xtask arch-check` covers `crates/core/` | Pass |
| IV.2 Evidence | Property-based tests, scripted rewrites, crash injection, determinism snapshot on both platforms, benchmarks (R13) | Pass |
| IV.3 Documentation describes today | `events.md`, `store.md`, `core.md`, TECDOC edited in place; ADR-0016 | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security, data and rollback | Shared data read as untrusted (decoders of feature 004); additive index entries only; nothing removed | Pass |
| IV.9 No AI attribution | Commits by the repository identity; texts checked before every push | Pass |
| TECH_SPEC: re-attachment of one fetched commit under 100 ms (p95) | SC-004, benchmark `reattach` | Pass |
| TECH_SPEC Retention: 90 days, then `gedo gc` | Report only (FR-011); removal stays in FEAT-087 | Pass |
| ADR-0007: lower id kept on reconciliation | Links follow it (R6) | Pass |
| ADR-0008: patch-id fast path, match on target, operation and result hash | Semantic signature and entity-state matching instead (R3, R4) | Justified (Complexity Tracking), ADR-0016 |
| ADR-0012: identity results are local derived data | Links derived on demand, never stored | Pass |
| ADR-0014: re-attachment adds index entries, never touches tapes or events | FR-007 | Pass |
| ADR-0015: cache failures never fail an operation | `fill_cache`, retention record | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/005-history-reattachment/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/reattachment.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
.github/workflows/ci.yml            # bench job adds the reattach benchmark
crates/core/
├── gedo-crdt/
│   ├── src/lib.rs                  # exports reattach
│   ├── src/reattach.rs             # signature, entity states, matching, attachment rule, links
│   └── tests/reattach.rs           # property-based tests of the matching
└── gedo-app/
    ├── src/events/mod.rs           # CommitEvents::Published { tapes, remainder }
    ├── src/events/reattach.rs      # reattach_rewritten, reattach_new, detached, fill_cache
    ├── src/events/records.rs       # retention record encoding
    ├── tests/reattach.rs           # scripted rewrites, crashes, joins, retention, cache
    ├── tests/support/              # history builders for rewrites
    ├── tests/event_history.rs      # updated for the remainder
    ├── benches/reattach.rs
    └── examples/reattach.rs
docs/adr/0016-*.md
```

**Structure Decision**: the layout of ADR-0001; the event layer crate `gedo-crdt` and the event use case of feature 004 grow, no new crate.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| ADR-0008 names Git's patch-id as the fast path and matching on target, operation and result hash | A squash replaces sequences of operations by their net effect and gives new ids to entities created on the branch; a patch-id needs line diffs the core does not have and breaks on resolved conflicts (research R3, R4, R6) | Operation-by-operation matching fails on the common squash; patch-id adds a diff engine to the core for a weaker test; ADR-0016 records the refinement |

## Dev Cycle Protocol

### Backlog items and release

FEAT-065 and IMP-005 (`docs/BACKLOG.md`), release **v0.3.0 Event log and convergence** (`docs/ROADMAP.md`), second feature; the user chose this scope and assigned IMP-005 to v0.3.0 on 2026-10-30. Plan envelope: `docs/plans/gedo_reattachment.md`. Next: FEAT-066 (tree CRDT). Not in scope: removing detached data (FEAT-087), the hooks and the daemon that call these use cases (FEAT-073, FEAT-090, FEAT-091), IMP-001, IMP-003, IMP-004.

### Tests (quality-assurance)

- **Matching (US1, US2, US3)**: property-based tests in `gedo-crdt` over generated branches and their squash, rebase and split: every published tape attaches, results are independent of input order, links follow the lower-id rule, neutral entities and identity merges are accounted for, the known limit of research R7 measured.
- **Use cases (US1, US2, US3)**: scripted histories on temporary repositories: rebase without and with a resolved conflict, amend of the message and of code, interactive squash, fixup and split, server squash, server rebase merge, cherry-pick, unrelated detached branch, commit never published; idempotence, crash injection between index writes, two machines joining their entries; `insta` snapshot of the index entries, identical on both platforms (SC-001 to SC-003).
- **Remainder**: `events_of_commit` of a live-published commit has an empty remainder; of an amend with code, the extra changes; of a squash with a created entity, the link.
- **Retention (US4)**: report before and after the grace period, a custom period, after a later re-attachment, without a cache, with a refused record (SC-006).
- **Cache (US5)**: `fill_cache` with an empty, a partial, a full and a refusing cache; the first filter after it reads no event object.
- **Benchmarks**: `reattach` (fetched squash, pair, `fill_cache` and the first filter), reported by the non-blocking bench job; the reference machine run is asked of the user at the release candidate, as for feature 004.
- **Manual tests**: none added; there is no end-user command yet. The quickstart steps are developer checks.
- **Acceptance**: every acceptance scenario passes in CI on both platforms, the determinism snapshot is identical on both.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/events.md`: re-attachment (entry points, fast path, entity-state matching, links, attachment rule), detached tapes and the retention report, the remainder of a commit's events, cache filling; moved from "later" to what exists today.
- `docs/architecture/store.md`: the `retention` cache record; `docs/architecture/core.md`: `CommitEvents` and the `reattach` module.
- `TECDOC.md`: data flow of re-attachment after a rewrite or a fetch.
- New **ADR-0016** "Re-attachment by entity state with a semantic-signature fast path" (amends ADR-0008). Regenerate the constitution (accepted ADRs are sources).
- README: no change (no user-facing command yet).

### CI, release and observability (devops-release)

- `ci.yml`: the bench job runs `cargo bench -p gedo-app --bench reattach`. Required checks unchanged.
- Version: the workspace stays `0.1.0`; v0.1.0 to v0.3.0 wait for the user's release gate.
- Observability profile `library`: the `tracing` events of the contract (`tapes re-attached`, `re-attachment done`, `changes not matched`, `detached tapes`, `event cache filled`, and the cache warnings); `event_tracing.rs` asserts the info events.

### Security, migration and rollback

- No secrets. Tapes, events and index entries read for matching come from other machines and keep the checked decoding of feature 004 (size limits, versions, id checks).
- No migration: no stored format changes. The new `retention` cache record is local; earlier builds ignore it. `CommitEvents` changes shape inside the workspace only (no public API yet).
- Rollback: revert the pull request. Index entries already added by re-attachment remain valid feature-004 entries (a commit with a tape), which earlier builds read as published commits.
