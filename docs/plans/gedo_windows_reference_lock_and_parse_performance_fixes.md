---
plan_id: gedo_windows_reference_lock_and_parse_performance_fixes
project: gedo
title: Windows reference lock and parse performance fixes
kind: bug
version: v0.2.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-25T13:31:27'
updated_at: '2026-10-25T13:43:06'
session_id: null
tags:
- windows
- store
- parsing
- performance
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- BUG-001
- BUG-008
milestones:
- name: M1 Reference lock
  done: true
  done_at: '2026-10-25'
  by: Levi Silveira
- name: M2 Kind table
  done: false
- name: M3 Allocations
  done: false
- name: M4 Reference machine and documentation
  done: false
status_log:
- at: '2026-10-25T13:31:27'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-25T13:40:24'
  by: Levi Silveira
  status: draft
  note: Plan updated
- at: '2026-10-25T13:41:27'
  by: Levi Silveira
  status: active
  note: Approved by Levi on 2026-10-25
- at: '2026-10-25T13:43:06'
  by: Levi Silveira
  note: 'Milestone ''M1 Reference lock'' completed: is_busy_lock with unit tests; store.md updated'
---

# Windows reference lock and parse performance fixes

Bug plan for the two open Windows bugs, fixed before the next feature (v0.3.0, event log) so that its pull requests start from a green, stable Windows CI. Both items are `medium`; Levi chose to fix them first on 2026-10-25.

## Backlog items

### BUG-001: Rust parse benchmark over 10 ms p95 on Windows
- **Severity**: medium
- **Found**: 2026-10-24, PR #3 CI bench, windows-latest
- **Description**: parse_rust (2,000 lines) measured p95 14.1 ms on GitHub Actions windows-latest against 8.0 ms on ubuntu-latest and 8.3 ms in a 4-vCPU Linux container (SC-001 of specs/002-syntax-model: under 10 ms on the reference machine). On Linux, Tree-sitter's own parse is about 3.3 ms of 8 ms; the walk and normalization (many small String allocations) take the rest, which points at the Windows allocator. Measure on the reference machine first, then reduce allocations in the walk and normalization if it misses.

### BUG-008: Reference update fails on Windows when a concurrent writer's lock file is being deleted
- **Severity**: medium
- **Found**: 2026-10-25, PR #7 CI (test windows-latest)
- **Description**: cas_stress failed once on windows-latest: gix reports 'Another IO error occurred while obtaining the lock: Access is denied' on .git/refs/gedo/<name>.lock. Windows refuses to create a file whose previous instance is pending deletion by another process, and gix does not treat that PermissionDenied as retryable, so compare_and_swap returns a backend error instead of waiting or returning Contended. Fix: in gedo-store refs.rs, treat a PermissionDenied I/O error while acquiring the lock as a busy lock on Windows (retry within REF_LOCK_WAIT_MS, then Contended).

## 1. Triage

