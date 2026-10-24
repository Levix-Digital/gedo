---

description: "Tasks for the store foundation (v0.1.0)"
---

# Tasks: Store Foundation

**Input**: Design documents from `/specs/001-store-foundation/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ports.md, contracts/repository-layout.md, quickstart.md

**Tests**: required. The constitution (IV.2 Evidence) and the plan's test section ask for automated proof of every story; tests are written before or with the code they cover and must pass before a story is closed.

**Organization**: grouped by user story (spec.md), in priority order. Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US7 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: workspace, toolchain and quality tooling

- [X] T001 Create the workspace manifest `Cargo.toml` with members `crates/core/gedo-core`, `crates/adapters/gedo-store`, `tools/xtask`, `edition = "2024"`, `rust-version = "1.95"`, `license = "MIT OR Apache-2.0"`, version `0.1.0`, and `[workspace.dependencies]` pinning `gix = { version = "0.90", default-features = false }`, `redb = "4.4"`, `postcard = { version = "1.1", features = ["alloc"] }`, `serde = { version = "1.0", features = ["derive"] }`, `crc32fast = "1.5"`, `thiserror = "2.0"`, `tracing = "0.1"`, `proptest = "1.12"`, `tempfile = "3.27"`, `criterion = "0.8"`, `cargo_metadata = "0.23"`
- [X] T002 [P] Create the crate `crates/core/gedo-core` (`Cargo.toml` with features `memory` and `contract-tests`, empty `src/lib.rs` with `#![forbid(unsafe_code)]` and crate docs)
- [X] T003 [P] Create the crate `crates/adapters/gedo-store` (`Cargo.toml` depending on `gedo-core`, `gix`, `redb`, `postcard`, `crc32fast`, `thiserror`, `tracing`; dev-dependencies `gedo-core` with `memory` and `contract-tests`, `proptest`, `tempfile`, `criterion`; empty `src/lib.rs`)
- [X] T004 [P] Create `tools/xtask` (binary crate, `cargo_metadata` dependency) and the alias `xtask = "run --package xtask --"` in `.cargo/config.toml`
- [X] T005 [P] Add `rustfmt.toml` (edition 2024, max width 110) and workspace lints in `Cargo.toml` (`unsafe_code = "forbid"`, clippy `unwrap_used = "deny"` outside tests)
- [X] T006 [P] Add `deny.toml`: licenses allowed `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`, `MPL-2.0` only if a dependency needs it (document why); advisories deny; bans warn on duplicates
- [X] T007 Verify the setup: `cargo build --workspace` and `cargo fmt --check` pass on the empty crates

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: core types, errors, records, ports, in-memory adapters and the contract suite every story builds on

**⚠️ CRITICAL**: no user story starts before this phase is complete

- [X] T008 [P] Implement `ObjectHash` (`Sha1`, `Sha256`), `GitOid` (hash kind + bytes, 20 or 32, lowercase hex parse and display, ordering by bytes), `EntryId` (lowercase hex) and `GedoRefName` (only `refs/gedo/` names; refuse `..`, uppercase and characters outside `[a-z0-9-/]`) in `crates/core/gedo-core/src/ids.rs`, with unit and property tests
- [X] T009 [P] Implement `StoreError` (`UnsupportedObjectFormat`, `NotInGedoNamespace { name }`, `ObjectNotFound { oid }`, `MissingIdentity`, `WalBusy`, `Corrupted { path, offset }`, `UnsupportedFormat { found, supported }`, `TooLarge { size, limit }`, `Io { operation, path, source }`, `Backend { operation, message }`) with `thiserror` in `crates/core/gedo-core/src/error.rs`
- [X] T010 [P] Implement `RecordEnvelope { format: u16, payload }` with `postcard` encode and decode, `MAX_RECORD_BYTES = 64 * 1024 * 1024` checked before decoding, unknown format → `UnsupportedFormat`, never panicking on malformed input, in `crates/core/gedo-core/src/record.rs`, with property tests for round trip and arbitrary bytes
- [X] T011 Define the port traits `ObjectStore`, `RefStore` (with `RefUpdate { expected: Absent | Value(GitOid), new, log_message }` and `RefOutcome::{Updated, Moved { current }}`), `WriteAheadLog` (with `LogSequence`, `OpenReport`), `SymbolCache` and `CacheRebuildSource` exactly as in `contracts/ports.md`, in `crates/core/gedo-core/src/ports/{object.rs,refs.rs,wal.rs,cache.rs}` and `src/ports/mod.rs`
- [X] T012 Implement the in-memory adapters (`MemoryObjectStore`, `MemoryRefStore` with atomic compare-and-swap and a reflog, `MemoryWal` with simulated durability, `MemoryCache`) behind the `memory` feature in `crates/core/gedo-core/src/memory/`
- [X] T013 Write the shared contract suite (macro or generic functions taking a factory) for the four ports behind the `contract-tests` feature in `crates/core/gedo-core/src/contract_tests.rs`, and run it against the in-memory adapters in `crates/core/gedo-core/tests/memory_contract.rs`
- [X] T014 Implement repository opening in `crates/adapters/gedo-store/src/repository.rs`: open with `gix` (worktrees and bare repositories included), refuse non-SHA-1 object formats with `UnsupportedObjectFormat`, resolve `.git/gedo/` (create on demand, race-safe), read the user identity (`MissingIdentity` when absent)

