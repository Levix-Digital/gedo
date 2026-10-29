# Gedo: Technical Specification

Gedo is a set of native command-line tools written in Rust: the `gedo` CLI, which also passes every Git command through to the user's Git, and the `gedod` background daemon. Both work on a local Git repository and store all shared data in its object store under `refs/gedo/*`; there is no server. The architecture style is **hexagonal** (`architecture.style` in `.dev-cycle.json`, ADR-0001): a pure core of five crates and adapters for Git, Tree-sitter, redb and the network. How each part works is described in [TECDOC.md](../../TECDOC.md); this document holds the product-wide technical decisions and requirements.

## Platforms and Stack

**Target platforms**

| Platform | When | Targets |
|---|---|---|
| Windows 10 22H2 and Windows 11 | MVP | `x86_64-pc-windows-msvc` |
| Linux (glibc 2.31 or later; static musl build for distribution) | MVP | `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu` |
| macOS 13 or later | After the MVP | `aarch64-apple-darwin`, `x86_64-apple-darwin` |

**Runtime requirement**: Git 2.34 or later on the user's machine (pass-through and hooks; the `reference-transaction` hook needs 2.28 or later).

**Toolchain**: Rust stable, edition 2024. Minimum supported Rust version (MSRV): **1.95**, set in `rust-version` of the workspace; the policy is "latest stable minus two", raised deliberately in a pull request.

**Main libraries** (latest stable versions on 2026-10-23, declared once in `[workspace.dependencies]`; `Cargo.lock` is committed and pins exact versions):

| Crate | Version | Used in | Why |
|---|---|---|---|
| `gix` | 0.90 | `gedo-store` | Pure-Rust Git object and reference access, no subprocess (ADR-0002) |
| `tree-sitter` | 0.27 | `gedo-parse` | Incremental parsing for many languages |
| `tree-sitter-rust` | 0.24 | `gedo-parse` | Rust grammar (MVP language) |
| `tree-sitter-typescript` / `tree-sitter-python` | 0.23 / 0.25 | `gedo-parse` | Next languages (after the MVP) |
| `blake3` | 1.8 | core | Structural hash and author fingerprint |
| `postcard` + `serde` | 1.1 + 1.0 | core, `gedo-store` | Compact, versioned binary records |
| `redb` | 4.4 | `gedo-store` | Local symbol cache (ADR-0004) |
| `crc32fast` | 1.5 | `gedo-store` | Checksum of each write-ahead log frame |
| `notify` | 8.2 | `gedod` | Cross-platform file-system watcher |
| `tokio` | 1.54 | `gedod`, `gedo-net` | Async runtime, kept out of the core |
| `clap` | 4.6 | `gedo-cli` | Argument parsing for Gedo commands |
| `thiserror` | 2.0 | all crates | Typed errors |
| `tracing` + `tracing-subscriber` | 0.1 + 0.3 | apps, adapters | Structured logs |
| `petgraph` | 0.8 | `gedo-graph` | Graph structures and algorithms |
| `proptest`, `insta` | 1.12, 1.49 | tests | Property-based and snapshot tests |
| `tempfile` | 3.27 | tests, `xtask` | Scratch repositories and directories |
| `criterion` | 0.8 | benchmarks | Write-ahead log append and parse latency |
| `libfuzzer-sys` (`cargo-fuzz`) | 0.4 | `fuzz/` | Coverage-guided fuzzing of the parser and decoders, on nightly, outside the workspace |
| `cargo_metadata` | 0.23 | `xtask` | Dependency graph for the architecture check and the parser sweep |

## Architecture Overview

```
crates/core/       gedo-core (model, ports), gedo-ast, gedo-crdt, gedo-graph, gedo-app (use cases)
crates/adapters/   gedo-store (gix, WAL, redb), gedo-parse (Tree-sitter), gedo-net (transports)
crates/apps/       gedo-cli (`gedo`), gedod (daemon)
```

The core has no I/O and imports no adapter or infrastructure crate (`check_design.py` enforces it). The CLI dispatches Gedo commands to use cases and passes every other command to the user's Git; Git hooks installed by `gedo init` keep the overlay in step when `git` is called directly (ADR-0006). A semantic merge driver registered in the local Git configuration takes part in every merge, and the daemon runs a background shadow merge against the team's branches (ADR-0009). Node identities are deterministic (ADR-0007) and Gedo data survives rebases and server-side squashes (ADR-0008). Real-time coordination comes in three tiers, the first needing no server (ADR-0010). Shared state lives in `refs/gedo/*`, local state in `.git/gedo/` (ADR-0003). Details: [TECDOC.md](../../TECDOC.md).

