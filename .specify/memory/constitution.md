<!-- dev-cycle-constitution
generated-by: dev-cycle-protocol/constitution.py (do not edit; change a source and regenerate)
sources-sha256: 2a9dc77fd857192b31957dba8b04c621f5b99bc048c54b2d516bfb8b26e8473f
content-sha256: 8cbf4d70e2ef1a12a9aaa9acb367c5a6b587d552bac4ab5340af912747c0e3b1
-->
# gedo Constitution

The non-negotiable rules every specification, plan and task of this project follows. It is generated from the project's sources of truth by `dev-cycle-protocol/constitution.py`; change a source and regenerate it, never edit it.

## Core Principles

### I. Design

*Source: software-design*

#### Modularity and separation of responsibilities
- One module, type or function has **one reason to change**. When a description needs "and", split it.
- Group code by what it is about (a domain concept or a feature), not by what kind of file it is, unless the architecture style says otherwise.
- Business rules never live in the user interface, in transport code (HTTP handlers, CLI argument parsing) or in persistence code.
- Dependencies point toward stable code: details (frameworks, databases, network, UI) depend on the core, never the other way around.

#### SOLID, in practice
| Principle | What it means when writing code | Smell that it is violated |
|---|---|---|
| **S**ingle responsibility | Each unit does one job and can be named without "and". | A 600-line file mixing parsing, rules and output. |
| **O**pen/closed | Add behavior by adding a new implementation, not by growing an `if/else` chain in stable code. | Every new provider edits the same `match` in five places. |
| **L**iskov substitution | Any implementation of an interface can replace another without surprises. | An implementation that panics or ignores half the contract. |
| **I**nterface segregation | Small interfaces shaped by what the caller needs. | Callers implementing methods they never use. |
| **D**ependency inversion | High-level code depends on abstractions it owns; adapters implement them. | Core code constructing an HTTP client or a database driver. |

#### Clean Code
- **Names** say what something is or does; no abbreviations a newcomer cannot decode; no `data`, `info`, `manager`, `util` as whole names.
- **Small functions** that do one thing at one level of abstraction; early returns instead of deep nesting.
- **No magic values**: named constants with units in the name (`TIMEOUT_MS`).
- **Comments explain why**, never what; delete commented-out code and dead code.
- **Consistency over preference**: follow the conventions already in the codebase (naming, error types, layout) unless the change is an agreed refactor.
- **Duplication**: remove it when the same rule exists twice; tolerate it when two pieces only look alike today.

#### Explicit error handling
- Never swallow an error (`except: pass`, `let _ =` on a meaningful result, an empty `catch`, a blind `.unwrap()` on input that can fail).
- Errors carry context (what was being done, with which input) and use the project's canonical error types.
- Decide at each boundary: recover, translate into the caller's error type, or propagate. Report to the user once, at the edge.
- Fallbacks are allowed only when they are explicit, logged and visible in status output; a silent fallback that changes behavior is a defect.

#### Testability
- Pure logic is separated from I/O so it can be unit-tested without mocks.
- External services sit behind an interface the tests can replace.
- A structure that is hard to test is a design problem, not a testing problem.

### II. Simplicity First

*Source: software-design*

Principles applied mechanically produce over-engineering. The rule: **the simplest solution that keeps responsibilities separate.**

- Introduce an abstraction (trait, interface, base class, factory) only when at least one of these is true:
  1. there is a **second implementation** now (not "maybe later");
  2. it creates a **test seam** you need today;
  3. it isolates an **external dependency** (framework, service, database, OS API).
- No speculative configuration, plugin systems or generic frameworks for a single use case.
- Prefer composition over inheritance; prefer plain data and functions over class hierarchies.
- Three similar lines are better than a premature helper with five parameters.

#### Design patterns: only when they solve a real problem
| Problem actually present | Pattern that fits |
|---|---|
| Several interchangeable algorithms or providers chosen at runtime | Strategy |
| A third-party or legacy interface that does not match yours | Adapter |
| Construction is complex or depends on configuration | Factory / Builder |
| Behavior changes with an explicit lifecycle state | State machine |
| Actions must be queued, logged, undone or dispatched uniformly | Command |
| Persistence must be swappable or mocked | Repository |
| Several parts react to an event without knowing each other | Observer / event channel |

If you cannot point to the problem in the left column, do not use the pattern. Name the pattern in code or docs only when it helps the reader.

### III. Architecture Style: hexagonal

