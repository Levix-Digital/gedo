# Implementation Plan: Syntax Model

**Branch**: `feat/002-syntax-model` | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-syntax-model/spec.md`

## Summary

Turn a source file into the list of its code entities with stable structural hashes, the base of node identity (v0.2.0). `gedo-core` gains the language-neutral model (entity kinds, tokens, parsed file, syntax map, hash format) and the `Parser` port; a new core crate `gedo-ast` normalizes a parsed file (drops comments, normalizes line endings, ignores trailing separators and the order of imports) and computes BLAKE3 structural, signature and body hashes under a versioned format; a new adapter crate `gedo-parse` implements the port with Tree-sitter for Rust, with the entity mapping, token classification, qualified paths, error tolerance and limits. Rust is declared supported by a conformance suite with cross-platform snapshots, formatter, change and error-injection corpora, a real-world sweep over the locked dependencies, property tests and a fuzz target.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95.

**Primary Dependencies**: new: `tree-sitter` 0.27 and `tree-sitter-rust` 0.24 (only in `gedo-parse`), `blake3` 1.8 (in `gedo-ast`, already in the technical specification). Dev: `insta` 1.x (snapshots), `proptest` 1.12, `criterion` 0.8. Fuzzing: `cargo-fuzz` with `libfuzzer-sys` 0.4 in `fuzz/`, outside the workspace.

**Storage**: none. Syntax maps are values; feature 003 stores them in `BlobAstMap` records.

**Testing**: `cargo test` (property tests in `gedo-ast`, conformance suite and corpora in `gedo-parse`), `insta` snapshots, `criterion` benchmark, `cargo xtask parse-sweep`, a fuzz target.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+), as v0.1.0.

**Project Type**: Rust library crates; one example binary.

**Performance Goals**: parse, normalization and hashing of a 2,000-line Rust file under 10 ms p95 (SC-001; spike: about 5 to 7 ms, research R1).

**Constraints**: core crates without Tree-sitter (ADR-0001, `forbidden_core_imports`); deterministic results across platforms (FR-007); limits of 16 MiB, nesting 512 and 2 s per file (research R8); no `unsafe` code (workspace lint).

**Scale/Scope**: the reference repository (about 100,000 lines in about 1,000 files); real-world sweep of about 300,000 lines.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | Model and port in `gedo-core`, normalization and hashing in `gedo-ast`, grammar mapping in `gedo-parse`, one module per language; `ParseError` with path and limit; normalization is pure and tested without the grammar | Pass |
| II. Simplicity first | One new port (`Parser`), which isolates an external dependency (Tree-sitter) and serves the core's tests; no plugin system for languages: one module per language and an enum; no incremental parsing until measured (R10) | Pass |
| III. Hexagonal | `tree_sitter` is already in `forbidden_core_imports`; `gedo_parse` is added to it; `gedo-ast` depends only on `gedo-core` and `blake3`; `cargo xtask arch-check` enforces it | Pass |
| IV.2 Evidence | Snapshots, corpora, property tests, sweep, benchmark and fuzz target (research R9) | Pass |
| IV.3 Documentation describes today | `parsing.md`, `identity.md` and `core.md` edited in place; TECH_SPEC library table | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux | Pass |
| IV.6 Security | Untrusted input limits, no panics, fuzzing; no new network access (the sweep reads the local Cargo registry) | Pass |
| IV.9 No AI attribution | Commits by the repository identity | Pass |
| TECH_SPEC: fuzzing for decoders | The parser is fuzzed with a coverage-guided target (SC-007) | Pass |
| ADR-0002: Tree-sitter isolated in `gedo-parse`, BLAKE3 structural hash | As decided | Pass |
| ADR-0007: identities from `kind ‖ qualified_path ‖ structural_hash` | This feature provides those three inputs; ids are feature 003 | Pass |

Re-check after design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/002-syntax-model/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/parser-port.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                          # members + gedo-ast, gedo-parse; tree-sitter, tree-sitter-rust, blake3, insta
.gitattributes                      # fixtures checked out byte-exact (-text)
.github/workflows/
├── ci.yml                          # + cargo xtask parse-sweep; bench job runs parse_rust too
└── fuzz.yml                        # manual and release-candidate: 10 min cargo-fuzz on Linux
crates/
├── core/
│   ├── gedo-core/src/
│   │   ├── syntax.rs               # Language, EntityKind, TokenClass, Token, ParsedFile, RawEntity,
│   │   │                           # SyntaxMap, CodeEntity, StructuralHash, HashFormat
│   │   ├── ports/parser.rs         # Parser port, ParseError
│   │   └── memory/parser.rs        # FixedParser: returns prepared ParsedFiles (feature "memory")
│   └── gedo-ast/                   # new core crate
│       ├── src/lib.rs
│       ├── src/normalize.rs        # trivia, line endings, trailing separators, import groups
│       ├── src/hash.rs             # HashFormat::V1 encoding, structural/signature/body hashes
│       ├── src/validate.rs         # ParsedFile structure checks
│       └── tests/properties.rs
└── adapters/
    └── gedo-parse/                 # new adapter crate
        ├── src/lib.rs              # TreeSitterParser: Parser, language detection, limits
        ├── src/walk.rs             # iterative tree walk, tokens, error flags
        ├── src/rust/mod.rs         # Rust mapping
        ├── src/rust/kinds.rs       # grammar node -> EntityKind
        ├── src/rust/paths.rs       # module path from file path, impl and method paths
        ├── src/rust/tokens.rs      # leaf -> TokenClass
        ├── tests/conformance.rs    # fixtures + insta snapshots
        ├── tests/formatting.rs     # formatter corpus, CRLF, re-indentation (SC-002)
        ├── tests/changes.rs        # one-token changes (SC-003)
        ├── tests/errors.rs         # error injection (SC-005), limits
        ├── tests/properties.rs     # arbitrary bytes
        ├── tests/known_grammar_gaps.toml
        ├── tests/fixtures/rust/    # fixtures, formatted/ variants
        ├── benches/parse_rust.rs
        └── examples/syntax_map.rs
fuzz/                               # cargo-fuzz, outside the workspace
└── fuzz_targets/parse_rust.rs
tools/xtask/src/
├── parse_sweep.rs                  # sweep over the Gedo workspace and the locked dependencies
└── format_corpus.rs                # regenerates tests/fixtures/rust/formatted/ with rustfmt
```