## Non-Functional Requirements

Reference repository for the numbers below: about 100,000 lines of Rust in about 1,000 files, on a 4-core machine with an SSD, measured on Windows and Linux. Benchmarks for each requirement run in CI.

### Performance

| Operation | Requirement |
|---|---|
| WAL append of one event | under 1 ms (p95) |
| Ingest of one saved file of 2,000 lines (parse, identity matching, WAL) | under 20 ms (p95) |
| `gedo publish` | under 1 s |
| `gedo trace <symbol>` | under 500 ms |
| `gedo init` on the reference repository | under 30 s |
| Pass-through overhead of `gedo <git command>` over `git <git command>` | under 20 ms |
| Overhead of all Gedo hooks on one `git commit` | under 100 ms |
| Merge driver on one file of 2,000 lines | under 200 ms (p95) |
| Shadow merge cycle after a background fetch | under 5 s |
| Re-attachment of one fetched commit | under 100 ms (p95) |
| `gedod` resident memory on the reference repository | under 200 MB |
| Identity accuracy on the reference corpus (renames, moves, formatters) | at least 95% of identities preserved |
| Identity determinism | the same history indexed on different machines and in different orders gives identical ids |
| Merge safety | on replayed real merge histories, the driver never produces a result without conflict markers that differs from the committed resolution in behavior; any doubt falls back to Git's textual merge |

### Security and Privacy

- No network access of its own: data moves only through the user's Git remotes, with the user's Git credentials; Gedo never reads or stores credentials.
- No telemetry. Logs stay in `.git/gedo/logs/` and never contain source code or credentials.
- Everything fetched under `refs/gedo/*` is untrusted input: decoding enforces size limits and format versions, never panics on malformed data, and the decoders are fuzzed. Nothing fetched is ever executed.
- Hooks installed by `gedo init` contain only a call to `gedo`; existing hooks are chained, never removed or rewritten.
- `author_fingerprint` is the BLAKE3 hash of the Git author identity, not the identity itself.
- Supply chain: `cargo-deny` runs in CI and checks licenses, duplicate versions and RustSec advisories; new dependencies need a reason in the pull request.
- No personal data beyond what Git already records; no regulation beyond the licenses applies.

### Availability and Recovery

- Local tool: no uptime target. The requirement is that Gedo never loses or corrupts user data and never blocks Git.
- The WAL is append-only with a checksum per record and `fsync` on consolidation; a torn last record after a crash is detected and dropped.
- The redb cache is derived data and is rebuilt from `refs/gedo/*` on corruption or schema change.
- Reference updates are compare-and-swap; Git's reflog keeps previous values of `refs/gedo/*`.
- If Gedo fails inside a hook, the Git command still succeeds (the hook reports the error and exits 0, except in `pre-push` when the user enabled strict mode); the overlay is resynchronized on the next run.
- A diagnostics command (`gedo doctor`) reports versions, paths, configuration, hook chain and overlay consistency, with the log path.

### Accessibility and Localization

- CLI messages in English; localization is out of scope until the repository is public.
- Information is never conveyed by color alone; `NO_COLOR` and non-TTY output disable colors; machine-readable output (`--json`) for Gedo commands.
- Pass-through keeps Git's own output and localization untouched.

## Integrations

| System | Direction | When unavailable |
|---|---|---|
| User's Git installation | Gedo calls it (pass-through) and Git calls Gedo (hooks) | `gedo` reports that Git is missing; Gedo commands that need only the object store still work |
| Git hosts (GitHub, GitLab, plain SSH or file remotes) | Through the user's Git: push and fetch of `refs/gedo/*` via refspecs; background fetch by `gedod` | Work continues offline; data syncs on the next push or fetch; the shadow merge reports how old its last fetch is |
| Git merge machinery | Git calls `gedo merge-driver` for supported languages | Any driver error falls back to Git's textual merge; Git never fails because of Gedo |
| GitHub Actions (after the MVP; GitLab CI later) | A Gedo Action reads pull requests and the merge queue, posts semantic diffs and contract checks, re-attaches data after server-side merges | Pull requests work as without Gedo; data is re-attached on the next local fetch |
| Optional relay (after the MVP) | Daemons publish and receive intents, leases and threads | Falls back to tier 1 (Git remotes) |
| File system events (OS APIs through `notify`) | OS to `gedod` | The daemon falls back to a periodic scan and logs it; `gedo` commands still work without the daemon |
| Language servers (after the MVP) | `gedo-graph` asks for types and diagnostics | The graph uses syntax-only references |
| crates.io, GitHub Releases, winget (distribution) | Releases published from CI | No impact on installed copies |

