---
plan_id: gedo_syntax_model_review_fixes
project: gedo
title: Syntax model review fixes
kind: bug
version: v0.2.0
status: completed
author: Levi Silveira
created_at: '2026-10-24T20:21:48'
updated_at: '2026-10-24T23:03:41'
session_id: null
tags:
- parsing
- hashing
- review
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- BUG-002
- BUG-003
- BUG-004
- BUG-005
- BUG-006
- BUG-007
- IMP-002
milestones:
- name: M1 Walk performance and deadline
  done: true
  done_at: '2026-10-24'
  by: Levi Silveira
- name: M2 Hash rules
  done: true
  done_at: '2026-10-24'
  by: Levi Silveira
- name: M3 Error flag and paths
  done: true
  done_at: '2026-10-24'
  by: Levi Silveira
- name: M4 Documentation and acceptance
  done: true
  done_at: '2026-10-24'
  by: Levi Silveira
status_log:
- at: '2026-10-24T20:21:48'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-24T20:23:15'
  by: Levi Silveira
  status: draft
  note: Plan updated
- at: '2026-10-24T20:27:40'
  by: Levi Silveira
  status: active
  note: Approved by Levi on 2026-10-24
- at: '2026-10-24T20:36:02'
  by: Levi Silveira
  note: 'Milestone ''M1 Walk performance and deadline'' completed: Regression tests added'
- at: '2026-10-24T20:36:04'
  by: Levi Silveira
  note: 'Milestone ''M2 Hash rules'' completed: Regression tests added'
- at: '2026-10-24T20:36:05'
  by: Levi Silveira
  note: 'Milestone ''M3 Error flag and paths'' completed: Regression tests added'
- at: '2026-10-24T23:03:41'
  by: Levi Silveira
  note: 'Milestone ''M4 Documentation and acceptance'' completed: Docs, checkers, sweep, fuzz and benchmark done'
- at: '2026-10-24T23:03:41'
  by: Levi Silveira
  status: completed
  note: All milestones completed
---

# Syntax model review fixes

Bug plan for the findings of the protocol review of PR #3 ([review](https://github.com/Levix-Digital/gedo/pull/3#pullrequestreview-5639788276)). Feature 002 (`specs/002-syntax-model`) is merged; nothing has stored or released a structural hash yet.

## Backlog items

### BUG-002: Parser walk is quadratic on long comma lists and has no deadline
- **Severity**: high
- **Found**: 2026-10-24, PR #3 review
- **Description**: tokens.rs calls next_sibling() for every comma, which costs O(siblings) in Tree-sitter, so a long list is O(n^2): a 1.6 MB flat array takes 0.9 s to parse and 6.5 s to walk. The 2 s timeout covers only the Tree-sitter parse, so a file near the 16 MiB limit hangs the caller (FR-012, parser contract guarantees 6 and 7). Fix: read the next sibling from the cursor and put the walk under the same deadline; regression test with a large flat array.

### BUG-003: One-element tuple comma dropped by the structural hash
- **Severity**: high
- **Found**: 2026-10-24, PR #3 review
- **Description**: The comma before a closing delimiter is dropped, so (a,) and (a) hash the same, as do (u8,) and (u8) and the pattern (a,). Breaks SC-003 and US2 scenario 4 of specs/002-syntax-model. Fix: the comma of a single-element tuple expression, type or pattern is Punctuation; must land before feature 003 stores hashes.

### BUG-004: Sorting the items of a use list changes the structural hash
- **Severity**: high
- **Found**: 2026-10-24, PR #3 review
- **Description**: Only whole use declarations are order-insensitive; rustfmt with default settings also sorts the items inside use std::{io, fmt}, which changes the hash of the enclosing module or function. Breaks SC-002 and US2 scenario 1, contradicts research R5. Fix: make use-list items order-insensitive and add an unsorted use list to the formatter fixtures; must land before feature 003 stores hashes.

### BUG-005: Syntax error in an attached attribute does not flag the entity
- **Severity**: medium
- **Found**: 2026-10-24, PR #3 review
- **Description**: has_error comes only from the item node, but the entity's tokens include its attached attributes and comments: #[derive(Debug] on a struct leaves the struct unflagged while its hash includes the broken attribute. Breaks parser contract guarantee 5 and FR-011. Fix: OR the error flag of the prefix nodes into the entity.

