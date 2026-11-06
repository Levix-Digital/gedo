# Implementation Plan: Virtual Streams

**Branch**: `feat/009-virtual-streams` | **Date**: 2026-11-06 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/009-virtual-streams/spec.md`

## Summary

Keep named units of work on the developer's machine. `gedo-core` gains the stream model (`StreamName`, `Stream`, the working tree state, their stored formats) and a new port, `StreamStore`, with an in-memory adapter: one record and one write-ahead log per stream, the active stream and outside work per working tree, and pins that keep saved work alive (research R2 to R4). `gedo-crdt` gains the pure rules: stream filters by labels and agent and the choice of the recording context, the stream's inside a stream and `GEDO_AGENT`/`GEDO_LABELS` outside (R6, R8). `gedo-app` gains `Streams`: create, get, list, record inside a stream through `EventLog` over the stream's log, switch (save the current work tree into the stream left, return the target's), publish (write the commit of the stream's work on its base, record it, publish the events as its tape, move the stream onto it), the convergence side of a stream and discard (R5, R7, R9 to R11). `gedo-store` gains `FileStreamStore`: `.git/gedo/streams/<hex name>/` with the record and `wal.log`, the state in each working tree's Git directory, pins under `refs/gedo-local/` (never pushed). No branch, `HEAD`, index or working file is written; the event format does not change. ADR-0020 records the decision.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: none new (`gix`, `postcard`, `serde`, `thiserror`, `tracing` already used).

**Storage**: new local files `.git/gedo/streams/<key>/{stream,wal.log}` and `<git dir>/gedo/stream-state`; local references `refs/gedo-local/streams/<key>` and `refs/gedo-local/worktrees/<id>`; commits and trees through `ObjectStore`; tapes through `EventLog::publish` (ADR-0014). Nothing under `refs/gedo/*` besides the tapes of published streams.

**Testing**: `cargo test` (unit and property tests in `gedo-core` and `gedo-crdt`, `StreamStore` contract tests on both adapters, scenario tests on temporary repositories in `gedo-app` with an `insta` snapshot, a `git gc` test in `gedo-store`), `criterion` benchmark.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as before.

**Project Type**: Rust library crates; one example binary.

**Performance Goals**: create, switch and get under 10 ms p95; list 1,000 streams under 100 ms; publishing a stream adds under 10 ms to `publish` (SC-003).

**Constraints**: pure logic without I/O in `gedo-core` and `gedo-crdt` (ADR-0001); no `git` subprocess outside tests; no reference written outside `refs/gedo/*` except the local pins under `refs/gedo-local/`; no write to the working tree, the index, `HEAD` or branches; no `unsafe` code.

**Scale/Scope**: up to thousands of streams per repository; a stream's log under the tape limit (65,536 events, ADR-0014).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | Model and port in `gedo-core::stream`, rules in `gedo-crdt::streams`, use cases in `gedo-app::streams`, I/O in `gedo-store::streams`; `StreamError` with context; every use case testable on the memory adapters | Pass |
| II. Simplicity first | One new port, needed because streams are neither derived data (cache) nor shared data (`refs/gedo/*`); `EventLog` and `FileWal` reused as they are; the stream's log is its lock (R10) | Pass |
| III. Hexagonal | Only `gedo-store` touches files and references; `cargo xtask arch-check` | Pass |
| IV.2 Evidence | Unit, property, contract and scenario tests per acceptance scenario, interruptions injected at every publish step, a `git gc` test, benchmark (R12) | Pass |
| IV.3 Documentation describes today | `events.md`, `store.md`, `surfaces.md`, TECDOC edited in place; ADR-0020 | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security, data and rollback | Local data only; atomic replace of records and state; a publish recorded before its tape; discard refuses to drop work without `force`; nothing pushed | Pass |
| IV.9 No AI attribution | Commits by the repository identity; texts checked before every push | Pass |
| TECH_SPEC: `gedo publish` under 1 s | SC-003, benchmark `streams` | Pass |
| ADR-0011: only `refs/gedo/*` is pushed and fetched | Pins live under `refs/gedo-local/`, outside both refspecs | Pass |
| ADR-0013: labels and author kind inherited from streams, `gedo.` reserved | R6; `Labels::for_recording` at creation | Pass |
| ADR-0014: events, clocks, tapes, idempotence | Events recorded by `EventLog::record` on the stream's base; published by `EventLog::publish` | Pass |
| ADR-0017: convergence of sides | A stream is a `Side` (R11) | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/009-virtual-streams/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/streams.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
.github/workflows/ci.yml              # bench job adds the streams benchmark
crates/core/
├── gedo-core/
│   ├── src/lib.rs                    # exports stream
│   ├── src/stream.rs                 # StreamName, Stream, WorktreeState, records and their checks
│   ├── src/ports/stream.rs           # StreamStore
│   ├── src/ports/mod.rs
│   ├── src/memory/stream.rs          # MemoryStreamStore (with MemoryWal logs)
│   └── src/contract_tests.rs         # StreamStore contract
├── gedo-crdt/
│   ├── src/lib.rs                    # exports streams
│   └── src/streams.rs                # StreamFilter, Stream::context, recording_context
└── gedo-app/
    ├── Cargo.toml                    # [[bench]] streams, [[example]] streams
    ├── src/lib.rs                    # exports streams
    ├── src/streams.rs                # Streams, StreamError, StreamPublish, Discarded
    ├── tests/streams.rs              # scenarios, interruptions, properties, snapshot
    ├── tests/event_tracing.rs        # asserts the stream info events
    ├── benches/streams.rs
    └── examples/streams.rs
crates/adapters/gedo-store/
├── src/lib.rs                        # exports streams
├── src/streams.rs                    # FileStreamStore
├── src/repository.rs                 # worktree id and per-worktree Gedo directory
├── tests/contract.rs                 # StreamStore contract on the file adapter
└── tests/streams_gc.rs               # pins survive git gc
docs/adr/0020-*.md
```

**Structure Decision**: the layout of ADR-0001; the event layer and the store adapter grow, no new crate.

## Complexity Tracking

| Addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| New port `StreamStore` | Streams are local, durable and not derived (R2) | The symbol cache may be dropped at any time; `refs/gedo/*` is pushed |
| Local references `refs/gedo-local/` | `git gc` prunes unreferenced objects after two weeks | Keeping work only in Gedo files would lose it silently |

## Dev Cycle Protocol

### Backlog items and release

FEAT-068 (`docs/BACKLOG.md`), release **v0.3.0 Event log and convergence** (`docs/ROADMAP.md`). Plan envelope: `docs/plans/gedo_virtual_streams.md`. Left in the release after it: FEAT-069 (sequencer). Not in scope: the `gedo stream` commands and their `--json` (CLI, FEAT-088, v0.4.0), writing the working tree (the CLI), hidden checkouts for agent streams (FEAT-130), local intents (FEAT-129), the radar's use of streams (FEAT-074), moving a stream to a new base, `gedo graft --into <stream>`.

### Tests (quality-assurance)

- **Model and port**: name rule unit tests and a property over generated names; record and state round trips; refused records (format byte, decoding, wrong name, reserved label); the `StreamStore` contract (create once, replace, remove, names, one log writer, pins, state) on the memory and file adapters.
- **Rules**: filter matching (every label, agent), context inside and outside a stream, environment ignored inside; a property that a stream's context always carries its agent and labels.
- **Use cases**: one scenario per acceptance scenario of US1 to US5 in `gedo-app/tests/streams.rs` on temporary repositories, across processes (reopened stores) and with a linked working tree; publish interrupted after the commit, after the record, after the tape; nothing to publish; converge with and without conflicts; discard with and without `force`; a discarded active stream; an `insta` snapshot of the outcomes; a property that every switch returns the tree last saved for its target.
- **Garbage collection**: saved work and a published commit survive `git gc --prune=now`; a discarded stream's pin is gone.
- **Benchmark**: `streams` (SC-003), reported by the non-blocking bench job; the reference machine run is asked of the user, as for the earlier features.
- **Manual tests**: none added; there is no end-user command yet. The quickstart steps are developer checks.
- **Acceptance**: every acceptance scenario passes in CI on both platforms.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/events.md`: virtual streams moved from "later" to what exists today (the model, inheritance of author and labels, switch, publish, discard, convergence side).
- `docs/architecture/store.md`: the stream store, its files, the per-working-tree state and the `refs/gedo-local/` pins.
- `docs/architecture/surfaces.md`: `gedo stream` calls `Streams`.
- `TECDOC.md`: index regenerated.
- New **ADR-0020** "Virtual streams as local records with their own log and pinned work commits". Regenerate the constitution.
- README: no change (no user-facing command yet).

### CI, release and observability (devops-release)

- `ci.yml`: the bench job runs `cargo bench -p gedo-app --bench streams`. Required checks unchanged.
- Version: the workspace stays `0.1.0`; v0.1.0 to v0.3.0 wait for the user's release gate.
- Observability profile `library`: `stream created`, `stream switched`, `stream published` (commit, tape, events), `stream discarded` (info); `events recorded in a stream` (debug); `stream publish completed`, `stream record refused`, `active stream missing` (warn); `event_tracing.rs` asserts the info events.

### Security, migration and rollback

- No secrets. Records are decoded with checks; a malformed record or state is an error with its name, never a panic.
- No migration: new files and references only; the event format and every existing stored format are unchanged. Streams are local and never pushed.
- Rollback: revert the pull request; the files under `.git/gedo/streams`, the state files and `refs/gedo-local/` are then unused and can be removed by hand; published commits and tapes stay valid data of features 001 and 004.
