# Implementation Plan: Event Model

**Branch**: `feat/004-event-model` | **Date**: 2026-10-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-event-model/spec.md`

## Summary

Turn the semantic deltas and identity merges of feature 003 into events, the shared history of v0.3.0. `gedo-core` gains the event model (`Event`, `EventId`, `Clock`, `Author` with the author kind, `Labels`, `SemanticOp`, `Origin`, `Tape`); a new core crate `gedo-crdt` (the crate the event log page reserves) gains the pure logic: content-derived ids for live and reconstructed events (research R2), clocks built on commit generations so reconstruction is reproducible and causal (R3), the author fingerprint over the trimmed, lowercased Git e-mail (R4), the label rules and the `GEDO_LABELS` text form of ADR-0013 (R6), reconstruction of a commit's events (R10) and filters (R12). `gedo-app` gains the `EventLog` use case: record live events into the write-ahead log, publish them as one tape per commit into three content-addressed collections (`events`, `tapes`, `commit-tapes`, R9), and read tapes, events, reconstructed events and filters. Reconstructed events stay local derived data; only live events are shared. ADR-0014 records the format and layout decisions.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: no new third-party dependency. `gedo-crdt` uses `gedo-core`, `blake3`, `postcard`, `serde`, `thiserror`. `gedo-app` adds `gedo-crdt`. The `uuid` crate is still not added (ids are built like `AstNodeId`).

**Storage**: shared: three new collections under `refs/gedo/` (research R9). Local: the write-ahead log (`.git/gedo/wal.log`, existing port) and the symbol cache namespace `events` (generations and reconstructed events).

**Testing**: `cargo test` (unit and property-based tests in `gedo-core` and `gedo-crdt`; use-case tests with crash injection, joined publishes, generated histories and a snapshot over this repository's history in `gedo-app`), `insta` snapshots, `criterion` benchmarks, a fuzz target for the record decoders.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as before.

**Project Type**: Rust library crates; one example binary.

**Performance Goals**: record one event under 1 ms p95, publish 1,000 events under 1 s (SC-004); reconstruct one commit under 100 ms p95 (SC-005); filter 10,000 events under 500 ms (SC-006).

**Constraints**: pure logic without I/O in `gedo-core` and `gedo-crdt` (ADR-0001); stored formats versioned, decoded safely and identical on every platform; no `git` subprocess; no `unsafe` code.

**Scale/Scope**: tapes up to 65,536 events; event collections measured to 100,000 entries; histories of thousands of commits.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | Model in `gedo-core::event`; one module per rule in `gedo-crdt` (`id`, `clock`, `author`, `labels`, `reconstruct`, `filter`); orchestration only in `gedo-app::events`; `EventError` and `LabelError` with context | Pass |
| II. Simplicity first | No new port (one additive field on `CommitInfo`, one read method on `Collection`); no new third-party crate; no secondary label index until measured (R12) | Pass |
| III. Hexagonal | `gedo-crdt` and `gedo-app` are core crates with core dependencies only; `cargo xtask arch-check` covers `crates/core/` | Pass |
| IV.2 Evidence | Property-based tests, crash-injection tests, determinism snapshot on both platforms, fuzzing, benchmarks (R13) | Pass |
| IV.3 Documentation describes today | `events.md`, `core.md`, `store.md`, TECDOC and the TECH_SPEC data table edited in place; ADR-0014 | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security, data and rollback | Shared records size-limited, versioned, id-checked and fuzzed; new collections only, no migration | Pass |
| IV.9 No AI attribution | Commits by the repository identity; texts checked with `_no_ai.py` | Pass |
| TECH_SPEC: `author_fingerprint` is a hash, not the identity | R4 | Pass |
| TECH_SPEC data table: events, tapes and index in `refs/gedo/tapes/` | Three collections (R9); ADR-0014 and the table edited | Justified (Complexity Tracking) |
| ADR-0007: event ids derived from content for reconstructed events | Also for live events (R2), which keeps ADR-0007's "live ids keep their id when they travel" | Pass |
| ADR-0008: content-addressed events, tapes and a commit index | R8, R9 | Pass |
| ADR-0011: collections as commit chains | Three new collections, no change to the layout | Pass |
| ADR-0013: labels, limits, reserved prefix, author kind | R5, R6 | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/004-event-model/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/events.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                          # members + gedo-crdt
.github/workflows/
├── ci.yml                          # bench job adds the events benchmarks
└── fuzz.yml                        # + decode_event_record target
crates/
├── core/
│   ├── gedo-core/src/
│   │   ├── event.rs                # Event, EventId, Clock, Author, Labels, SemanticOp, Origin,
│   │   │                           # Tape, TapeId and their invariants
│   │   ├── ports/object.rs         # + CommitInfo.author_email
│   │   ├── memory/objects.rs       # + author e-mail of written test commits
│   │   └── collection.rs           # + entries_with_prefix
│   ├── gedo-crdt/                  # new core crate
│   │   ├── src/lib.rs
│   │   ├── src/id.rs               # event and tape id derivation
│   │   ├── src/clock.rs            # generations, live and reconstructed clocks, total order
│   │   ├── src/author.rs           # fingerprint
│   │   ├── src/labels.rs           # GEDO_LABELS text form, RecordingContext
│   │   ├── src/reconstruct.rs      # events of a commit from its deltas and merges
│   │   ├── src/filter.rs           # EventFilter
│   │   └── tests/properties.rs     # ids, labels, clocks, order
│   └── gedo-app/
│       ├── src/events/mod.rs       # EventLog: record, publish, read, filter
│       ├── src/events/records.rs   # WalRecord, record encoding and checked decoding
│       ├── src/events/generation.rs  # generations memoized in the cache
│       ├── tests/events.rs         # record, publish, crashes, joined publishes, filters
│       ├── tests/event_history.rs  # reconstruction: generated histories, orders, snapshot
│       ├── benches/events.rs
│       └── examples/events.rs
└── adapters/
    └── gedo-store/src/objects.rs   # + author e-mail in read_commit
fuzz/fuzz_targets/decode_event_record.rs
docs/adr/0014-event-format-content-derived-ids-history-clocks-and-three-collections.md
```