### BUG-006: Semicolon after a mid-block return is treated as optional
- **Severity**: medium
- **Found**: 2026-10-24, PR #3 review
- **Description**: The ; after return, break or continue is optional anywhere, so fn f() { return; g() } and fn f() { return g() } hash the same. Research R5 justifies it only for a jump that ends a block. Fix: optional only when the statement is the last of its block.

### BUG-007: Wrong module paths in multi-file binaries
- **Severity**: low
- **Found**: 2026-10-24, PR #3 review
- **Description**: src/bin/tool/helper.rs gets crate::bin::tool::helper instead of crate::helper; examples/<dir>/ and benches/<dir>/ crates have the same shape. Paths become identities in feature 003, so fix before it.

### IMP-002: Test the parse timeout path
- **Priority**: low
- **Found**: 2026-10-24, PR #3 review
- **Description**: No test produces ParseError::Timeout or its tracing event, although T027 is ticked as covering it. Make the deadline injectable and test it with a zero duration.

## 1. Triage

| Item | Expected (source) | Actual | Verdict | Severity |
|---|---|---|---|---|
| BUG-002 | Parsing never hangs; `Timeout` after 2 s (FR-012, contract guarantees 6 and 7) | The walk after the parse is quadratic on long comma lists and has no deadline | bug | high (confirmed) |
| BUG-003 | A significant change changes the hash (SC-003, US2 scenario 4) | `(a,)` and `(a)` hash the same | bug | high (confirmed) |
| BUG-004 | A default `rustfmt` run keeps every hash (SC-002, US2 scenario 1, research R5) | Sorting the items of a `use` list changes the hash of the enclosing module or function | bug | high (confirmed) |
| BUG-005 | `has_error` is set on an entity if and only if its tokens contain a syntax error (contract guarantee 5, FR-011) | An error in an attached attribute does not flag the entity | bug | medium (confirmed) |
| BUG-006 | Only the `;` a formatter may add or remove is optional (research R5) | `return; g()` and `return g()` hash the same | bug | medium (confirmed) |
| BUG-007 | Qualified paths follow the module tree (research R7) | `src/bin/tool/helper.rs` is `crate::bin::tool::helper` instead of `crate::helper` | bug | low (confirmed) |
| IMP-002 | T027 covers `Timeout` and its event | No test produces `ParseError::Timeout` | improvement | low |

Suspected code: `crates/adapters/gedo-parse/src/rust/tokens.rs` (`classify`: comma and `;` rules), `src/walk.rs` (prefix error flag, walk loop), `src/rust/kinds.rs` (`entity_kind`), `src/rust/paths.rs` (`module_path`), `src/lib.rs` (`parse_tree` deadline).

## 2. Reproduction

On `main` at df7c38a, `cargo run --release -p gedo-parse --example syntax_map -- <file>`:

- BUG-002: `const A: [u8; 0] = [0,0,...];` with 100,000, 200,000 and 400,000 elements takes 0.51 s, 1.35 s and 3.28 s (each doubling costs about 2.5 times). 400,000 elements is 0.8 MB, far below the 16 MiB limit, and already over the 2 s timeout without a `Timeout` error.
- BUG-003: `fn t() { let x = (a,); }` and `fn t() { let x = (a); }` both give `ad99520e…`.
- BUG-004: `mod m { use std::{io, fmt}; fn f() {} }` gives `caa0bc7e…` for `crate::m`, the sorted `use std::{fmt, io};` gives `d340b5c0…`.
- BUG-005: `#[derive(Debug]` then `struct A;`: `A` is printed without `(syntax error)`.
- BUG-006: `fn f() { return; g() }` and `fn f() { return g() }` both give `523d0e8f…`.
- BUG-007: `src/bin/tool/helper.rs` with `fn h() {}` gives `crate::bin::tool::helper::h`.

## 3. Evidence and root cause

