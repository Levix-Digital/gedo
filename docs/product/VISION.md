# Gedo (Git Event-Driven Overlay)

Gedo is a distributed collaboration overlay built directly on Git's object store, for software teams of any stack who want to collaborate on code entities (functions, types, modules) instead of lines of text. It turns a repository into a reactive graph of syntax nodes with stable identities, synchronized by an event-driven CRDT engine, so that conflicts are merged by meaning, detected while they are born and negotiated before the pull request, without a mandatory server, without annotations in source files and without breaking any existing Git tool.

## Problem

Git was designed for asynchronous, low-trust exchange of patches between isolated developers. Teams today work the other way around: high-frequency collaboration on the same code base. Git versions text, not code, and keeps each working tree invisible to the team until a push, so every tool built on it (diff, blame, merge, pull requests) reasons about lines and discovers collisions at the most expensive moment:

- **Divergence in the dark.** Two developers reshape the same module for hours or days without knowing; the collision appears only at merge, rebase or pull request time, when nobody remembers the details.
- **Merges conflict on text, not meaning.** Moving a function or running a formatter on one side while another developer edits a line inside it becomes a whole-block conflict; two fields added at the end of the same struct conflict on the closing brace.
- **Merges that pass and break.** Git declares a merge clean when two developers edited different files, even when one changed a function's contract and the other added a call with the old one; the main branch stops compiling.
- **The race to main.** On an active branch, the first push wins and everyone else gets `rejected (non-fast-forward)`, then rebases, re-runs tests and tries again.
- **Blame loses history.** Moving a function to another file, renaming it or running a formatter makes `git blame` attribute the whole entity to whoever touched it last.
- **Moving code between branches is coarse.** `git cherry-pick` moves whole commits; taking one function with exactly the dependencies it needs is manual, and copying it creates conflicts later.
- **Context switches are traumatic.** Switching branches in the middle of a refactor means `git stash` (and conflicts on `stash pop`) or dirty "WIP" commits, and invalidates incremental build caches.
- **Frankenstein commits.** Renames, extractions and formatting get mixed with business logic in one diff; separating them with `git add -p` is fragile, and reviews and reverts become risky.
- **Squash amnesia.** Teams choose between squash (a clean history that destroys the reasoning) and merge commits (a history polluted with "wip" and "fix tests"); server-side squash and rebase also rewrite commit ids.
- **Lockfiles, generated files and binaries.** Lockfiles produce hundreds of unreadable conflict lines; binaries cannot be merged and depend on manual locks people forget.
- **Reviews are late and file-shaped.** Pull requests of a thousand lines arrive when everything is "done", in no logical order, and architecture problems are found too late.
- **Lost work.** `reset --hard`, a force push over a colleague's commits or a forgotten stash loses work that Git cannot easily bring back.

Today teams cope with conventions (small pull requests, "who is touching X?" messages), with `.git-blame-ignore-revs`, merge queues and manual archaeology through `git log -S`. Existing tools solve single pieces: syntax-aware merge drivers and diff tools work per file without identity or team coordination, real-time co-editing tools need everyone online at once, and other semantic tools need a proprietary server, replace Git, or store metadata inside the working tree.

## Target Users

- **Levix Digital development team (primary).** Daily users of Rust, TypeScript and Python, working on several products in parallel. They want fewer and earlier conflicts, a precise per-symbol history and the ability to move a function with its dependencies between work streams, without changing their hosting (GitHub) or their teammates' tools. Gedo is built and dogfooded by this team first.
- **Small and medium development teams, any stack (later).** Teams of roughly 2 to 50 developers on GitHub, GitLab or self-hosted Git. They want semantic collaboration without migrating away from Git, without a new server to operate and without forcing every collaborator to install a new tool.

## Value Proposition

- **Conflicts merged by meaning**: a semantic merge driver used by plain `git merge`, `rebase` and `pull` converges moves, formatting and edits per syntax node; independent additions to the same structure never conflict.
- **Conflicts found while they are born**: a background shadow merge of your uncommitted work against what the team has pushed warns you minutes, not days, after a collision starts; with the optional relay, seconds.
- **Merges that compile**: a semantic check after every merge reports calls that no longer match a changed contract.
- **Conflicts negotiated before the pull request**: per-symbol threads with executable proposals let two developers agree asynchronously, without being online together.
- **History per entity, not per line**: `gedo trace <symbol>` follows a function through renames, moves and reformatting; `gedo playback` shows how a delivery evolved.
- **Clean commits without effort**: semantic lanes split refactoring from feature logic into separate commits.
- **Surgical code movement**: `gedo graft` moves a symbol and its minimal dependency closure into another stream, and the duplicate is removed when both sides merge.
- **Never lose work**: every save is in the local log; `gedo recover` and `gedo undo` bring work back, and force pushes over colleagues' commits are blocked.
- **Zero cost for the rest of the team**: source files stay pure code, `refs/heads/*` stay canonical, every Git command keeps working through `gedo`, and a teammate without Gedo clones, pushes and reviews exactly as before.
- **No server to buy or run**: events and node identities travel through Git itself, in `refs/gedo/*`; a real-time relay is optional and self-hostable.

