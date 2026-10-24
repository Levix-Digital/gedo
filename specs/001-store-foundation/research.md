# Research: Store Foundation

Decisions taken while planning `specs/001-store-foundation`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0010, `docs/product/TECH_SPEC.md`).

## R1. What `refs/gedo/*` references point to

- **Decision**: every Gedo reference points to a **commit chain**. Each update writes a new commit whose tree holds the collection's entries in a fan-out layout (two-hex-digit directories by id, as Git notes do) and whose parent is the previous value; independent updates are merged with a merge commit whose tree is the union of both sides (entries are content-addressed, so a union never conflicts). There is one chain per collection (`refs/gedo/ast-map`, `refs/gedo/tapes`, `refs/gedo/threads`, plus the ones later layers add), not one reference per stored object.
- **Rationale**: verified in a spike on Git 2.43: updating a remote reference that points to a blob or a tree is rejected with "You cannot update a remote ref that points at a non-commit object ... without using the '--force' option", and a forced push would overwrite teammates' data. A commit chain fast-forwards like a branch, so a plain push never needs force, hosts keep every object reachable from it (retention), and the reference log keeps local history. A few chains instead of one reference per object also removes the reference-count risk that FEAT-007 measures.
- **Alternatives considered**: references to raw blobs or trees (rejected: needs force pushes, unsafe); one commit-chain reference per object (fast-forwards, but tens of thousands of references); Git notes under `refs/notes/` (attached to commits only, rewritten by some tools; already rejected in ADR-0003).
- **Recorded in**: ADR-0011 (proposed with this plan).

## R2. Fetching remote Gedo data without overwriting local data

- **Decision**: `gedo`'s remote setup adds one fetch refspec per remote, `+refs/gedo/*:refs/gedo-remote/<remote>/*`. A plain `git fetch` mirrors the remote chains into that namespace; Gedo merges them into the local chains (R1) when it next writes or reads. No push refspec is ever added.
- **Rationale**: a fetch refspec straight into `refs/gedo/*` would make a forced fetch overwrite local, unpushed chains. A separate namespace outside `refs/gedo/` is never pushed by `refs/gedo/*:refs/gedo/*` and keeps the remote's state visible for merging. A push refspec in `remote.<name>.push` replaces Git's `push.default` behavior for branches, which breaks the invariance principle (clarification 3).
- **Alternatives considered**: fetch into `refs/gedo/remotes/<remote>/*` (it would be pushed back by `refs/gedo/*`); no fetch refspec and an explicit `gedo` fetch (a plain `git fetch` would not carry the data).

## R3. Pushing Gedo data

- **Decision**: the push is `git push <remote> refs/gedo/*:refs/gedo/*` (fast-forward only, never forced), run by Gedo's `pre-push` hook (v0.4.0) with a recursion guard (`GEDO_INNER_PUSH=1`). A rejected push (remote moved) is resolved by fetching, merging the chains (R1) and pushing again. This release ships the push arguments, the merge of chains and the validation harness; the hook arrives with the CLI.
- **Rationale**: the hook must use the user's credentials and network configuration, like the pass-through and the background fetch (ADR-0006, ADR-0009). Git runs `pre-push` before transferring the branches; pushing content-addressed data even when the branch push is then rejected is harmless.
- **Subprocess scope**: this adds "push of `refs/gedo/*` from the pre-push hook" to the places allowed to run the user's `git` (CLI pass-through, background fetch). Recorded in ADR-0011, which amends ADR-0009.

## R4. Git object and reference access

- **Decision**: `gix` 0.90 with `default-features = false` and only the features the store needs (SHA-1 object hashing, object writing, reference transactions with reflog), used only in `gedo-store`. Compare-and-swap uses reference transactions with `PreviousValue::MustNotExist` or `MustExistAndMatch`; `gix` takes a `.lock` file per reference, so concurrent processes cannot both win.
- **Verified in a spike**: blobs, trees and commits written with `gix` leave the working tree and index untouched (`git status` unchanged, `git fsck` clean); the reflog records each update; an update whose expected value does not match is refused. Creating a reference with `MustNotExist` when it already holds the **same** value succeeds (idempotent); the contract treats that as success, and a different existing value is refused.
- **SHA-256**: `gix` reports the repository's object hash; anything but SHA-1 is refused with `UnsupportedObjectFormat` (FR-018) until FEAT-008.
- **Alternatives considered**: libgit2 and the `git` CLI (rejected in ADR-0002).

## R5. Write-ahead log format and durability