| Item | Expected (source) | Actual | Verdict | Severity |
|---|---|---|---|---|
| BUG-008 | A reference update whose lock stays busy for the whole wait returns `Contended` and writes nothing (`docs/architecture/store.md`, "Reference updates wait up to 2 s ... then return `Contended`") | On Windows it can return a `Backend` error instead, which callers do not retry | bug | medium (confirmed): CI flake on a required check, no data is lost |
| BUG-001 | Parse, normalize and hash of 2,000 lines under 10 ms p95 on the reference machine (SC-001 of `specs/002-syntax-model`) | 14.1 ms (PR #3) and 15.6 ms (`main` at 6d2f534) p95 on windows-latest; never measured on the reference machine | bug, to be confirmed on the reference machine | medium (confirmed): CI runners are not the reference machine and the bench job is not blocking |

Suspected code: `crates/adapters/gedo-store/src/refs.rs` (`GitRefStore::apply` and `outcome`), `crates/adapters/gedo-parse/src/walk.rs` and `src/rust/{kinds,tokens,paths}.rs` (node kind lookups), `crates/core/gedo-ast/src/normalize.rs` (one `String` per token).

## 2. Reproduction

- BUG-008: `cargo test -p gedo-store --test cas_stress` on windows-latest, CI run 40180124844 attempt 1 (PR #7, job 121129441805); the re-run passed. The log:
  ```
  Caused by:
      0: Another IO error occurred while obtaining the lock
      1: I/O error (PermissionDenied)
      2: Access is denied. (os error 5) at path "C:\...\work\.git\refs\gedo\stress.lock"
  cas_stress::write_updates at .\tests\cas_stress.rs:56
  ```
  It is a race: it needs 8 writers in 2 processes and a slow file system; it has not been seen on Linux.
- BUG-001: the non-blocking `bench (windows-latest)` job of every CI run. Latest, `main` at 6d2f534 (run 40197160660): `parse_rust 2000 lines over 500 runs: p50 14.2244ms, p95 15.5843ms, p99 18.8638ms`; the same run on ubuntu-latest stays near 8 ms.

## 3. Evidence and root cause

- **BUG-008 (confirmed, more precise than the backlog text)**: `gix-lock` 26.0.1 (`acquire.rs`, `lock_with_mode`) already retries `PermissionDenied` on Windows while the wait lasts. When the 2 s wait runs out it makes one last attempt, and only an `AlreadyExists` failure of that attempt is classified `Retryable`; any other kind becomes "Another IO error occurred while obtaining the lock". So when the lock stays busy for the whole wait (the situation the stress test already handles as `Contended`, see its comment) and the last attempt meets a lock file that another process is deleting, Windows answers `PermissionDenied` and `GitRefStore::outcome` reports `Backend` instead of `Contended`. Nothing was written: the failure is in `prepare`, before any lock is held.
- **BUG-001 (cause confirmed on Linux, Windows share to be measured)**: instruction counts with `valgrind --tool=callgrind` on one parse, normalize and hash of the benchmark input (deterministic, unlike timings in a shared container):

  | Stage | Instructions | Share |
  |---|---|---|
  | Tree-sitter parse | 75 M | 40% |
  | Walk (`walk::collect`) | 76 M | 40% |
  | Normalize and hash (`gedo_ast::normalize`) | 35 M | 19% |

  Half of the walk, 38 M instructions, is `tree_sitter::Node::kind()`: the binding measures the C string of the kind name and validates it as UTF-8 on every call (`CStr::from_ptr(..).to_str()`), and the walk calls it from 24 places in `walk.rs` and `rust/{kinds,tokens,paths}.rs`, several times per node. This is independent of the allocator. Allocations are the second cost: 4,716 in the walk and 20,321 in normalize (one `String` per token, cloned path strings), about 12% of the instructions in glibc `malloc` and `free` together with Tree-sitter's own; the Windows heap is known to be slower for small blocks, which matches the larger gap on Windows. Hypothesis to confirm: on Windows the same changes give a larger gain than on Linux.

  A prototype of the first fix (below) cut the walk from 76 M to 47 M instructions (-38%) and the whole pipeline from 189 M to 160 M (-15%) with every `gedo-parse` test passing.

## 4. Fix design (`software-design`)

- **Busy lock on Windows (BUG-008)**: in `GitRefStore::apply`, an error from `prepare` (the lock is not held yet, nothing is written) whose I/O cause is `PermissionDenied` is a busy lock on Windows, and `outcome` maps it to `Contended`, like a `Retryable` lock error. The rule lives in one function, `is_busy_lock(error, on_windows)`, called with `cfg!(windows)`, so both branches are unit-tested on every platform. On other platforms `PermissionDenied` stays a `Backend` error: there it means a real permission problem that waiting does not fix. A `PermissionDenied` from `commit` is not touched: by then the edit may be half applied. Alternative rejected: a retry loop around `prepare` in Gedo; `gix-lock` already waits, and a second loop would double the documented 2 s wait.
- **Kind table (BUG-001)**: `rust::kind_of(node)` reads the node's kind from a table indexed by `kind_id()`, built once per process from the grammar (`Language::node_kind_for_id`), and falls back to `Node::kind()` for ids outside the table (the error symbol). Every `.kind()` call of the walk uses it. Behavior is unchanged: `kind_id()` is the same alias-aware symbol `kind()` names. The grammar is loaded once (`rust::language()`) and reused by `parse_tree`.
- **Fewer allocations (BUG-001), only if needed**: after the kind table, measure on Windows CI. If the p95 there is still above 10 ms, remove the per-token `String` of normalization: `NormalizedToken::text` becomes a `Cow<'_, str>` borrowed from the source and owned only when line endings are rewritten. This changes a `gedo-core` type used by `gedo-ast` matching and hashing (not a port and not stored data), so it is a separate milestone with its own measurement. Hashes do not change: the bytes hashed are the same.
- **Reference machine (BUG-001)**: SC-001 names the reference machine of the technical specification (4 cores, SSD), not a CI runner. After the fixes, the benchmark runs on Levi's Windows machine through a Remote Control session (Levi approves the session; the agent runs it). If the p95 there is under 10 ms, BUG-001 closes, with the CI numbers recorded as informational. If it is still over, the options go to Levi as a decision: a faster global allocator in the future binaries (an ADR for FEAT-081, the composition root), or more work on the Tree-sitter share.

Deliberately left out: a global allocator now (there is no binary yet, and a library must not set one), changes to Tree-sitter's own allocations, and the Windows share of the other benchmarks (all within their targets).

## 5. Affected surfaces and files

No surface in `architecture.surfaces` changes, and no port changes. Files: `crates/adapters/gedo-store/src/refs.rs`; `crates/adapters/gedo-parse/src/{lib.rs, walk.rs, rust/mod.rs, rust/kinds.rs, rust/tokens.rs, rust/paths.rs}`; only for M3, `crates/core/gedo-core/src/syntax.rs` (`NormalizedToken`) and `crates/core/gedo-ast/src/{normalize.rs, hash.rs, matching.rs}`. Same defect elsewhere: the WAL lock (`wal.rs`) is an OS file lock (`File::try_lock`), not a lock file, and file operations already retry Windows sharing violations (`fs.rs`); no other `.kind()` call sits on a per-node path.

## 6. Tests (`quality-assurance`)

- BUG-008: unit tests in `refs.rs` for `is_busy_lock` with a `gix::Error` built like the one in the log (an `io::Error` of kind `PermissionDenied` raised with gix's lock message): busy with `on_windows = true`, not busy with `false`; an `AlreadyExists` lock error stays `Contended` and a `ReferenceOutOfDate` stays `Moved`. The unit test fails before the fix (there is no busy rule). `cas_stress` stays the integration evidence on windows-latest.
- BUG-001: no correctness change, so the regression guard is the existing suite (snapshots, formatter corpus, properties, `cargo xtask parse-sweep` with no new syntax errors) plus a unit test that `kind_of` equals `Node::kind()` for every node of the conformance fixture, including a file with a syntax error (the `ERROR` symbol). The benchmark p95 on Linux must not grow; the Windows CI and the reference machine numbers are recorded in this plan.
- No manual test: there is no user surface yet.

## 7. Documentation (`architecture-docs`, `user-docs`)

`docs/architecture/store.md`: the Windows paragraph says that a lock file being deleted by another writer counts as a busy lock. `docs/architecture/parsing.md`: the kind table, if the page describes the walk's lookups. No ADR unless the allocator decision is needed (then for FEAT-081). README: nothing changes for users.

## 8. Release, migration and rollback (`devops-release`)

Part of v0.2.0, not released (BUG-008 touches v0.1.0 code, also unreleased). No data format changes, no stored data affected: the failed update wrote nothing. Rollback is a revert of the pull request.

## 9. Execution steps and milestones

One branch `fix/windows-lock-and-parse` from `main`, one pull request into `main`.

1. M1 Reference lock: BUG-008 fix and its unit tests.
2. M2 Kind table: the `kind_of` table, its equality test, the Linux benchmark and the Windows CI benchmark of the pull request.
3. M3 Allocations (only if the Windows CI p95 is still over 10 ms after M2): borrowed token text, measured again.
4. M4 Reference machine and documentation: the benchmark on Levi's Windows machine, section 7, the checkers; then BUG-001 closes or goes to Levi as a decision.

## Results