## Business Model

Open source, no revenue intended. Dual-licensed under MIT and Apache-2.0, the convention of the Rust ecosystem. The repository starts **private** and is opened once Gedo has enough traction that contributing upstream is more attractive than forking; until then the license is in place but nothing is distributed. The name and logo "Gedo" are protected by a trademark policy so that forks cannot present themselves as the project.

Costs that matter: CI minutes on a private repository (Windows minutes count double; macOS is deferred partly for this reason), code-signing certificates for released binaries, and the development time of a small team.

## Success Metrics

| Indicator | Target |
|---|---|
| Dogfooding | The Levix team uses Gedo on at least one real repository for 4 consecutive weeks after the MVP |
| Standard Git invariance | Zero corruption or divergence for collaborators using plain Git, across the whole test suite and the dogfooding period |
| Identity accuracy | At least 95% of node identities preserved on a reference corpus of renames, moves and formatter runs |
| Identity determinism | Indexing the same history on any machine produces the same node identities, on the whole reference corpus |
| Git compatibility | Every Git command run through `gedo` gives the same output, exit code and repository state as through `git` on the end-to-end suite |
| Publish latency | `gedo publish` completes in under 1 second on a medium repository (about 100k lines) |

## Scope

The complete product, as capabilities:

1. **Git object store bridge**: read and write blobs, trees and custom references under `refs/gedo/*` natively (no Git subprocess, no working-tree changes), with compare-and-swap reference updates for safe local concurrency, and transparent transport through standard `git push` / `git fetch`.
2. **Syntactic identity tracking**: incremental parsing with Tree-sitter, trivia normalization, a structural hash that ignores whitespace and comments, and the 3-tier matching algorithm (exact, signature and scope heuristics, spawn or tombstone) that keeps a node's identity through mutation, renames and moves, emitting semantic deltas. Identities are deterministic: any machine indexing the same history gets the same ids, and commits made without Gedo are indexed afterwards.
3. **Language coverage**: Rust first, then the Levix stacks (TypeScript, Python), then about 30 programming languages ordered by worldwide usage (the GitHub Octoverse and Stack Overflow Developer Survey rankings combined with Reciprocal Rank Fusion, k = 60), limited to languages with a mature Tree-sitter grammar.
4. **Data and markup formats**: structural tracking for JSON, YAML, TOML, SQL, Markdown and similar formats, after the programming languages.
5. **Event log and convergence**: an immutable local write-ahead log, event tapes robust to squash and rebase, a tree CRDT with deterministic conflict resolution (set semantics for members of a structure, idempotent events so a grafted change is never applied twice), semantic lanes (refactor and feature, split into separate commits on publish) and virtual streams (`gedo stream`) that switch or isolate work in memory without `git stash` or `git checkout`.
6. **Semantic merge and early conflict detection**: a semantic merge driver for Git, a background shadow merge of uncommitted work against the team's pushed branches, a semantic diff (`gedo delta`: moved, renamed, refactored or changed) and a semantic check after every merge.
7. **Symbol graph and contracts**: a directed dependency graph between node identities, LSP integration for precise types and diagnostics, the minimum-cut algorithm behind `gedo graft`, and detection of contract breaks before compilation and after merges that Git declares clean.
8. **Developer CLI and daemon**: the `gedo` command line (`init`, `publish`, `trace`, `graft`, `delta`, `radar`, `stream`, `doctor`, and later `inbox`, `accept`, `peek`, `playback`, `recover`, `undo`) and the `gedod` background daemon that watches the file system, feeds the parser and the WAL, and fetches the team's branches for the shadow merge.
9. **Full Git command compatibility**: every Git command works through `gedo`, transparently: Gedo commands are handled by the overlay, any other command passes straight to the user's Git with the same output and exit code, and Git hooks installed by `gedo init` (chained with existing hooks) keep the overlay correct even when `git` is called directly.
10. **Safety net**: recovery of any saved state from the local log (`gedo recover`), an operation log with `gedo undo`, and a guard against force pushes that would erase colleagues' commits.
11. **Hosting integration**: a GitHub Action (GitLab later) that posts the semantic diff and contract checks on pull requests, re-indexes after server-side merges, squashes and rebases, and complements the host's merge queue by checking that queued pull requests are semantically independent.
12. **Lockfiles and generated files**: treated as derived artifacts; merges regenerate them deterministically from the dependency changes instead of merging their text.
13. **Distributed control plane**: developer intents (`SymbolLocked`, `SymbolMutating`) and `gedo peek` into a colleague's work in progress, soft leases with heartbeat and expiry for binary files, over three tiers: background fetch through Git remotes (MVP), an optional self-hostable relay for second-level latency, and peer-to-peer in local networks.
14. **Symbol inbox**: when two developers change the same node in incompatible ways, the second one chooses to adopt the other's change, propose a unification or keep their own for now; proposals become asynchronous threads per symbol with executable snapshots, closed by `gedo accept`; changes to public contracts can ask for a quick acknowledgment from their owners while the code is still being written.
15. **History replay**: `gedo playback <commit>` shows the event-by-event evolution of a delivery and its negotiation threads, keeping the main history clean.
16. **IDE integration**: a language server and lightweight extensions (VS Code first) that show the tree status, collisions, threads and semantic locks.
17. **Platforms and distribution**: Windows and Linux first, macOS later; signed binaries; installation through `cargo install`, release archives and the platforms' package managers.
18. **Pair programming (future)**: live collaboration on the same symbols, built on the control plane, much later.