- **Decision**: `.git/gedo/wal.log` starts with an 8-byte magic (`GEDOWAL\0`) and a file format version (`u16`), followed by records framed as `length: u32 LE | checksum: u32 LE (CRC-32 of version and payload) | record format version: u16 LE | payload`. Appends go to the OS immediately; a flusher thread in the adapter calls `sync_data` whenever unsynced records are older than 100 ms; `sync()` flushes at once (publish, commit). On open, records are read until the first incomplete frame at the end (torn record: dropped, file truncated to the last good record, reported) or a checksum mismatch before the end (corruption: reported with its offset, never returned as data).
- **Single writer**: an exclusive OS file lock on `.git/gedo/wal.lock` (`std::fs::File::lock`, stable since Rust 1.89, released by the OS when the process dies, so a crashed writer never leaves a stale lock). A second writer waits up to 2 s, then gets `WalBusy`.
- **Rationale**: a per-append `fsync` costs several milliseconds on Windows and would break the 1 ms p95 append requirement; the 100 ms group commit is the clarified durability window (clarification 1).
- **Dependency**: the checksum is CRC-32 (IEEE) computed with `crc32fast` 1.5 (MIT OR Apache-2.0, widely used, SIMD-accelerated). BLAKE3 is not used here: a 4-byte checksum per record is enough to detect torn and flipped data, and it is cheaper.
- **Alternatives considered**: one file per record (slow, many files on Windows); SQLite or redb as the log (heavier than an append-only file; a corrupt database is harder to salvage than a truncated file).

## R6. Symbol cache

- **Decision**: `redb` 4.4 at `.git/gedo/symbol_cache.db`, with a `meta` table holding `schema_version` and one table per cache namespace (byte keys and values). Opening checks the schema version; on a mismatch, a redb error or a missing file, the file is deleted and rebuilt by the caller-provided rebuild source (the chains in `refs/gedo/*`), and the rebuild is logged.
- **Rationale**: ADR-0004. The cache content (symbols) is defined by v0.2.0; this release delivers the port, the adapter, the versioning and the rebuild path, tested with generic entries.

## R7. Record encoding

- **Decision**: every stored record is an envelope `{ format: u16, payload }` encoded with `postcard` 1.1 and `serde`; decoding first checks the length against `MAX_RECORD_BYTES = 64 MiB` and the format against the versions the reader knows, and returns an error (never panics) on malformed input. Decoding is fuzzed in later releases (FR-012 tests use property-based malformed inputs now).
- **Rationale**: TECH_SPEC (format version prefix, untrusted input, size limits).

## R8. Architecture check in CI

- **Decision**: a workspace tool (`tools/xtask`, `cargo xtask arch-check`) reads `cargo metadata` and fails when a crate under `crates/core/` has, in its normal (non-dev) dependency closure, a crate listed in `architecture.forbidden_core_imports` of `.dev-cycle.json`. `software-design/check_design.py` (source-level import check) is run locally before each pull request, as the protocol asks; CI does not depend on the private skills repository.
- **Rationale**: the dependency graph is the authoritative rule and is cheaper and stricter than scanning imports; it also catches transitive leaks.

## R9. CI

- **Decision**: GitHub Actions. `ci.yml` on every pull request and push to `main`: matrix `ubuntu-latest` and `windows-latest`, with `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test` (including file-remote push and fetch tests), `cargo xtask arch-check`, `cargo deny check` (licenses, advisories, bans; `deny.toml`), and the benchmarks in quick mode with results uploaded as an artifact (not blocking, see the spec's assumptions). The SSH-remote test runs on Linux only, against an `sshd` started on localhost with a generated key (Windows runners have no ready SSH server; the file remote covers Windows). `host-validation.yml`: `workflow_dispatch` and weekly `schedule`, pushing and fetching against the GitHub and GitLab test repositories with tokens from secrets (`GEDO_TEST_GITHUB_URL`, `GEDO_TEST_GITHUB_TOKEN`, `GEDO_TEST_GITLAB_URL`, `GEDO_TEST_GITLAB_TOKEN`), checking retention of the data pushed by the previous run; it reports "not configured" and succeeds when the secrets are missing.
- **Advisories**: `cargo deny check advisories` covers the RustSec database, so a separate `cargo-audit` run is not needed (one tool, one configuration); TECH_SPEC's "cargo-audit" requirement is met by the same database.
- **Toolchain**: stable Rust, MSRV 1.95 checked by a `cargo check` job with that exact version on Linux.

## R10. Remaining defaults

- **Windows file sharing**: opening or renaming the log or the cache retries for up to 2 s on sharing violations (antivirus, indexers), then fails with context.
- **Reference names**: collection names are lowercase ASCII with `-`; entry ids in fan-out paths are lowercase hex, valid and distinct on case-insensitive file systems.
- **Commit identity for chain commits**: the repository's configured user identity (`user.name`, `user.email`); if none is set, Gedo refuses to write with a clear message instead of inventing one.
- **Scale study thresholds**: consolidation was the fallback for "too many references"; with R1 the study measures chains holding 100 to 100,000 entries (fetch time and size, incremental push after one change) and compares them with one reference per object, to confirm R1 against SC-007's thresholds (10,000 entries adding more than 5 s or 50 MB fails).