**Checkpoint**: core and repository access ready

---

## Phase 3: User Story 1 - Store and read Gedo data inside a Git repository (Priority: P1) 🎯 MVP

**Goal**: blobs, trees and commits written and read without touching the working tree, index or branches

**Independent Test**: invariance tests in `crates/adapters/gedo-store/tests/invariance.rs`

### Tests for User Story 1

- [X] T015 [P] [US1] Write invariance tests: repositories with uncommitted edits, staged files, detached `HEAD`, a worktree and a bare repository; after every store operation `git status --porcelain`, the index file, `HEAD` and `refs/heads/*` are byte-identical, and `git fsck` is clean, in `crates/adapters/gedo-store/tests/invariance.rs`
- [X] T016 [P] [US1] Write format tests: SHA-256 repository refused, missing identity refused before a commit is written, in `crates/adapters/gedo-store/tests/formats.rs`

### Implementation for User Story 1

- [X] T017 [US1] Implement `GixObjectStore` (`write_blob`, `read_blob` with the 64 MiB limit, `write_tree` with Git ordering, `read_tree`, `write_commit` with the repository identity, `read_commit`) in `crates/adapters/gedo-store/src/objects.rs`; no `git` subprocess
- [X] T018 [US1] Run the contract suite against `GixObjectStore` in `crates/adapters/gedo-store/tests/contract.rs`
- [X] T019 [US1] Add the example `crates/adapters/gedo-store/examples/roundtrip.rs` (writes a chain entry in a given repository and prints `refs/gedo` contents) used by quickstart step 7

**Checkpoint**: US1 passes on its own

---

## Phase 4: User Story 2 - Concurrent writers never lose an update (Priority: P1)

**Goal**: compare-and-swap references under `refs/gedo/` and commit-chain collections

**Independent Test**: `crates/adapters/gedo-store/tests/cas_stress.rs`

### Tests for User Story 2

- [X] T020 [P] [US2] Write the stress test: 8 threads and 2 processes (the test binary re-invoked with an environment variable) perform 10,000 compare-and-swap updates on one reference; zero lost updates, every `Moved` carries the current value, the reflog lists every value, and collection `put` retries emit a `tracing` event with the attempt count, in `crates/adapters/gedo-store/tests/cas_stress.rs`
- [X] T021 [P] [US2] Write property tests for collections on the in-memory adapters: fan-out paths unique and valid on case-insensitive file systems; union merge commutative, associative and idempotent; `put` retries on `Moved` up to 8 attempts, in `crates/core/gedo-core/tests/collection.rs`

### Implementation for User Story 2