## Non-Goals

- Replacing GitHub, GitLab, their pull requests or their merge queues; Gedo works alongside and complements them.
- A mandatory proprietary or self-hosted server; the relay stays optional and everything works without it.
- A graphical client of its own; the surfaces are the CLI and IDE extensions.
- Real-time co-editing in the style of Google Docs; live pair programming is a distant, separate capability, not part of the core.
- Writing metadata, UUIDs or helper files into the working tree or into source files.
- Changing the content or meaning of `refs/heads/*`.
- Building secret scanning, large-file storage or submodule management; existing tools (secret scanners, Git LFS) keep running through the chained hooks.
- Solving team process or culture (branching policy, review habits) beyond what the tooling makes visible.

## Risks and Assumptions

| Risk or assumption | How it is reduced |
|---|---|
| The 3-tier matching does not reach the identity accuracy target on real code | Build the reference corpus first (renames, moves, formatters) and gate the AST layer on it; property-based tests; tune thresholds with data |
| Machines derive different identities for the same code, splitting history at merge time | Deterministic identity derivation and a reconciliation rule decided in an ADR before the identity phase; a determinism test on the reference corpus |
| A semantic merge produces code that differs from what a careful human would write | The driver falls back to Git's textual merge whenever it is not certain, and never hides a conflict it cannot resolve; differential tests against Git on real merge histories |
| Server-side merges, squashes and rebases detach Gedo data from commits | Data keyed by content as well as by commit, re-indexing of commits made without Gedo, and the hosting integration after server-side merges |
| Git hosts do not transfer or keep custom references (`refs/gedo/*`) as expected, or garbage-collect objects only they reference | Validate push and fetch against GitHub, GitLab and plain remotes in the first phase; document the refspec setup that `gedo init` writes |
| Event tapes and AST maps grow the repository too much | Compact binary serialization, measure repository growth in the store phase and add compaction if needed |
| The pure-Rust Git implementation (gitoxide) lacks a needed write capability | Check the required operations in the store phase; isolate gitoxide behind the store interface so a fallback is possible |
| Windows specifics (CRLF, case-insensitive paths, file locking, file watching) break hashing or byte ranges | Windows in CI from the first phase; line-ending normalization defined in the structural hash |
| Tree-sitter grammar quality varies across languages | Only languages with mature grammars; a per-language conformance suite before a language is declared supported |
| Team features need several teammates to adopt Gedo | Individual value first (merge driver, shadow merge, trace, safety net); the hosting integration shows value to teammates who do not use Gedo |
| Forks before the project has traction | Private repository until traction; trademark policy; reserve the `gedo` crate names early |
| A small team builds a large system | Strict bottom-up layers, each deterministic, offline-first and tested before the next; internal releases per phase |

## Open Questions

- When the repository becomes public, and which traction metric triggers it.
- The trademark policy and when to register the name.
- When to buy code-signing certificates (Windows Authenticode; Apple Developer Program when macOS starts).
- Whether repositories using SHA-256 object format are supported from the MVP or later.