*Source: software-design/styles/hexagonal.md*

##### Structure

- **Core (domain + application services)**: entities, business rules, use cases and the **ports** they need (interfaces/traits for storage, providers, clocks, file systems). No framework, UI, transport or driver code.
- **Driving adapters** (input): CLI, TUI, HTTP handlers, RPC, MCP servers. They translate requests into core calls and core results into output.
- **Driven adapters** (output): database repositories, HTTP clients, model providers, file system access. They implement the core's ports.
- **Composition root**: one place per executable that builds the adapters and wires them into the core.

##### Dependency rule

Adapters depend on the core; the core depends on nothing outside itself except plain libraries (serialization, math, date handling). List the adapter and framework modules in `architecture.forbidden_core_imports` so `check_design.py` enforces the rule.

##### Where things go

| Piece | Location |
|---|---|
| A business rule or validation | Core service or entity |
| A new user command | Core command/use case, then one thin handler per surface |
| Database schema or query | Driven adapter implementing a core repository port |
| A new LLM, payment or storage provider | Driven adapter implementing an existing port |
| Output formatting, colors, layout | Driving adapter |

##### Common mistakes

- Business logic in a CLI handler or TUI event loop "because it was quicker".
- The core constructing its own HTTP client, database connection or UI widget.
- Ports shaped like the first adapter's API (leaking driver types into the core).
- A port for something with one implementation and no test need (see "Simplicity First").

### IV. Development Cycle

*Source: dev-cycle-protocol*

1. **Design**: modular code with clear responsibilities, SOLID, Clean Code, explicit errors, patterns only when needed, in the project's architecture style (`software-design`).
2. **Evidence**: every change is proven by automated tests; manual tests cover only what cannot be automated, in one curated suite (`quality-assurance`).
3. **Documentation describes today**: technical docs and README are edited in place; history lives in Git and release notes (`architecture-docs`, `user-docs`).
4. **Plans are shared and complete**: approved plans are versioned files in the project (`plan-memory`).
5. **Safe delivery**: short-lived branches on the project's branching model, CI on every pull request, release candidates, the user's release gate (`devops-release`).
6. **Security**: no secrets in Git, backward-compatible data migrations, a rollback path for every release.
7. **Bugs first**: open `critical` and `high` bugs are planned and fixed before new features, improvements and adjustments, unless the user explicitly accepts the risk; nothing found is lost, it is fixed now or captured in the backlog.
8. **Follow-through**: the agent follows CI and releases to the end and fixes what fails; the user's decisions (plan approval, candidate acceptance, the release gate) are never taken by the agent.
9. **No AI attribution, anywhere**: no artifact names an AI, a model or a coding agent as author, co-author, reviewer or source: commits and their trailers, tags, branches, pull and merge requests, reviews and review comments, issues, plans, documentation, release notes and code comments. Everything is authored by the repository's Git identity (`devops-release`). Configuration a project needs for its tools (an agent's settings folder, an integration name) is technical, not attribution.

## Product Requirements and Constraints

*Source: docs/product/TECH_SPEC.md*

### Non-Functional Requirements

Reference repository for the numbers below: about 100,000 lines of Rust in about 1,000 files, on a 4-core machine with an SSD, measured on Windows and Linux. Benchmarks for each requirement run in CI.

#### Performance

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

#### Security and Privacy

- No network access of its own: data moves only through the user's Git remotes, with the user's Git credentials; Gedo never reads or stores credentials.
- No telemetry. Logs stay in `.git/gedo/logs/` and never contain source code or credentials.
- Everything fetched under `refs/gedo/*` is untrusted input: decoding enforces size limits and format versions, never panics on malformed data, and the decoders are fuzzed. Nothing fetched is ever executed.
- Hooks installed by `gedo init` contain only a call to `gedo`; existing hooks are chained, never removed or rewritten.
- `author_fingerprint` is the BLAKE3 hash of the Git author identity, not the identity itself.
- Supply chain: `cargo-deny` runs in CI and checks licenses, duplicate versions and RustSec advisories; new dependencies need a reason in the pull request.
- No personal data beyond what Git already records; no regulation beyond the licenses applies.

#### Availability and Recovery