- [X] T022 [US2] Implement `GixRefStore` (`read`, `list`, `list_remote` over `refs/gedo-remote/<remote>/*`, `compare_and_swap` with `PreviousValue::MustNotExist` / `MustExistAndMatch` and reflog messages `gedo: <operation>`, `delete` with an expected value; idempotent create of the same value is `Updated`) in `crates/adapters/gedo-store/src/refs.rs`
- [X] T023 [US2] Implement `Collection` (two-level fan-out tree by `EntryId`, `get`, `put` writing blobs, tree and a commit with the head as parent then compare-and-swap with up to 8 retries, `merge_remote` writing a union merge commit; same id with different content → `Corrupted`) in `crates/core/gedo-core/src/collection.rs`
- [X] T024 [US2] Extend the contract tests in `crates/adapters/gedo-store/tests/contract.rs` to `GixRefStore` and run the collection tests of T021 against the real adapters too

**Checkpoint**: US1 and US2 pass

---

## Phase 5: User Story 3 - Local work survives crashes (Priority: P1)

**Goal**: crash-safe WAL with 100 ms group commit and a single writer

**Independent Test**: `crates/adapters/gedo-store/tests/wal_torn.rs`

### Tests for User Story 3

- [X] T025 [P] [US3] Write torn and corruption tests: truncate the last record at every byte offset, flip bytes inside earlier records, truncate the header; open returns every good record, drops and reports a torn tail, returns `Corrupted { offset }` for other damage; simulate a failing write in the middle of an append (disk full, through a writer that errors after N bytes) and check the partial record is never returned and earlier records stay intact, in `crates/adapters/gedo-store/tests/wal_torn.rs`
- [X] T026 [P] [US3] Write durability and lock tests: an appended record is durable within 100 ms (observed through a second handle after the window), `sync` is immediate, a second writer gets `WalBusy` after at most 2 s and succeeds once the first is dropped, a crashed writer process leaves no stale lock, the lock wait emits a `tracing` event; on Windows only, a log file held open by another handle without sharing is retried for up to 2 s and then fails with context, in `crates/adapters/gedo-store/tests/wal_durability.rs`
- [X] T027 [P] [US3] Write the benchmark `wal_append` (1 KiB payloads, report p50, p95, p99) in `crates/adapters/gedo-store/benches/wal_append.rs`

### Implementation for User Story 3

- [X] T028 [US3] Implement `FileWal` in `crates/adapters/gedo-store/src/wal.rs`: header magic `GEDOWAL\0` and file format `u16 LE` = 1; frames `length u32 LE | crc32 u32 LE (crc32fast over version and payload) | version u16 LE | payload`; exclusive `std::fs::File::lock` on `.git/gedo/wal.lock` with a 2 s bounded wait; flusher thread calling `sync_data` when unsynced records are older than 100 ms; `sync`; `read_all`; torn-tail truncation on open with a `tracing` warning; `truncate_through` via temporary file and atomic rename; Windows sharing-violation retries up to 2 s
- [X] T029 [US3] Run the WAL contract tests against `FileWal` in `crates/adapters/gedo-store/tests/contract.rs`

**Checkpoint**: US1 to US3 pass

---

## Phase 6: User Story 5 - Gedo data travels with plain Git push and fetch (Priority: P1)

**Goal**: remote setup and validated transport without changing branch push behavior

**Independent Test**: `crates/adapters/gedo-store/tests/transport.rs` with file and SSH remotes

### Tests for User Story 5

- [X] T030 [P] [US5] Write transport tests with a file remote (Windows and Linux) and an SSH remote on `localhost` with a generated key (Linux, `#[ignore]` unless `GEDO_TEST_SSH=1`): push of `refs/gedo/*:refs/gedo/*`, fresh clone plus setup plus fetch yields the same chains in `refs/gedo-remote/<remote>/*`; a plain `git push` sends exactly the same branches as without Gedo; a fetch never overwrites an unpushed local chain; a rejected push is resolved by fetch, `merge_remote` and push again; running the setup twice leaves one fetch refspec and no `remote.<name>.push`, in `crates/adapters/gedo-store/tests/transport.rs` (the tests run the `git` executable as test tooling only)

### Implementation for User Story 5