**Structure Decision**: the layout of ADR-0001, adding the two crates this release fills: `gedo-ast` (core) and `gedo-parse` (adapter). The model and the port live in `gedo-core` with the other shared types; `gedo-ast` holds the algorithms, which feature 003 extends with matching and identity. `gedo-app` is not created yet: there is no use case until the ingest of feature 003.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New dev dependency `insta` and the nightly-only `fuzz/` crate, not in TECH_SPEC's library table | Reviewable snapshots for the conformance suite; TECH_SPEC requires fuzzing, which needs `cargo-fuzz` | Hand-written expected files are more work to keep in sync; property tests alone do not explore the parser's state space like coverage-guided fuzzing. TECH_SPEC's table is updated |

## Dev Cycle Protocol

### Backlog items and release

FEAT-009, FEAT-010 and FEAT-016 (`docs/BACKLOG.md`), release **v0.2.0 Node identity** (`docs/ROADMAP.md`), first of two features; FEAT-011 to FEAT-015 follow in `003-node-identity`. Plan envelope: `docs/plans/gedo_syntax_model.md`.

### Tests (quality-assurance)

- **Conformance (User Story 1, 5)**: fixtures for every entity kind, nested and inline modules, inherent, trait and generic impls, items in function bodies, `#[cfg]` duplicates, every module path rule (`lib.rs`, `main.rs`, `mod.rs`, `tests/`, `benches/`, `examples/`, `build.rs`), raw identifiers, byte order mark, non-ASCII identifiers; `insta` snapshots.
- **Determinism (User Story 3, SC-004)**: the snapshots are the reference on both platforms; a test parses all fixtures on 8 threads in random order and compares; the snapshot changes only with a `HashFormat` bump.
- **Formatter corpus (User Story 2, SC-002)**: committed `rustfmt` variants (research R5), CRLF conversion and re-indentation; every entity keeps its three hashes.
- **Change corpus (SC-003)**: one significant token changed per case for every `TokenClass` except comments, plus swapped statements and changed attributes; the changed entity and its containers change hash, its siblings do not.
- **Error injection (User Story 4, SC-005)**: research R8; at least 95% of entities outside the damaged region unchanged; empty, truncated, binary and invalid UTF-8 files; `TooLarge`, `TooDeep`, `Timeout`.
- **Property tests**: `gedo-ast` (trivia and permutation invariance, sensitivity to token changes, validation rejects malformed `ParsedFile`s) and `gedo-parse` (arbitrary bytes never panic).
- **Real-world sweep (SC-006)**: `cargo xtask parse-sweep` in CI on Linux.
- **Benchmark (SC-001)**: `parse_rust` on a 2,000-line fixture, reported by the non-blocking bench job.
- **Fuzzing (SC-007)**: `fuzz.yml`, 10 minutes, before each release candidate.
- **Manual tests**: none added; quickstart step 3 is a developer check.
- **Acceptance**: all acceptance scenarios pass in CI on both platforms; the sweep reports zero crashes and only listed grammar gaps; the fuzz run passed once.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/parsing.md`: Rust mapping, token classes, qualified path rules, limits, error tolerance, known grammar gaps; edited in place.
- `docs/architecture/identity.md`: normalization rules and the hash format (`gedo-ast`), with the note that matching arrives in feature 003.
- `docs/architecture/core.md`: the syntax model types and the `Parser` port with its in-memory `FixedParser`.
- `docs/product/TECH_SPEC.md`: library table gains `insta` and `cargo-fuzz`; regenerate the constitution only if its sources changed.
- No ADR: every decision follows ADR-0001, ADR-0002 and ADR-0007. README: the build prerequisite of a C compiler.

### CI, release and observability (devops-release)

- `ci.yml`: `cargo xtask parse-sweep` in the Linux test job after `cargo fetch`; the bench job also runs `parse_rust`. Required checks unchanged (test on both platforms, msrv, deny).
- New `fuzz.yml` (manual dispatch), run before each release candidate.
- Version: the workspace stays `0.1.0` until v0.2.0 is released after feature 003; no tag from this feature.
- Observability profile `library`: `tracing` events in `gedo-parse` for refused files (limit and path) and grammar errors (count per file); tests assert them.

### Security, migration and rollback

- No secrets. Source files are untrusted input for the parser: limits, timeout, no panics, fuzzing.
- No persisted data and no migration: syntax maps are not stored until feature 003, so the hash format can still change freely until v0.2.0 is released; after that, a change needs `HashFormat::V2`.
- Rollback: revert the pull request; nothing outside the code changes.
