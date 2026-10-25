# Implementation Plan: Node Identity

**Branch**: `feat/003-node-identity` | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-node-identity/spec.md`

## Summary

Give every code entity a stable, deterministic identity and describe each change as semantic deltas, completing release v0.2.0. `gedo-core` gains the identity model (`AstNodeId`, `AnchoredNode`, `BlobAstMap`, `SemanticDelta`, `IdentityMerged`, `MatchFormat`) and a `unit` field on the syntax model; `gedo-ast` gains three-tier matching over a change set (exact; same place, same content elsewhere and a scored similarity at 80%; spawn or tombstone), the UUID v8 derivation of ADR-0007, delta classification and alias reconciliation by lowest id; a new core crate `gedo-app` gains the first use case, history indexing, which walks the whole history from every root, parses only changed Rust files and keeps per-commit results in the local symbol cache. Identity results are local derived data computed per commit, not shared records per blob (research R2, new ADR-0012). A reference identity corpus of scripted edits on real code measures accuracy (95% target) and generated and real histories prove determinism across machines and indexing orders.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: no new third-party dependency. `blake3` 1.8 (derivation), `postcard` and `serde` (records), existing ports. `gedo-app` depends on `gedo-core` and `gedo-ast`; its tests, examples and benchmarks use `gedo-parse` and `gedo-store` as dev-dependencies. The `uuid` crate is not added (research R7).

**Storage**: the local symbol cache (`.git/gedo/symbol_cache.db`, redb, namespace `identity`): commit records, checkpoints and maps (data-model). Nothing is written to `refs/gedo/*`.

**Testing**: `cargo test` (unit and property-based tests in `gedo-ast`; history tests, identity corpus and the Gedo-history snapshot in `gedo-app`), `insta` snapshots, `criterion` benchmarks, a fuzz target for the record decoders.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as before.

**Project Type**: Rust library crates; one example binary.

**Performance Goals**: matching a 2,000-line file under 5 ms p95 (SC-004); indexing one commit under 100 ms p95 and the total for the reference histories recorded (SC-005).

**Constraints**: pure algorithms without I/O (ADR-0001); results identical across platforms, orders and thread counts, with integer scores (research R6); bounded pair scoring (research R5); the core never spawns `git` (TECH_SPEC); no `unsafe` code.

**Scale/Scope**: the reference repository (about 100,000 lines, about 1,000 files) and histories of thousands of commits.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | Model in `gedo-core`; one module per algorithm in `gedo-ast` (`matching`, `identity`, `delta`, `alias`); orchestration only in `gedo-app::index`; `IndexError` with commit and path; algorithms tested without I/O | Pass |
| II. Simplicity first | No new port (object store, parser and cache ports suffice); no new third-party crate; one use case module; results kept local instead of a new shared layout (research R2) | Pass |
| III. Hexagonal | `gedo-app` is a core crate with only core dependencies; adapters only as dev-dependencies; `cargo xtask arch-check` enforces it | Pass |
| IV.2 Evidence | Property-based tests, corpus with accuracy gates, determinism snapshots on both platforms, fuzzing, benchmarks (research R13) | Pass |
| IV.3 Documentation describes today | `identity.md`, `core.md`, `store.md`, TECDOC data flow and TECH_SPEC data table edited in place; ADR-0012 | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security, data and rollback | Cache data is local and rebuildable; decoders size-limited and fuzzed; no migration of shared data | Pass |
| IV.9 No AI attribution | Commits by the repository identity | Pass |
| TECH_SPEC: core never spawns `git` | History is read through the object store port | Pass |
| TECH_SPEC data table: `BlobAstMap` in `refs/gedo/ast-map/<blob-sha>`, alias table in `refs/gedo/` | Changed by ADR-0012 (research R2): per-commit local results; table edited | Justified (Complexity Tracking) |
| ADR-0007: deterministic ids, versioned matching, lowest id kept | As decided (research R7, R11); the `IdentityMerged` event itself is v0.3.0 | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/003-node-identity/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/identity.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                          # members + gedo-app
.github/workflows/
├── ci.yml                          # full-depth checkout for the history snapshot; bench job adds
│                                   # match_change and index_history (ripgrep at a pinned tag)
└── fuzz.yml                        # + decode_identity_record target
crates/
├── core/
│   ├── gedo-core/src/
│   │   ├── syntax.rs               # + unit on ParsedFile and SyntaxMap
│   │   └── identity.rs             # AstNodeId, AnchoredNode, BlobAstMap, SemanticDelta,
│   │                               # IdentityMerged, MatchFormat
│   ├── gedo-ast/
│   │   ├── src/matching.rs         # change set, tiers, scoring, bounds
│   │   ├── src/identity.rs         # derive_id
│   │   ├── src/delta.rs            # classification and order
│   │   ├── src/alias.rs            # AliasTable (union-find)
│   │   ├── tests/matching.rs       # per tier, per delta, syntax errors
│   │   ├── tests/identity_properties.rs
│   │   └── benches/match_change.rs
│   └── gedo-app/                   # new core crate: use cases
│       ├── src/lib.rs
│       ├── src/index/mod.rs        # IdentityIndex, index_history, map_at, changes_of
│       ├── src/index/walk.rs       # ancestry walk and topological order
│       ├── src/index/diff.rs       # tree diff against the first parent
│       ├── src/index/records.rs    # CommitRecord, Checkpoint, encoding
│       ├── tests/history.rs        # generated histories, orders, interruption
│       ├── tests/identity_corpus.rs  # FEAT-014 generator and accuracy report
│       ├── tests/gedo_history.rs   # snapshot over the repository's own history
│       ├── benches/index_history.rs
│       └── examples/index_history.rs
└── adapters/
    └── gedo-parse/src/rust/paths.rs  # + unit_of(path)
fuzz/fuzz_targets/decode_identity_record.rs
```

**Structure Decision**: the layout of ADR-0001, adding `gedo-app`, the use case crate the architecture reserved for the first use case. Algorithms extend `gedo-ast`, as feature 002 planned; the parser adapter gains only the unit rule.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| TECH_SPEC and ADR-0003 say `BlobAstMap` is shared per blob in `refs/gedo/ast-map/` | Identities depend on history, so a per-blob record is not deterministic across machines (research R2) | Per-blob first-writer-wins diverges between machines; per-commit shared trees need a new store layout and add hundreds of MB to `refs/gedo` to save seconds of recomputation. ADR-0012 records the change and TECH_SPEC is edited |

## Dev Cycle Protocol

### Backlog items and release

FEAT-011, FEAT-012, FEAT-013, FEAT-014 and FEAT-015 (`docs/BACKLOG.md`), release **v0.2.0 Node identity** (`docs/ROADMAP.md`), second and last feature. Plan envelope: `docs/plans/gedo_node_identity.md`. Not in scope: BUG-001 (medium, Windows parse benchmark) and IMP-001 (grammar gaps), both outside any release.

### Tests (quality-assurance)

- **Matching (User Stories 1, 4)**: unit tests per tier (exact, same place, same content elsewhere, similarity, fallback), moves between files and file renames, copies, splits, merges of entities, ties, the pair bound, syntax errors (carried nodes, no tombstone).
- **Derivation and determinism (User Story 2)**: property tests for input order invariance, idempotence (a version against itself), uniqueness of derived ids and the collision counter; resurrection test.
- **Reconciliation (User Story 3)**: alias resolution invariant under permutation and grouping of merges; chains; histories with independent creations merged later.
- **History (User Story 5)**: generated histories on the in-memory store (linear, branches, merges, octopus, several roots), indexed in at least 3 orders and interrupted at random points; snapshot of identities over selected commits of the Gedo repository on both platforms.
- **Corpus (User Story 6)**: `identity_corpus` with the per-category report and the gates of SC-001 and SC-002.
- **Fuzzing (SC-007)**: decoder target for commit records, checkpoints and maps.
- **Benchmarks**: `match_change` (SC-004), `index_history` (SC-005), reported by the non-blocking bench job.
- **Manual tests**: none added; quickstart steps 3 and 4 are developer checks.
- **Acceptance**: all acceptance scenarios pass in CI on both platforms, the corpus gates pass, the determinism snapshot is identical on both platforms, the fuzz run passed once.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/identity.md`: matching tiers, score, derivation, deltas, reconciliation and indexing, edited in place.
- `docs/architecture/core.md`: identity types (and `AstNodeId` as v8, replacing "v7 for new nodes"), the `gedo-app` crate.
- `docs/architecture/store.md`: the `identity` namespace of the cache.
- `TECDOC.md`: data flow of indexing; `docs/product/TECH_SPEC.md`: data table rows for `BlobAstMap` and the alias table.
- New **ADR-0012** "Identity results are local derived data computed per commit" (amends ADR-0003 and ADR-0007's note on the shared alias table). Regenerate the constitution (accepted ADRs are a source).
- README: no change (no user-facing command yet).

### CI, release and observability (devops-release)

- `ci.yml`: `fetch-depth: 0` in the test job for the history snapshot; the bench job runs `match_change` and `index_history`, cloning ripgrep at a pinned tag with a known commit id. Required checks unchanged.
- `fuzz.yml`: the new decoder target.
- Version: the workspace stays `0.1.0`; this feature completes the items of v0.2.0, whose candidate and release follow the release of v0.1.0 at the user's release gate.
- Observability profile `library`: `tracing` events in `gedo-app::index` (commits indexed and skipped, files refused with the reason, records written, time per commit at debug level); tests assert the refused-file event.

### Security, migration and rollback

- No secrets. Blobs from the history are untrusted input for the parser (limits of feature 002); cache records are decoded with size limits and format versions, never panic, and are fuzzed.
- No shared data and no migration: the `identity` namespace is new; an unknown format or corruption clears it and indexing runs again.
- Rollback: revert the pull request; the cache namespace is ignored by older builds.