- [X] T031 [US5] Implement the remote setup in `crates/adapters/gedo-store/src/remote.rs`: add `+refs/gedo/*:refs/gedo-remote/<remote>/*` to `remote.<name>.fetch` once per remote through `gix` configuration editing, keep every other value, never add `remote.<name>.push`; expose the push arguments `["push", <remote>, "refs/gedo/*:refs/gedo/*"]` and the recursion guard name `GEDO_INNER_PUSH`
- [X] T032 [US5] Implement `cargo xtask host-validate` in `tools/xtask/src/host_validate.rs`: for each configured host (`GEDO_TEST_GITHUB_URL` / `GEDO_TEST_GITHUB_TOKEN`, `GEDO_TEST_GITLAB_URL` / `GEDO_TEST_GITLAB_TOKEN`) push a new chain entry tagged with the run id, clone fresh, fetch, check the entries of all earlier runs are still present (retention), write a Markdown summary with timings and sizes; report "not configured" and exit 0 when secrets are missing; never print tokens

**Checkpoint**: US1, US2, US3 and US5 pass

---

## Phase 7: User Story 7 - Every change is checked on Windows and Linux (Priority: P1)

**Goal**: CI enforcing the full check set

**Independent Test**: a pull request breaking a test on one platform, and one importing `gix` from the core, are both blocked

- [X] T033 [P] [US7] Implement `cargo xtask arch-check` in `tools/xtask/src/arch_check.rs`: read `architecture.core_path` and `architecture.forbidden_core_imports` from `.dev-cycle.json`, read `cargo metadata`, fail when any crate under `crates/core/` has a forbidden crate (names with `-` and `_` normalized) in its normal dependency closure; unit-tested with a fake metadata fixture
- [X] T034 [US7] Write `.github/workflows/ci.yml`: on pull requests and pushes to `main`; matrix `ubuntu-latest` and `windows-latest` running `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask arch-check`; Linux job with `GEDO_TEST_SSH=1` and `openssh-server`; MSRV job `cargo +1.95 check --workspace`; `cargo deny check` job; benchmarks `cargo bench -p gedo-store -- --quick` uploading results as an artifact (not blocking); caching of the cargo registry and `target`
- [X] T035 [US7] Write `.github/workflows/host-validation.yml`: `workflow_dispatch` and weekly `schedule` (Monday 06:17 UTC), running `cargo xtask host-validate` with the four secrets and publishing the summary to the job summary

**Checkpoint**: CI green on both platforms

---

## Phase 8: User Story 4 - The local cache can always be rebuilt (Priority: P2)

**Goal**: versioned redb cache with rebuild

**Independent Test**: `crates/adapters/gedo-store/tests/cache_rebuild.rs`

### Tests for User Story 4

- [ ] T036 [P] [US4] Write rebuild tests: missing file, corrupted file (random bytes), schema version 0 and 2; each opens as `Rebuilt { reason }`, emits a `tracing` event, and answers the same lookups as before; a rebuild interrupted by a write failure leaves no cache that opens as `Ready` with partial content (the next open rebuilds again); on Windows only, a cache file held by another handle is retried for up to 2 s then fails with context, in `crates/adapters/gedo-store/tests/cache_rebuild.rs`

### Implementation for User Story 4

- [ ] T037 [US4] Implement `RedbCache` in `crates/adapters/gedo-store/src/cache.rs`: `meta.schema_version: u32` = 1, one table per namespace, open → `Ready` or delete and rebuild from the `CacheRebuildSource` → `Rebuilt { reason: Missing | SchemaMismatch { found } | Corrupted }`; Windows sharing-violation retries up to 2 s
- [ ] T038 [US4] Run the cache contract tests against `RedbCache` in `crates/adapters/gedo-store/tests/contract.rs`

**Checkpoint**: all P1 and P2 stories except US6 pass

---

## Phase 9: User Story 6 - Know the reference limits before building on them (Priority: P2)

**Goal**: measured confirmation of the commit-chain layout

**Independent Test**: the report exists and the decision is in ADR-0011

