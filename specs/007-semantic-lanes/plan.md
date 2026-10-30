# Implementation Plan: Semantic Lanes and Split Publish

**Branch**: `feat/007-semantic-lanes` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/007-semantic-lanes/spec.md`

## Summary

Publish mixed work as a refactor commit and a feature commit. `gedo-crdt` gains a pure `lanes` module that gives each change of a publish (each delta, one part of one entity, ADR-0017) a lane: moves and renames are refactor, and so are signature and body changes, imports and module declarations whose only difference is the references a move or rename forces (follow-ups, research R3, R4); everything else is feature (R2). A pure `split` module builds the refactor version of every changed file from the base and final sources, their tokens and identity maps: equal and follow-up text from the final version, moved entities as blocks rebuilt recursively, every other change kept as in the base (R5), and generates the refactor message (R8). `gedo-app::events` gains `publish_split(request)`: it writes and indexes the single commit (the fallback and the source of the deltas), builds and writes the refactor and feature commits, verifies them against the lanes with the identity index, assigns each event of the log to a lane and publishes one tape per commit with one truncation of the log (R6, R7). When the work has one lane, or the verification fails, the single commit is published with the reason. No branch is moved; nothing new is stored; no format changes. ADR-0018 records the decision.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: `similar` 2.7 (Myers diff over token slices; MIT or Apache-2.0; already in `Cargo.lock` through `insta`) becomes a dependency of `gedo-crdt`. No other new dependency.

**Storage**: Git objects (blobs, trees, commits) through `ObjectStore`, the collections of ADR-0014 through `publish`, identity results in the symbol cache through `IdentityIndex`. No new collection or format.

**Testing**: `cargo test` (unit and property-based tests of the lanes and the builder in `gedo-crdt`; scripted repositories in `gedo-app` with an `insta` snapshot of the outcome), `criterion` benchmark.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as before.

**Project Type**: Rust library crates; one example binary.

**Performance Goals**: split-publish 100 changed files and 1,000 events under 1 s p95 on the reference machine (SC-003, the `gedo publish` budget).

**Constraints**: pure logic without I/O in `gedo-crdt` (ADR-0001); no `git` subprocess; no reference written outside `refs/gedo/*`; no `unsafe` code; sorted maps only, so results are identical on every platform.

**Scale/Scope**: publishes of up to thousands of files (a large rename) and 65,536 events (the tape limit).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | Lanes and builder in `gedo-crdt` (`lanes`, `split`); orchestration in `gedo-app::events::split`; `BuildError`, `NotSplit` and `EventError` with context | Pass |
| II. Simplicity first | No new port, crate, collection or format; the identity index is reused as the verifier; one rule for files and moved entities (R5) | Pass |
| III. Hexagonal | Only core crates change; `similar` is a pure library, not in the forbidden list; `cargo xtask arch-check` | Pass |
| IV.2 Evidence | Unit and property tests, scenario tests per acceptance scenario and per fallback reason, snapshot on both platforms, benchmark (R10) | Pass |
| IV.3 Documentation describes today | `events.md`, `surfaces.md`, TECDOC edited in place; ADR-0018 | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security, data and rollback | Writes only content-addressed objects and `refs/gedo/*` through `publish`'s steps; never moves a branch; the log kept on any failure before the index entries | Pass |
| IV.9 No AI attribution | Commits by the repository identity; texts checked before every push | Pass |
| TECH_SPEC: `gedo publish` under 1 s | SC-003, benchmark `split` | Pass |
| ADR-0007: identities derived from content and history | Created entities get the same ids through the refactor commit; verification compares parts by id | Pass |
| ADR-0012: identity results are local derived data | The commits written are indexed into the local cache only | Pass |
| ADR-0014: events, tapes, index order, idempotence | One tape per commit, stored in the ADR's order, log truncated once | Pass |
| ADR-0017: parts of an entity | Lanes are per part | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/007-semantic-lanes/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/split.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
.github/workflows/ci.yml            # bench job adds the split benchmark
Cargo.toml                          # similar in [workspace.dependencies]
crates/core/
├── gedo-crdt/
│   ├── Cargo.toml                  # similar
│   ├── src/lib.rs                  # exports lanes, split
│   ├── src/lanes.rs                # Lane, FollowUps, LanePlan, classify, follow-up test, event lanes
│   ├── src/split.rs                # units, blocks, build of refactor files, refactor message
│   └── tests/lanes.rs              # rules, follow-ups, builder scenarios and properties
└── gedo-app/
    ├── Cargo.toml                  # [[bench]] split
    ├── src/events/mod.rs           # declares split; publish shares its storing steps
    ├── src/events/split.rs         # SplitRequest, SplitOutcome, EventLog::publish_split
    ├── tests/split.rs              # scripted work, fallbacks, snapshot
    ├── tests/event_tracing.rs      # asserts "split published"
    ├── benches/split.rs
    └── examples/split.rs
docs/adr/0018-*.md
```

**Structure Decision**: the layout of ADR-0001; the event layer crate `gedo-crdt` and the event use cases grow, no new crate.

## Complexity Tracking

No violation to justify.

## Dev Cycle Protocol

### Backlog items and release

FEAT-067 (`docs/BACKLOG.md`), release **v0.3.0 Event log and convergence** (`docs/ROADMAP.md`), fourth feature. Plan envelope: `docs/plans/gedo_semantic_lanes.md`. Next in the release: FEAT-068 (streams), FEAT-069 (sequencer), IMP-006. Not in scope: the `gedo publish --split-lanes` command and moving the branch (FEAT-083, v0.4.0), stack proposals by lane (FEAT-113), checking that the refactor commit builds (Gedo does not compile).

### Tests (quality-assurance)

- **Lanes (US1)**: one test per acceptance scenario in `gedo-crdt/tests/lanes.rs`: each delta kind alone, a rename with a body change, follow-ups (renamed calls, moved paths, imports), a module split, a local variable that shares the old name (not a follow-up), determinism of the result.
- **Builder (US2, US3)**: scenario tests of the refactor text (rename with callers in other files, move within and across files, module split, mixed work, created and removed entities, formatting-only changes) re-parsed with `gedo-parse` in tests; a property over generated files: the refactor text keeps every feature change as in the base and every refactor change as in the final version.
- **Use case (US2, US3)**: scripted repositories in `gedo-app/tests/split.rs`: a split with both tapes, one lane only, nothing to publish, determinism and idempotence, each fallback reason (syntax error, mismatch, build error), the final tree always the last commit's tree, a base not indexed; `insta` snapshot of the outcome.
- **Benchmark**: `split` (SC-003), reported by the non-blocking bench job; the reference machine run is asked of the user, as for features 004 to 006.
- **Manual tests**: none added; there is no end-user command yet. The quickstart steps are developer checks.
- **Acceptance**: every acceptance scenario passes in CI on both platforms.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/events.md`: lanes, the refactor builder, the split publish and its fallback; moved from "later" to what exists today.
- `docs/architecture/surfaces.md`: `gedo publish --split-lanes` calls `publish_split`.
- `TECDOC.md`: data flow of a split publish.
- New **ADR-0018** "Semantic lanes per part with a refactor commit verified by re-indexing". Regenerate the constitution.
- README: no change (no user-facing command yet).

### CI, release and observability (devops-release)

- `ci.yml`: the bench job runs `cargo bench -p gedo-app --bench split`. Required checks unchanged.
- Version: the workspace stays `0.1.0`; v0.1.0 to v0.3.0 wait for the user's release gate.
- Observability profile `library`: `split published` (info: base, commits, changes and events per lane, outcome and reason), `refactor tree built` (debug: files); `event_tracing.rs` asserts the info event.

### Security, migration and rollback

- No secrets. Sources and maps come from the local repository and keep the checked decoding of features 002 and 003; a malformed input gives an error, never a panic.
- No migration: nothing new stored, no format changes. Commits not used stay unreferenced and are removed by `git gc`.
- Rollback: revert the pull request; the commits and tapes it wrote stay valid data of features 001 and 004.