**Structure Decision**: the layout of ADR-0001, adding `gedo-crdt`, the event layer crate named by the architecture. Adapters change only by one read field.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| TECH_SPEC and ADR-0003 put events, tapes and the index in `refs/gedo/tapes/` | Three kinds of content-addressed entries need their own keys, and the index keyed by commit and tape is the only conflict-free form (research R8, R9) | One collection with mixed entries makes every reader list all kinds and needs a kind prefix in ids; ADR-0014 records the change and TECH_SPEC is edited |

## Dev Cycle Protocol

### Backlog items and release

FEAT-063 and FEAT-064 (`docs/BACKLOG.md`), release **v0.3.0 Event log and convergence** (`docs/ROADMAP.md`), first feature; FEAT-065 to FEAT-069 follow in later features (the user chose this scope on 2026-10-29). Plan envelope: `docs/plans/gedo_event_model.md`. Not in scope: IMP-001, IMP-003 and IMP-004 (outside any release or later releases).

### Tests (quality-assurance)

- **Ids and encoding (US1)**: property tests for equal content giving equal ids and any changed field giving a different id; round trips; snapshot of the encoded bytes of sample events (same on both platforms).
- **Author and labels (US2)**: parsing and refusals of the text forms (every rule, positions in errors), the reserved prefix refused when recording and accepted when decoding, filters by label, kind, agent and fingerprint.
- **Record and publish (US3)**: publish of recorded events, empty log, events appended during a publish, oversize refusal keeping the log, crash injection at every step with re-run equality (SC-003), two machines publishing and joining chains.
- **Reconstruction (US4)**: generated histories (linear, branches, merges, several roots) reconstructed in 3 orders; ids as recorded, not resolved; snapshot over selected commits of this repository on both platforms (SC-001).
- **Clocks (US5)**: property tests of monotonicity along generated histories and of order independence.
- **Safety (US6)**: malformed, truncated, oversized, wrong-version and id-mismatch records; fuzz target (SC-007).
- **Benchmarks**: `events` (record, publish, reconstruct, filter), reported by the non-blocking bench job.
- **Manual tests**: none added; there is no end-user command yet. Quickstart steps 3 to 5 are developer checks.
- **Acceptance**: all acceptance scenarios pass in CI on both platforms, the determinism snapshot is identical on both, the fuzz run passed once.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/events.md`: what exists today (event, ids, clocks, author, labels, tapes, index, reconstruction, record and publish), with the CRDT, lanes, streams and sequencer as later responsibilities; code paths `crates/core/gedo-crdt` and `crates/core/gedo-app/src/events`.
- `docs/architecture/core.md`: the event types (replacing the placeholder `SemanticOp` names), `CommitInfo.author_email`, `entries_with_prefix`.
- `docs/architecture/store.md`: the three collections and the `events` cache namespace.
- `TECDOC.md`: data flow of recording and publishing; `docs/product/TECH_SPEC.md`: data table rows for events, tapes and the index, and the `uuid` row removed.
- New **ADR-0014** "Event format: content-derived ids, history clocks and three collections" (amends ADR-0003 and ADR-0008 on the layout). Regenerate the constitution (accepted ADRs and TECH_SPEC are sources).
- README: no change (no user-facing command yet).

### CI, release and observability (devops-release)

- `ci.yml`: the bench job runs `cargo bench -p gedo-app --bench events`. Required checks unchanged.
- `fuzz.yml`: the new decoder target.
- Version: the workspace stays `0.1.0`; v0.1.0 and v0.2.0 are still waiting for the user's release gate, and v0.3.0 is released after them.
- Observability profile `library`: `tracing` events in `gedo-app::events` (research R14); tests assert the publish and refusal events.

### Security, migration and rollback

- No secrets. Shared records are untrusted input: size limits, format versions, invariant and id checks, no panic, fuzzed. The author e-mail is never stored, only its hash.
- No migration: the collections and the cache namespace are new; earlier builds ignore them. The `CommitInfo` field is additive and internal.
- Rollback: revert the pull request; data already written to the new collections is ignored by older builds and can be removed by deleting the three references.
