# Implementation Plan: Bounded Detached-Tape Walk

**Branch**: `feat/008-bounded-detached-walk` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/008-bounded-detached-walk/spec.md`

## Summary

Stop the detached-tape search at the generation of the lowest indexed commit still unseen. `EventLog::generation` keeps every generation it computes in a new `generations` namespace of the symbol cache and reads it back, checked against the parents' generations, before walking the history (research R2, R4). The search behind `detached` and `reattach_new` visits commits by descending generation with a max-heap and stops when the highest commit still to visit is below the lowest unseen indexed commit; indexed commits missing from the object database are unreachable (R3). No port, format, collection or command changes. ADR-0019 records the decision.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: none new.

**Storage**: the symbol cache (`SymbolCache` port, redb adapter) gains the `generations` namespace. No change in `refs/gedo/*`.

**Testing**: `cargo test` (scenario tests and a proptest property on the in-memory store, cache tests, the Git store adapter in the benchmark), `criterion` benchmark `detached`.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as before.

**Project Type**: Rust library crates.

**Performance Goals**: SC-001 (p95 under 50 ms with 100,000 commits below the Gedo data, generations cached), SC-002 (first call at most twice one call before the change).

**Constraints**: pure core without I/O libraries (ADR-0001); no `git` subprocess outside tests and benches; no `unsafe`; results identical on every platform.

**Scale/Scope**: histories up to 1,000,000 commits; commit indexes of thousands of entries.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | The search is one function of `events::reattach`; generations stay in `events::mod` with the cache reads and writes in one small module `events::generations`; cache failures are warnings, store failures errors with context | Pass |
| II. Simplicity first | No new port, crate or format; the cache already holds local derived data; the commit-graph is left for later (R2) | Pass |
| III. Hexagonal | Only `gedo-app` changes; `cargo xtask arch-check` | Pass |
| IV.2 Evidence | Scenario tests, a property against a reference walk, cache tests, a benchmark before and after (R1, R6) | Pass |
| IV.3 Documentation describes today | `events.md`, `store.md` edited in place; ADR-0019 | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security, data and rollback | Only local cache data is added; checked on read; pushed data unchanged | Pass |
| IV.9 No AI attribution | Commits by the repository identity; texts checked before every push | Pass |
| ADR-0012, ADR-0015: local derived data in the cache, failures are warnings | Same rules for `generations` | Pass |
| ADR-0014: generations and clocks | Same definition; the check on read keeps clocks right (R4) | Pass |
| ADR-0016: detached tapes and re-attachment | Same results; only the walk is bounded | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/008-bounded-detached-walk/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/generations.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
.github/workflows/ci.yml                 # bench job adds the detached benchmark
crates/core/gedo-app/
├── Cargo.toml                           # [[bench]] detached
├── src/events/mod.rs                    # generation reads and writes the cache
├── src/events/generations.rs            # record encoding, checked read, batched write
├── src/events/records.rs                # GENERATIONS_CACHE_NAMESPACE
├── src/events/reattach.rs               # bounded search
├── tests/generations.rs                 # cache reuse, refusal, failures, property
├── tests/reattach.rs                    # bound scenarios, missing commits, property against the reference walk
└── benches/detached.rs
docs/adr/0019-*.md
```

**Structure Decision**: the layout of ADR-0001; only the event use cases change.

## Complexity Tracking

No violation to justify.

## Dev Cycle Protocol

### Backlog items and release

IMP-006 (`docs/BACKLOG.md`), release **v0.3.0 Event log and convergence** (`docs/ROADMAP.md`). Plan envelope: `docs/plans/gedo_bounded_detached_tape_walk.md`. Left in the release after it: FEAT-068 (virtual streams), FEAT-069 (sequencer). Not in scope: reading Git's commit-graph, shallow clones, `gedo gc`.

### Tests (quality-assurance)

- **US1**: scenario tests in `tests/reattach.rs`: deep linear history with one detached tape (commits read counted through a counting object store, none below the bound), merges, several tips, an indexed commit missing from the store, every commit reachable, no index; `reattach_new` attaches the same tapes as before.
- **US2**: `tests/generations.rs`: generations reused by a new event log on the same cache (commits read counted), cleared cache, refused records (length, format byte, wrong value), a cache that fails every call, the same clocks for recorded events with and without a cache.
- **Property** (SC-003, 256 cases): random DAGs with random indexed, missing and tip commits: the bounded search equals a reference unbounded walk; cached generations equal uncached ones.
- **Benchmark**: `detached` (SC-001, SC-002), reported by the non-blocking bench job; the reference machine run is asked of the user.
- **Manual tests**: none; there is no end-user command.
- **Acceptance**: every acceptance scenario passes in CI on both platforms.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/events.md`: the bounded search and the kept generations.
- `docs/architecture/store.md`: the `generations` namespace; the sentence that generations are kept in memory only is replaced.
- New **ADR-0019** "Commit generations kept in the symbol cache and a generation-bounded detached-tape search". Regenerate the constitution.
- README and `TECDOC.md`: no change of content beyond the regenerated index if needed.

### CI, release and observability (devops-release)

- `ci.yml`: the bench job runs `cargo bench -p gedo-app --bench detached -- --quick` with a depth that fits the runner (10,000 commits). Required checks unchanged.
- Version: the workspace stays `0.1.0`; releases wait for the user's release gate.
- Observability profile `library`: `detached-tape search done` (debug: commits read, bound reached); `generation record refused`, `could not write generations` (warn). `event_tracing.rs` asserts the refusal warning.

### Security, migration and rollback

- No secrets. Cache records are decoded with length and format checks and verified against the parents (R4); a bad record is refused, never trusted.
- No migration: a new cache namespace, filled on first use; older builds ignore it.
- Rollback: revert the pull request; the `generations` namespace is then unused and cleared with the cache.