- [ ] T039 [US6] Implement `cargo xtask scale-study` in `tools/xtask/src/scale_study.rs`: for entry counts (default 100, 1,000, 10,000, 50,000, 100,000) build a repository with one chain holding N entries and one with N references, measure initial push, fresh clone plus fetch, and incremental push and fetch after one change against a file remote (and a host when `--remote` names one), write `target/scale-study/report.md` with times and transferred sizes
- [ ] T040 [US6] Run the scale study, compare with SC-007 (10,000 entries adding more than 5 s or 50 MB fails), and record the results and the decision in the Consequences of `docs/adr/0011-gedo-references-as-commit-chains-with-a-separate-fetch-names.md` (status `proposed` until the user accepts the plan's results)

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T041 [P] Edit `docs/architecture/store.md` in place: commit-chain collections, fan-out, `refs/gedo-remote/`, push and fetch, WAL format and group commit, cache versioning and rebuild, remote setup; link ADR-0011
- [ ] T042 [P] Edit `docs/architecture/core.md` in place: port list, in-memory adapters, contract suite, `Collection`
- [ ] T043 [P] Update `docs/product/TECH_SPEC.md` library table (`crc32fast`, `tempfile`, `criterion`, `cargo_metadata`) and replace "cargo-audit" with "RustSec advisories through cargo-deny"; run `dev-cycle-protocol/constitution.py generate`
- [ ] T044 [P] Write `README.md` with `user-docs`: what Gedo is, status (v0.1.0, library only), build and test commands, the `git log --all` note, license
- [ ] T045 Run `architecture-docs/build_index.py`, `architecture-docs/check_docs.py`, `software-design/check_design.py`, `user-docs/check_readme.py`, all tests and `cargo deny check`; fix everything reported
- [ ] T046 Walk through `quickstart.md` steps 1 to 7 and fix any drift between the guide and the code
- [ ] T047 Run `cargo bench -p gedo-store --bench wal_append` on the reference machine (4 cores, SSD) on Windows and Linux and record p50, p95 and p99 in `docs/plans/gedo_store_foundation.md` (SC-003: p95 under 1 ms)
- [ ] T048 Write `docs/ci/branch-protection.md` with the exact required checks for `main` (both matrix jobs, MSRV, deny, arch-check) and ask the repository owner to enable them in the GitHub settings; verify with a deliberately failing draft pull request that merging is blocked (SC-008)

---

## Dependencies & Execution Order

- **Setup (T001–T007)** → **Foundational (T008–T014)** → stories.
- **US1 (T015–T019)** first; **US2 (T020–T024)** depends on US1's object store (chains write commits); **US3 (T025–T029)** depends only on Foundational and can run in parallel with US1 and US2; **US5 (T030–T032)** depends on US2 (chains and `merge_remote`); **US7 (T033–T035)** can start after Setup and must be green before the pull request; **US4 (T036–T038)** depends only on Foundational; **US6 (T039–T040)** depends on US2 and US5.
- **Polish (T041–T048)** after all stories; T048 needs the repository owner.

### Parallel Opportunities

- Setup: T002 to T006 in parallel after T001.
- Foundational: T008, T009 and T010 in parallel; T011 after them; T012 and T013 after T011; T014 in parallel with T012.
- Tests of each story ([P]) in parallel with each other before the story's implementation.
- US3, US4 and US7's arch-check (T033) in parallel with US1 and US2.
- Polish: T041 to T044 in parallel.

## Parallel Example: User Story 3

```text
T025 wal_torn.rs        ┐
T026 wal_durability.rs  ├─ in parallel, then T028 FileWal, then T029 contract run
T027 wal_append bench   ┘
```

## Implementation Strategy

1. Setup and Foundational, then **US1** (MVP slice: data in and out of a repository without side effects) and validate it alone.
2. Add US2 (concurrency and chains), US3 (WAL) in parallel, then US5 (transport), the riskiest external assumption.
3. US7 makes CI enforce everything before the first pull request review; US4 and US6 complete the release.
4. Each checkpoint maps to a plan milestone (`check_milestone.py`): M1 Setup and core, M2 Objects and references, M3 WAL, M4 Transport, M5 CI, M6 Cache, M7 Scale study, M8 Documentation.
