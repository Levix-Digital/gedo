# Implementation Plan: Store Foundation

**Branch**: `feat/001-store-foundation` | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-store-foundation/spec.md`

## Summary

Create the Rust workspace and Gedo's persistence foundation: core types and ports (object store, reference store, write-ahead log, symbol cache) with in-memory adapters and a shared contract test suite; a `gix`-based store adapter that never spawns `git` or touches the working tree; Gedo collections stored as **commit chains** under `refs/gedo/<collection>` updated by compare-and-swap (ADR-0011, from a spike showing non-commit references need forced pushes); a crash-safe WAL with 100 ms group commit; a versioned, rebuildable `redb` cache; a remote setup that adds only a fetch refspec into `refs/gedo-remote/<remote>/*`; push and fetch validation against file and SSH remotes on every pull request and GitHub and GitLab weekly; a scale study; and CI on Windows and Linux.

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.95 (`rust-version` in the workspace).

**Primary Dependencies**: `gix` 0.90 (store adapter only, `default-features = false`), `redb` 4.4 (store adapter only), `postcard` 1.1 + `serde` 1.0 (records), `crc32fast` 1.5 (WAL checksums), `thiserror` 2.0 (errors), `tracing` 0.1 (adapter diagnostics, no subscriber in libraries). Dev and tools: `proptest` 1.12, `tempfile` 3.27, `criterion` 0.8, `cargo_metadata` 0.23 (xtask).

**Storage**: the repository's Git object database and `refs/gedo/*` (shared); `.git/gedo/wal.log`, `.git/gedo/wal.lock` and `.git/gedo/symbol_cache.db` (local).

**Testing**: `cargo test` (unit, contract suite against both adapters, integration against temporary repositories, property-based tests), `criterion` benchmarks, transport tests against file and SSH remotes, a weekly host validation workflow.

**Target Platform**: Windows 10 22H2+ / 11 (x86_64 MSVC) and Linux (x86_64 and aarch64, glibc 2.31+).

**Project Type**: Rust library crates (no binary in this release besides `xtask` and an example).

**Performance Goals**: WAL append under 1 ms p95 (SC-003).

**Constraints**: core crates without I/O or infrastructure libraries (ADR-0001); records versioned and size-limited (64 MiB); no writes outside `refs/gedo/`, the object database and `.git/gedo/`; durability window 100 ms; second WAL writer waits at most 2 s.

**Scale/Scope**: reference repository of about 100,000 lines in about 1,000 files; collections of up to 100,000 entries in the scale study.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle (constitution) | How this plan complies | Status |
|---|---|---|
| I. Design: single responsibility, explicit errors, testability | One module per port and per adapter; `StoreError` with operation and path on every error; pure collection logic tested on in-memory adapters | Pass |
| II. Simplicity first | Exactly one abstraction per port, each with two implementations now (real and in-memory) and an external dependency to isolate; no crates for later layers yet; no plugin or configuration system | Pass |
| III. Hexagonal | Ports and in-memory adapters in `crates/core/gedo-core`; `gix` and `redb` only in `crates/adapters/gedo-store`; `cargo xtask arch-check` and `check_design.py` enforce it | Pass |
| IV.2 Evidence | Contract, property-based, invariance, stress, torn-log, rebuild and transport tests; benchmarks | Pass |
| IV.3 Documentation describes today | `store.md` and `core.md` edited in place, ADR-0011, README build section | Pass |
| IV.5 Safe delivery | Feature branch, CI on Windows and Linux on every pull request | Pass |
| IV.6 Security | No secrets in Git (host tokens are CI secrets); untrusted decoding limits | Pass |
| IV.9 No AI attribution | Commits by the repository identity; chain commits use the user's identity | Pass |
| TECH_SPEC: Git subprocess only where allowed | Store never spawns `git`; the push from the pre-push hook is added to the allowed places by ADR-0011 (amends ADR-0009) | Pass (with ADR-0011) |
| TECH_SPEC: `cargo-audit` in CI | Advisories checked by `cargo deny check advisories` against the same RustSec database (research R9) | Pass (justified below) |
| ADR-0003 layout | Changed by ADR-0011 (proposed with this plan, accepted with its approval) | Pass (with ADR-0011) |

Re-check after design: no new violations; data model and contracts keep every rule above.

## Project Structure

### Documentation (this feature)

```text
specs/001-store-foundation/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── ports.md
│   └── repository-layout.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                         # workspace, [workspace.dependencies], rust-version = "1.95"
deny.toml                          # licenses, advisories, bans (cargo-deny)
.github/workflows/
├── ci.yml                         # Windows + Linux matrix, MSRV, deny, arch-check, benches
└── host-validation.yml            # manual + weekly: GitHub and GitLab round trip and retention
crates/
├── core/
│   └── gedo-core/
│       └── src/
│           ├── lib.rs
│           ├── ids.rs             # ObjectHash, GitOid, EntryId, GedoRefName
│           ├── error.rs           # StoreError
│           ├── record.rs          # RecordEnvelope, limits
│           ├── ports/             # object.rs, refs.rs, wal.rs, cache.rs
│           ├── collection.rs      # commit-chain collections, fan-out, union merge
│           ├── memory/            # in-memory adapters (feature "memory")
│           └── contract_tests.rs  # shared port contract suite (feature "contract-tests")
└── adapters/
    └── gedo-store/
        ├── src/
        │   ├── lib.rs
        │   ├── repository.rs      # open, object format check, identity, .git/gedo paths
        │   ├── objects.rs         # ObjectStore over gix
        │   ├── refs.rs            # RefStore over gix reference transactions
        │   ├── wal.rs             # file WAL, lock, flusher thread
        │   ├── cache.rs           # redb SymbolCache with rebuild
        │   └── remote.rs          # fetch refspec setup, push arguments
        ├── tests/                 # contract.rs, invariance.rs, cas_stress.rs, wal_torn.rs,
        │                          # cache_rebuild.rs, transport.rs, formats.rs
        ├── benches/wal_append.rs
        └── examples/roundtrip.rs
tools/
└── xtask/                         # cargo xtask arch-check | scale-study | host-validate
```

**Structure Decision**: the layout of ADR-0001, created only as far as this release needs: `crates/core/gedo-core` and `crates/adapters/gedo-store`. The other crates (`gedo-ast`, `gedo-crdt`, `gedo-graph`, `gedo-app`, `gedo-parse`, `gedo-net`, `gedo-cli`, `gedod`) are created by the releases that fill them. In-memory adapters live in `gedo-core` behind a feature because they are pure (no I/O) and serve the core's own tests.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| TECH_SPEC names `cargo-audit` in CI; the plan uses `cargo deny check advisories` | One tool and one configuration for licenses, bans and advisories | Running both checks the same RustSec database twice and doubles configuration |
| TECH_SPEC lists `blake3`, `uuid`, `tokio` and others; this release adds `crc32fast`, `tempfile`, `criterion`, `cargo_metadata` | WAL checksums, test repositories, benchmarks and the architecture check | BLAKE3 is heavier than needed for a 4-byte frame checksum; the others have no in-repo alternative. TECH_SPEC's library table is updated |

## Dev Cycle Protocol

### Backlog items and release

FEAT-001, FEAT-002, FEAT-003, FEAT-004, FEAT-005, FEAT-006, FEAT-007 and FEAT-119 (`docs/BACKLOG.md`), release **v0.1.0 Store foundation** (`docs/ROADMAP.md`). Plan envelope: `docs/plans/gedo_store_foundation.md`.

### Tests (quality-assurance)

- **Contract suite** (`gedo_core::contract_tests`) run against the in-memory and the real adapters: identical behavior for every port operation, including `Moved` outcomes, `WalBusy`, rebuild statuses and namespace refusals.
- **Property-based**: record envelope round trip and malformed input never panics; fan-out paths are unique and valid on case-insensitive file systems; collection union merge is commutative, associative and idempotent.
- **Invariance (SC-001)**: in repositories with uncommitted edits, staged files, a detached `HEAD`, a worktree and a bare repository, every store operation leaves `git status --porcelain`, the index, `HEAD` and `refs/heads/*` byte-identical.
- **Compare-and-swap stress (SC-002)**: 8 threads and 2 processes, 10,000 updates of one reference; zero lost updates; the reflog shows every value.
- **Torn log (SC-004)**: truncate the last record at every byte offset and flip bytes in earlier records; open returns all good records, drops the torn one, reports corruption with its offset.
- **Durability**: records appended and not synced become durable within 100 ms (checked by reopening from a second handle after the window); `sync` is immediate.
- **Cache rebuild (SC-005)**: missing, corrupted and other-schema caches are rebuilt and answer the same lookups.
- **Transport (User Story 5, SC-006)**: file remote on Windows and Linux, SSH remote on Linux; round trip, fresh clone, unchanged branch pushes, fetch never overwriting local chains, push rejection resolved by fetch-merge-push, idempotent setup.
- **Formats**: SHA-256 repository refused; missing identity refused; future format versions refused.
- **Benchmark (SC-003)**: `wal_append` p95 under 1 ms on the reference machine.
- **Manual tests**: none added to the curated suite; the host validation is automated (weekly workflow) and the quickstart's step 7 is a developer check, not a release acceptance test.
- **Acceptance**: all spec acceptance scenarios pass in CI on both platforms; the host validation passed at least once on GitHub and GitLab; the scale study report and the layout decision are in ADR-0011.

### Documentation (architecture-docs, user-docs)

- `docs/architecture/store.md`: layout with commit chains, fan-out trees and `refs/gedo-remote/`, WAL format and group commit, cache versioning and rebuild, remote setup; edited in place.
- `docs/architecture/core.md`: the port list and the in-memory adapters and contract suite.
- ADR-0011 (proposed now, accepted with this plan's approval; scale study results added before acceptance of v0.1.0).
- `docs/product/TECH_SPEC.md`: library table gains `crc32fast`, `tempfile`, `criterion`, `cargo_metadata`; "cargo-audit" becomes "RustSec advisories through cargo-deny"; regenerate the constitution.
- `README.md`: what Gedo is (one paragraph), build and test commands, the note that `git log --all` shows Gedo chains.

### CI, release and observability (devops-release)

- New workflows `ci.yml` and `host-validation.yml` (research R9). Required checks on `main`: the matrix jobs, MSRV, deny and arch-check.
- Version: workspace `0.1.0` (pre-release crates, not published to crates.io yet; crate names are reserved in v0.4.0 with FEAT-120). Release candidate `v0.1.0-rc.1` tagged on `main` after the pull request merges (trunk model).
- Observability profile `library`: no logging by default; `tracing` events in the adapters (torn record dropped, cache rebuilt, CAS retries, lock waits) for the binaries to collect later; errors with context. Tests assert the events for torn records and rebuilds.

### Security, migration and rollback

- Secrets: host validation tokens only in GitHub Actions secrets; never logged; the workflow masks URLs with credentials.
- Untrusted input: size limits and versioned decoding for everything read from `refs/gedo/*`.
- Migration: first release; no existing data. Every format carries a version so later releases migrate additively.
- Rollback: Gedo data is isolated in `refs/gedo/*`, `refs/gedo-remote/*` and `.git/gedo/`; removing them (`git update-ref -d`, deleting `.git/gedo/`, removing the fetch refspec) restores the repository exactly. Reverting the pull request removes the code.
