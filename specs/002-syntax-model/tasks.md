---

description: "Tasks for the syntax model (v0.2.0, first feature)"
---

# Tasks: Syntax Model

**Input**: Design documents from `/specs/002-syntax-model/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/parser-port.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md), in priority order. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US5 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: new crates and dependencies

- [X] T001 Add `crates/core/gedo-ast` and `crates/adapters/gedo-parse` to the workspace members in `Cargo.toml` and pin `tree-sitter = "0.27"`, `tree-sitter-rust = "0.24"`, `blake3 = "1.8"`, `insta = { version = "1", features = ["yaml"] }` in `[workspace.dependencies]`
- [X] T002 [P] Create the crate `crates/core/gedo-ast` (`Cargo.toml` depending on `gedo-core` and `blake3`; dev-dependencies `gedo-core` with `memory`, `proptest`; `src/lib.rs` with crate docs)
- [X] T003 [P] Create the crate `crates/adapters/gedo-parse` (`Cargo.toml` depending on `gedo-core`, `gedo-ast`, `tree-sitter`, `tree-sitter-rust`, `thiserror`, `tracing`; dev-dependencies `insta`, `proptest`, `criterion`; `src/lib.rs` with crate docs)
- [X] T004 [P] Add `.gitattributes` entries so `crates/adapters/gedo-parse/tests/fixtures/**` is checked out byte-exact (`-text`)
- [X] T005 Verify the setup: `cargo build --workspace`, `cargo deny check` and `cargo xtask arch-check` pass, and `gedo-ast` is accepted as a core crate without Tree-sitter

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the neutral model, the port and the normalization every story builds on

**⚠️ CRITICAL**: no user story starts before this phase is complete

- [X] T006 [P] Implement `Language`, `EntityKind`, `TokenClass`, `Token`, `RawEntity`, `ParsedFile`, `CodeEntity`, `SyntaxMap`, `StructuralHash` (32 bytes, lowercase hex display) and `HashFormat` in `crates/core/gedo-core/src/syntax.rs` as in `data-model.md`
- [X] T007 [P] Define the `Parser` port and `ParseError` (`UnsupportedLanguage`, `TooLarge`, `TooDeep`, `Timeout`, `Internal`, each with the path) in `crates/core/gedo-core/src/ports/parser.rs` as in `contracts/parser-port.md`
- [X] T008 Implement `FixedParser` (returns prepared `ParsedFile`s by path, `UnsupportedLanguage` otherwise) behind the `memory` feature in `crates/core/gedo-core/src/memory/parser.rs`
- [X] T009 Implement `ParsedFile` validation (nested token ranges, containers before children, ranges within the source) in `crates/core/gedo-ast/src/validate.rs`, with tests for each violation
- [X] T010 Write property tests in `crates/core/gedo-ast/tests/properties.rs`: inserting, removing or editing `Comment` and `DocComment` tokens never changes a hash; changing the text or class of any significant token changes the hashes of its entity and containers and no other; permuting the `Import` entities of a container never changes its hash; a trailing `Separator` before a `CloseDelimiter` never changes a hash; CRLF and CR in token text hash like LF
- [X] T011 Implement normalization (drop trivia, line endings, trailing separators, order-insensitive `Import` groups) in `crates/core/gedo-ast/src/normalize.rs` (research R4, R5)
- [X] T012 Implement `HashFormat::V1` hashing (derive-key context, class byte, `u32` length, text; structural, signature and body hashes) in `crates/core/gedo-ast/src/hash.rs` (research R6) and the public `normalize(&ParsedFile, &[u8]) -> Result<SyntaxMap, ParseError>`; T010 passes

**Checkpoint**: the core turns any valid `ParsedFile` into a `SyntaxMap`, tested without a grammar

---

## Phase 3: User Story 1 - Turn a Rust file into its code entities (Priority: P1) 🎯 MVP

**Goal**: Rust files become syntax maps with the right kinds, paths, ranges and containers

**Independent Test**: `crates/adapters/gedo-parse/tests/conformance.rs`

- [X] T013 [P] [US1] Write the conformance fixtures in `crates/adapters/gedo-parse/tests/fixtures/rust/` (every entity kind, nested and inline modules, inherent, trait and generic impls, trait items, items in function bodies, `#[cfg]` duplicates, raw and non-ASCII identifiers, a byte order mark) laid out to exercise every module path rule (`src/lib.rs`, `src/main.rs`, `src/a/mod.rs`, `src/a/b.rs`, `tests/`, `benches/`, `examples/`, `build.rs`)
- [X] T014 [P] [US1] Write unit tests for module paths from file paths and for impl and method paths in `crates/adapters/gedo-parse/src/rust/paths.rs` (research R7)
- [X] T015 [US1] Implement language detection, the size limit and the parse call with the timeout in `crates/adapters/gedo-parse/src/lib.rs` (`TreeSitterParser`, byte order mark skipped, ranges against the original bytes)
- [X] T016 [US1] Implement the iterative walk producing tokens, entities, containers, body starts and occurrence indices, with the depth limit, in `crates/adapters/gedo-parse/src/walk.rs`
- [X] T017 [P] [US1] Implement the Rust token classification in `crates/adapters/gedo-parse/src/rust/tokens.rs` and the node-to-kind mapping in `src/rust/kinds.rs` (research R7 table)
- [X] T018 [US1] Implement Rust module, impl and method paths in `crates/adapters/gedo-parse/src/rust/paths.rs`; T014 passes
- [X] T019 [US1] Write `crates/adapters/gedo-parse/tests/conformance.rs`: parse and normalize every fixture and assert `insta` snapshots in the format of `contracts/parser-port.md`; review and commit the snapshots; assert `UnsupportedLanguage` for other extensions
- [X] T020 [P] [US1] Write `crates/adapters/gedo-parse/examples/syntax_map.rs` (prints one line per entity, quickstart step 2)

**Checkpoint**: User Story 1 works alone

---

## Phase 4: User Story 2 - Formatting and comments never change an entity's hash (Priority: P1)

**Goal**: formatter runs, line endings, indentation and comments leave hashes unchanged; meaning changes do not

**Independent Test**: `tests/formatting.rs` and `tests/changes.rs`

- [X] T021 [US2] Implement `cargo xtask format-corpus` in `tools/xtask/src/format_corpus.rs`: runs `rustfmt` with the configurations of research R5 on the fixtures and on a copy of the workspace sources, writes the variants to `crates/adapters/gedo-parse/tests/fixtures/rust/formatted/`; generate and commit them
- [X] T022 [P] [US2] Write `crates/adapters/gedo-parse/tests/formatting.rs`: every entity keeps its three hashes across the formatter variants, CRLF conversion, re-indentation and rewritten comments and doc comments (SC-002)
- [X] T023 [P] [US2] Write `crates/adapters/gedo-parse/tests/changes.rs`: one significant token changed per case for every token class, swapped statements, changed attributes; the entity and its containers change hash and its siblings keep theirs (SC-003)
- [X] T024 [US2] Fix the Rust token classification until T022 and T023 pass (separators that the formatter touches classified as `Separator`, delimiters as `OpenDelimiter` and `CloseDelimiter`, `use` declarations as `Import`)

**Checkpoint**: User Stories 1 and 2 work

---

## Phase 5: User Story 3 - The same file gives the same result everywhere (Priority: P1)

**Goal**: identical results on Windows and Linux, across threads and orders

**Independent Test**: the snapshots of T019 on both CI platforms and `tests/determinism.rs`

- [X] T025 [P] [US3] Write `crates/adapters/gedo-parse/tests/determinism.rs`: all fixtures parsed on 8 threads in a seeded random order, twice, equal to a sequential run; the snapshot hashes include `hash_format`
- [ ] T026 [US3] Run the conformance suite on the Windows and Linux CI jobs and fix any platform difference (paths with `\`, line endings, ordering)

**Checkpoint**: User Stories 1 to 3 work

---

## Phase 6: User Story 4 - Code that is being edited still yields its entities (Priority: P1)

**Goal**: syntax errors stay local; limits and pathological inputs are refused cleanly

**Independent Test**: `tests/errors.rs` and `tests/properties.rs`

- [X] T027 [P] [US4] Write `crates/adapters/gedo-parse/tests/errors.rs`: the error-injection corpus of research R8 (at least 95% of entities outside the damaged region keep kind, path and hashes, SC-005; damaged entities have `has_error`), empty, truncated, binary and invalid UTF-8 files, and `TooLarge`, `TooDeep` and `Timeout` with their `tracing` events
- [X] T028 [P] [US4] Write `crates/adapters/gedo-parse/tests/properties.rs`: arbitrary bytes and arbitrary token soups never panic and stay within the limits
- [X] T029 [US4] Implement the error flags and the `tracing` events for refused files and grammar errors in `crates/adapters/gedo-parse/src/walk.rs` and `src/lib.rs`; T027 and T028 pass
- [X] T030 [P] [US4] Add the fuzz target `fuzz/fuzz_targets/parse_rust.rs` (`cargo-fuzz`, outside the workspace) and the manual workflow `.github/workflows/fuzz.yml` (10 minutes on Linux, nightly); run it once and record the result in the plan envelope (SC-007)

**Checkpoint**: User Stories 1 to 4 work

---

## Phase 7: User Story 5 - Rust is declared supported only after its conformance suite passes (Priority: P2)

**Goal**: the real-world sweep and the benchmark close the Rust support

**Independent Test**: `cargo xtask parse-sweep` and `cargo bench -p gedo-parse --bench parse_rust`

- [X] T031 [US5] Implement `cargo xtask parse-sweep` in `tools/xtask/src/parse_sweep.rs`: reads the sources of every package in `cargo metadata` (workspace and locked dependencies), parses every `.rs` file, reports files, lines, crashes and files with syntax errors, and fails on any crash or on an error file not listed in `crates/adapters/gedo-parse/tests/known_grammar_gaps.toml`
- [X] T032 [US5] Write `known_grammar_gaps.toml` with the gaps found (starting with `unsafe extern` blocks, research R2), add a backlog item per gap in `docs/BACKLOG.md`, and run the sweep in the Linux test job of `.github/workflows/ci.yml` after `cargo fetch` (SC-006)
- [X] T033 [P] [US5] Write the benchmark `crates/adapters/gedo-parse/benches/parse_rust.rs` over a 2,000-line fixture (parse, normalization and hash) and add it to the bench job of `.github/workflows/ci.yml` (SC-001)

**Checkpoint**: all user stories work

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T034 [P] Edit `docs/architecture/parsing.md` in place: Rust mapping, token classes, qualified path rules, limits, error tolerance, known grammar gaps
- [ ] T035 [P] Edit `docs/architecture/identity.md` (normalization rules and hash format in `gedo-ast`) and `docs/architecture/core.md` (syntax model types, `Parser` port, `FixedParser`) in place
- [ ] T036 [P] Update `docs/product/TECH_SPEC.md` (library table: `insta`, `cargo-fuzz`) and `README.md` (C compiler prerequisite, status); regenerate the constitution if its sources changed
- [ ] T037 Run `architecture-docs/build_index.py`, `architecture-docs/check_docs.py`, `software-design/check_design.py`, `user-docs/check_readme.py`, all tests, `cargo clippy` and `cargo deny check`; fix everything reported
- [ ] T038 Walk through `quickstart.md` steps 1 to 6 and fix any drift between the guide and the code
- [ ] T039 Run `cargo bench -p gedo-parse --bench parse_rust` on the reference machine on Windows and Linux and record p50, p95 and p99 in `docs/plans/gedo_syntax_model.md` (SC-001: p95 under 10 ms)

---

## Dependencies & Execution Order

- **Setup (T001–T005)** → **Foundational (T006–T012)** → stories.
- **US1 (T013–T020)** first; **US2 (T021–T024)** and **US3 (T025–T026)** depend on US1; **US4 (T027–T030)** depends on US1 and can run in parallel with US2 and US3; **US5 (T031–T033)** depends on US1 and US4.
- **Polish (T034–T039)** after all stories.

### Parallel Opportunities

- Setup: T002 to T004 in parallel after T001.
- Foundational: T006 and T007 in parallel; T008 and T009 after them; T010 before T011 and T012.
- US1: T013, T014 and T017 in parallel; T020 after T019.
- US2's tests T022 and T023 in parallel; US4's T027, T028 and T030 in parallel.
- Polish: T034 to T036 in parallel.

## Implementation Strategy

1. Setup and Foundational, then **US1** (MVP slice: entities of a Rust file) and validate it alone.
2. US2 and US3 make the hashes trustworthy for identity; US4 makes them usable on code being edited.
3. US5 closes the Rust support with the sweep and the benchmark.
4. Each checkpoint maps to a plan milestone: M1 Setup and core model, M2 Rust entities, M3 Formatting invariance, M4 Determinism, M5 Error tolerance, M6 Rust support, M7 Documentation and acceptance.