- BUG-002 (confirmed): `classify` calls `node.next_sibling()` for every `,`. Tree-sitter answers it by walking the parent's children from the first one, so a list of n commas costs O(n²). The walk already avoids the same trap for `parent()` with an ancestor stack. The 2 s deadline lives in the progress callback of `parse_tree` only; `walk::collect` runs after it with no limit.
- BUG-003 (confirmed): every `,` is a `Separator`, and normalization drops a separator directly before a closing delimiter (R5 rule 1). In a one-element tuple the comma is what makes it a tuple.
- BUG-004 (confirmed): R5 rule 2 makes whole `use` declarations order-insensitive, but the items inside a `use_list` stay in the import's token stream in source order. `rustfmt` sorts them by default (`reorder_imports`); the formatter corpus did not catch it because every fixture list was already sorted.
- BUG-005 (confirmed): `open_entity` sets `has_error: node.has_error()` from the item node only, while the entity's token range starts at the prefix of attached attributes and comments.
- BUG-006 (confirmed): the rule "the `;` of a jump statement is optional" ignores where the statement is. Only at the end of a block does a formatter add or remove it; in the middle, the `;` decides whether the following expression is the returned value.
- BUG-007 (confirmed): `module_path` treats `src/bin/<dir>/main.rs` as a crate root but keeps `bin/<dir>` as modules for its sibling files. Cargo always builds `src/bin/<dir>/` as one binary, so its other files are modules of that binary. For `tests/`, `benches/` and `examples/` a subdirectory is either a multi-file target (with `main.rs`) or shared modules (`tests/common/mod.rs`), which a single file cannot tell apart; those keep today's rule.
- IMP-002: the deadline is a constant inside `parse_tree`, so no test can trigger it on a small input.

## 4. Fix design (`software-design`)

All changes stay in the adapter `gedo-parse`, except the documentation of the hash rules. The core model and the `Parser` port do not change.

- **Last-child facts from the cursor (BUG-002, BUG-006)**: the walk loop already learns, at no cost, whether a node has a next sibling (`goto_next_sibling` fails). It reports that to the collector (`Collector::leave(node, is_last)`), which reclassifies a `,` that was the last child of its parent as `OptionalPunctuation`. `classify` no longer calls `next_sibling()`. For BUG-006 the `;` of a jump statement is first classified `Punctuation`; the collector keeps it pending and makes it `OptionalPunctuation` when the enclosing block ends with no other code after the statement (comments and the closing `}` do not count).
- **Walk deadline (BUG-002, IMP-002)**: one deadline, computed once before the parse, is shared by `parse_tree` and `walk::collect`. The walk checks the clock every 4,096 nodes and returns `ParseError::Timeout` with the same limit. `TreeSitterParser::with_timeout(Duration)` builds a parser with another limit; `new()` keeps 2 s. This is the only interface change, on the adapter, not the port.
- **One-element tuples (BUG-003)**: in `classify`, the `,` of a `tuple_expression`, `tuple_type` or `tuple_pattern` with exactly one element (comments aside) is `Punctuation`, so normalization keeps it. Tuple struct patterns and calls (`Foo(a,)`) keep the trailing-comma rule.
- **Use-list items (BUG-004)**: each item of a `use_list` (named children other than comments) becomes an `Import` entity nested in its `use` declaration. Normalization already takes `Import` children out of the token stream and hashes them sorted (R5 rule 2), so their order no longer counts, and nested lists (`std::{io::{self, Read}, fmt}`) work recursively. Imports never reach the syntax map, so no visible entity changes. Alternative rejected: sorting the comma-separated items inside `gedo-ast`, which would make the neutral core understand one language's import syntax.
- **Prefix error flag (BUG-005)**: `Prefix` gets `has_error`, ORed from every attached node; the entity's flag is `node.has_error() || prefix.has_error`.
- **Binary module paths (BUG-007)**: in `module_path`, below `src/`, a path `bin/<dir>/<rest…>` drops `bin/<dir>`.
- **Hash format**: the fixes change hashes (tuples, use lists, mid-block jumps), so research R6 asks for `HashFormat::V2`. **Decision**: keep `V1` and change its definition, because no hash has been stored or released (feature 003 is the first to store them, v0.2.0 is not released). The snapshots and the golden hash change in this pull request. The R6 rule applies from the first stored hash on.

Deliberately left out: the Windows benchmark (BUG-001, it needs the reference machine), derive merging and `>>` splitting in macros (residual differences documented in R5), and the `tests/`, `benches/`, `examples/` subdirectory ambiguity.

## 5. Affected surfaces and files

No surface in `architecture.surfaces` changes (there is no CLI or daemon yet). Files: `crates/adapters/gedo-parse/src/{lib.rs, walk.rs, rust/tokens.rs, rust/kinds.rs, rust/paths.rs}`, the snapshots in `crates/adapters/gedo-parse/tests/snapshots/`, the fixtures and the regenerated formatter corpus, `crates/core/gedo-ast/tests/properties.rs` (golden hash) only if its synthetic input changes. Same defect elsewhere: no other `next_sibling()` or `parent()` call sits on a per-token path (`kinds::body_node` calls `next_sibling()` once per macro definition).