- Local tool: no uptime target. The requirement is that Gedo never loses or corrupts user data and never blocks Git.
- The WAL is append-only with a checksum per record and `fsync` on consolidation; a torn last record after a crash is detected and dropped.
- The redb cache is derived data and is rebuilt from `refs/gedo/*` on corruption or schema change.
- Reference updates are compare-and-swap; Git's reflog keeps previous values of `refs/gedo/*`.
- If Gedo fails inside a hook, the Git command still succeeds (the hook reports the error and exits 0, except in `pre-push` when the user enabled strict mode); the overlay is resynchronized on the next run.
- A diagnostics command (`gedo doctor`) reports versions, paths, configuration, hook chain and overlay consistency, with the log path.

#### Accessibility and Localization

- CLI messages in English; localization is out of scope until the repository is public.
- Information is never conveyed by color alone; `NO_COLOR` and non-TTY output disable colors; machine-readable output (`--json`) for Gedo commands.
- Pass-through keeps Git's own output and localization untouched.

### Constraints

- **Non-negotiable principles** (blueprint): no metadata in source files or the working tree; `git clone` and `git push` without Gedo keep working and `refs/heads/*` stay canonical; all Gedo data travels in `refs/gedo/*`; no upper layer is built before the lower one is deterministic, offline-first and covered by unit and property-based tests.
- **Bottom-up delivery**: Phase 1 store, Phase 2 identity, Phase 3 events, Phase 4 graph, CLI and daemon (the MVP).
- **Git subprocess** only in the CLI pass-through (ADR-0006) and the daemon's background fetch (ADR-0009); the core, the store and the merge driver never spawn `git`.
- **Command names**: a command with a Git name keeps Git's behavior and output (`init` and `gc` run Git's command first, then the overlay step); Gedo-specific commands use names Git does not have.
- **Merges never hide conflicts**: when the semantic merge is not certain, Git's textual merge is the result.
- **No mandatory server** (ADR-0005, ADR-0010); the relay tier is optional, self-hostable, and Git stays the canonical store.
- **Licenses**: the project is MIT OR Apache-2.0; dependencies must be permissive and compatible (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0), enforced by `cargo-deny`.
- **Quality**: property-based tests for the core algorithms, snapshot tests for outputs, fuzzing for decoders, an end-to-end suite that compares `git` and `gedo` on the same scenarios, and CI on Windows and Linux for every pull request.
- **Team and budget**: one developer plus the agent; private GitHub repository, so CI minutes are paid (Windows counts double); no paid infrastructure.

## Architecture Decisions

Accepted decisions; a plan that contradicts one needs a new ADR that supersedes it.

- ADR-0001: Hexagonal architecture over a bottom-up crate stack (`docs/adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md`)
- ADR-0002: Rust workspace with gitoxide, Tree-sitter, BLAKE3 and Postcard (`docs/adr/0002-rust-workspace-with-gitoxide-tree-sitter-blake3-and-postcard.md`)
- ADR-0003: Git object store as the canonical datastore (`docs/adr/0003-git-object-store-as-the-canonical-datastore.md`)
- ADR-0004: redb for the local symbol cache (`docs/adr/0004-redb-for-the-local-symbol-cache.md`)
- ADR-0005: Serverless operation with Git remotes as the only transport (`docs/adr/0005-serverless-operation-with-git-remotes-as-the-only-transport.md`)
- ADR-0006: Git command pass-through with hook-based augmentation (`docs/adr/0006-git-command-pass-through-with-hook-based-augmentation.md`)
- ADR-0007: Deterministic node identity derived from history (`docs/adr/0007-deterministic-node-identity-derived-from-history.md`)
- ADR-0008: Gedo data robust to history rewrites (`docs/adr/0008-gedo-data-robust-to-history-rewrites.md`)
- ADR-0009: Semantic merge driver and shadow merge (`docs/adr/0009-semantic-merge-driver-and-shadow-merge.md`)
- ADR-0010: Three tiers of real-time coordination and merge queue complement (`docs/adr/0010-three-tiers-of-real-time-coordination-and-merge-queue-comple.md`)

## Governance

- This constitution supersedes other practices for specifications, plans and tasks. A plan passes the Constitution Check or records each justified violation in its Complexity Tracking table; a lasting exception becomes an ADR.
- Amendments change the source, not this file: an ADR for decisions, the technical specification for requirements and constraints, `.dev-cycle.json` for the architecture style, the skills for principles. Then run `constitution.py generate` and commit the result with the change.
- Gate 3 of the development cycle blocks a plan while this file is stale or edited by hand.

**Version**: 1.1.0 | **Ratified**: 2026-10-23 | **Last Amended**: 2026-10-24
