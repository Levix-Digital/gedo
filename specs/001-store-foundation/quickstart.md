# Quickstart: Validating the Store Foundation

Runnable checks that prove `specs/001-store-foundation` works end to end. Commands run from the repository root on Windows (PowerShell) or Linux.

## Prerequisites

- Rust stable (MSRV 1.95) and Git 2.34 or later.
- Linux only, for the SSH-remote test: `openssh-server` (the test starts `sshd` on localhost with a generated key).
- For the host validation: the secrets listed in `research.md` (R9).

## 1. Build and unit tests (User Stories 1 to 4)

```text
cargo build --workspace
cargo test --workspace
```

Expected: all tests pass, including the contract suite run against both the in-memory and the real adapters, the working-tree invariance tests (SC-001), the compare-and-swap stress test (SC-002), the torn-log test at every byte offset (SC-004) and the cache rebuild tests (SC-005).

## 2. Architecture rule (FR-001)

```text
cargo xtask arch-check
python <skills_root>/software-design/check_design.py   # when the dev-cycle tools are installed
```

Expected: no core crate depends on `gix`, `redb`, `tokio`, `notify`, `tree_sitter` or an adapter crate.

## 3. Push and fetch through plain remotes (User Story 5)

```text
cargo test -p gedo-store --test transport -- --include-ignored file_remote
GEDO_TEST_SSH=1 cargo test -p gedo-store --test transport -- --include-ignored ssh_remote      # Linux
```

Expected: Gedo chains survive push, fresh clone with the remote setup and fetch; branches pushed are the same as without Gedo; unpushed local chains are not overwritten by a fetch; running the setup twice leaves one fetch refspec.

## 4. Host validation (User Story 5, GitHub and GitLab)

Run the `host-validation` workflow from the Actions tab (or wait for the weekly run). Expected: a summary per host with round trip, retention of the data pushed by the previous run, timings and sizes; "not configured" when the secrets are missing.

## 5. Scale study (User Story 6)

```text
cargo xtask scale-study --remote file --entries 100,1000,10000,50000,100000
```

Expected: a Markdown report in `target/scale-study/` with fetch and push times and sizes per entry count, for chains and for one reference per entry, plus the SC-007 verdict at 10,000 entries; the decision is copied into ADR-0011.

## 6. Benchmarks (SC-003)

```text
cargo bench -p gedo-store --bench wal_append
```

Expected on the reference machine (4 cores, SSD): append p95 under 1 ms.

## 7. Manual check of invariance

In any scratch repository with uncommitted edits, run the store's example (`cargo run -p gedo-store --example roundtrip -- <repo>`), then `git status`, `git log --branches --remotes --oneline` and `git fsck`. Expected: same status as before, no new commit on any branch, a clean `fsck`, and `git for-each-ref refs/gedo` lists the new chain. (`git log --all` does list the chain's commits as separate histories with no relation to the code; see ADR-0011, Consequences.)