## 6. Tests (`quality-assurance`)

Each regression test fails on `main` and passes after its fix:

- BUG-002: a flat array of 400,000 elements parses in under the timeout (`tests/errors.rs`); a unit test of `Collector` checks that a last-child comma is optional and a middle one is a separator.
- BUG-003: `(a,)`/`(a)`, `(u8,)`/`(u8)` and `let (a,) = t;`/`let (a) = t;` hash differently (`tests/changes.rs`).
- BUG-004: an unsorted use list, nested lists and lists with comments hash like their sorted versions (`tests/formatting.rs`); a fixture with unsorted lists is added and the `rustfmt` corpus regenerated with `cargo xtask format-corpus`.
- BUG-005: `#[derive(Debug]` before a struct flags the struct (`tests/errors.rs`).
- BUG-006: `return; g()`/`return g()` and `break; x`/`break x` hash differently; `return;`/`return` at the end of a block still hash the same (`tests/changes.rs`, `tests/formatting.rs`).
- BUG-007: `src/bin/tool/helper.rs` gives `crate::helper` (unit test in `paths.rs`).
- IMP-002: `TreeSitterParser::with_timeout(Duration::ZERO)` returns `Timeout` and emits the `file refused` warning (`tests/errors.rs`, `tests/tracing_events.rs`).

Then the whole suite, `cargo xtask parse-sweep` (no new syntax errors), the benchmark (p95 must not grow on Linux) and the fuzz target for 10 minutes. No manual test: there is no user surface yet.

## 7. Documentation (`architecture-docs`, `user-docs`)

`docs/architecture/parsing.md` (token rules, use-list items, binary paths, deadline over parse and walk), `docs/architecture/identity.md` (use-list order), `specs/002-syntax-model/research.md` R5, R6, R7 and R8 and `contracts/parser-port.md` (timeout covers the walk). No ADR: the decisions stay inside research R5 to R8. README: nothing changes for users.

## 8. Release, migration and rollback (`devops-release`)

Part of v0.2.0, not released. No migration: no hash or path has been stored. Rollback is a revert of the pull request, which brings back the old hash rules; nothing depends on them.

## 9. Execution steps and milestones

One branch `fix/syntax-model-review` from `main`, one pull request into `main`.

1. M1 Walk performance and deadline: BUG-002 and IMP-002.
2. M2 Hash rules: BUG-003, BUG-004 and BUG-006, with the snapshots and the formatter corpus regenerated.
3. M3 Error flag and paths: BUG-005 and BUG-007.
4. M4 Documentation and acceptance: section 7, the checkers, the sweep, the benchmark and the fuzz run.

## Results

- Regression tests: each new test failed on `main` and passes with the fixes (`tests/changes.rs`, `tests/formatting.rs`, `tests/errors.rs`, `tests/tracing_events.rs`, unit tests in `walk.rs` and `paths.rs`). The walk test (`walk::tests::the_walk_takes_linear_time`) walks 6,250 and 400,000 elements: 64 times the size takes about 66 times as long, against about 170 times with the per-comma `next_sibling()` call put back.
- The protocol review of this pull request found that a single-item `use` list (`std::{io}`, which `rustfmt` writes `std::io`) still changed the hash, and that the tuple check gave up on tuples with several comments; both are fixed with tests, and the residual macro case is documented in research R5.
- New fixture `src/review.rs` (unsorted use lists, one-element tuples, jump statements) with its snapshot and its `rustfmt` variants; `formatter_variants_keep_every_hash` passes on it.
- `cargo xtask parse-sweep`: 238 packages, 8,602 files, 3,342,061 lines; 0 crashes, 0 refused, 1,517 files with syntax errors, all known grammar gaps (unchanged).
- Fuzzing, `cargo +nightly fuzz run parse_rust -- -max_total_time=600`, Linux container, 4 vCPUs: 238,902 runs in 601 s, no crash, hang or out-of-memory.
- `parse_rust` benchmark, same container, back to back: `main` p50 7.9 ms, p95 8.3 ms, p99 9.0 ms; this branch p50 8.3 ms, p95 8.5 ms, p99 9.1 ms. About 3% slower (the walk now tracks last children and pending jumps, and use-list items are entities), still under the 10 ms of SC-001 on Linux; BUG-001 tracks Windows.