## Data

| Entity | Where | Notes |
|---|---|---|
| `BlobAstMap` (nodes of one file at a commit) | `.git/gedo/symbol_cache.db`, namespace `identity` | Local, derived per commit, rebuilt by indexing the history (ADR-0012) |
| Event | `refs/gedo/events` (entry: event id) | Immutable, id derived from content; carries clock, target, operation, author fingerprint and kind, labels (ADR-0014). Events of commits made without Gedo are reconstructed on demand and never stored |
| Tape and commit index | `refs/gedo/tapes` (entry: tape id) and `refs/gedo/commit-tapes` (entry: commit id + tape id) | A tape lists the events of one publish in order and names no commit; the index maps commits to tapes and gains entries after rewrites (ADR-0008, ADR-0014) |
| Identity alias | `.git/gedo/symbol_cache.db`, namespace `identity` | `IdentityMerged` reconciliations found while indexing; rebuilt like the maps, and shared as events from v0.3.0 (ADR-0012) |
| Thread (intents and negotiation of a symbol) | `refs/gedo/threads/<thread-uuid>` | Append-only |
| WAL (unconsolidated micro-events) | `.git/gedo/wal.log` | Local; truncated after `gedo publish` |
| Symbol cache | `.git/gedo/symbol_cache.db` | Local, derived, rebuildable |
| Logs | `.git/gedo/logs/` | Local, rotated |

- Relations: an event targets one `AstNodeId`; a tape groups events; a `BlobAstMap` anchors `AstNodeId`s to byte ranges of one file at a commit; a thread is about one `AstNodeId`.
- Retention: shared data follows the life of the Git history; events no longer reachable from any indexed commit are kept for 90 days, then pruned by `gedo gc`; the WAL lives until it is published.
- Formats: Postcard records prefixed with a format version. Migrations are additive: every release reads all earlier format versions; a format change is an ADR.
- Object format: SHA-1 repositories in the MVP; `GitOid` already carries SHA-256 (see Open Questions).

## Constraints

- **Non-negotiable principles** (blueprint): no metadata in source files or the working tree; `git clone` and `git push` without Gedo keep working and `refs/heads/*` stay canonical; all Gedo data travels in `refs/gedo/*`; no upper layer is built before the lower one is deterministic, offline-first and covered by unit and property-based tests.
- **Bottom-up delivery**: Phase 1 store, Phase 2 identity, Phase 3 events, Phase 4 graph, CLI and daemon (the MVP).
- **Git subprocess** only in the CLI pass-through (ADR-0006) and the daemon's background fetch (ADR-0009); the core, the store and the merge driver never spawn `git`.
- **Command names**: a command with a Git name keeps Git's behavior and output (`init` and `gc` run Git's command first, then the overlay step); Gedo-specific commands use names Git does not have.
- **Merges never hide conflicts**: when the semantic merge is not certain, Git's textual merge is the result.
- **No mandatory server** (ADR-0005, ADR-0010); the relay tier is optional, self-hostable, and Git stays the canonical store.
- **Licenses**: the project is MIT OR Apache-2.0; dependencies must be permissive and compatible (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0), enforced by `cargo-deny`.
- **Quality**: property-based tests for the core algorithms, snapshot tests for outputs, fuzzing for decoders, an end-to-end suite that compares `git` and `gedo` on the same scenarios, and CI on Windows and Linux for every pull request.
- **Team and budget**: one developer plus the agent; the GitHub repository is public, so standard CI runners are free (they would be paid, with Windows counting double, if it went private again); no paid infrastructure.

## Open Questions

- Support for SHA-256 repositories in the MVP or later.
- Compatibility of the grammar crates with `tree-sitter` 0.27 (some grammars are published against older versions); verified at the start of Phase 2.
- How many references `refs/gedo/*` can hold before hosts or `git fetch` slow down (one per blob and per commit); measured in Phase 1, with consolidation into index trees if needed.
- Windows on ARM (`aarch64-pc-windows-msvc`) as a target.
