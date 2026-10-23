# Gedo (Git Event-Driven Overlay)

Gedo is a distributed collaboration overlay built directly on Git's object store, for software teams of any stack who want to collaborate on code entities (functions, types, modules) instead of lines of text. It turns a repository into a reactive graph of syntax nodes with stable identities, synchronized by an event-driven CRDT engine, without proprietary servers, without annotations in source files and without breaking any existing Git tool.

## Problem

Git versions text, not code. Every tool built on it (diff, blame, merge, pull requests) reasons about lines, while developers reason about symbols:

- **Blame loses history.** Moving a function to another file, renaming it or running a formatter (rustfmt, Prettier, Black) makes `git blame` attribute the whole entity to whoever touched it last. The real history of a function is lost or must be traced by hand.
- **Merges conflict on text, not meaning.** Two developers who change different functions that happen to be close in a file get a textual conflict; a rename on one side and a body change on the other become a manual merge.
- **Moving code between branches is coarse.** `git cherry-pick` moves whole commits. Moving one function with exactly the dependencies it needs, into another branch, is manual and error-prone.
- **Collaboration is blind until the pull request.** A teammate only learns that someone else is reshaping the same symbol when the pull request is opened, after the work is done.
- **Reviews are file-shaped.** Pull requests show changed lines across files, not the set of symbols whose behavior or contract changed.

Today teams cope with conventions (small pull requests, "who is touching X?" messages), with formatter-only commits listed in `.git-blame-ignore-revs`, and with manual archaeology through `git log -S`. Existing semantic tools either need a proprietary server, replace Git, or store metadata inside the working tree.

## Target Users

- **Levix Digital development team (primary).** Daily users of Rust, TypeScript and Python, working on several products in parallel. They want a precise per-symbol history, fewer textual conflicts and the ability to move a function with its dependencies between work streams, without changing their hosting (GitHub) or their teammates' tools. Gedo is built and dogfooded by this team first.
- **Small and medium development teams, any stack (later).** Teams of roughly 2 to 50 developers on GitHub, GitLab or self-hosted Git. They want semantic collaboration without migrating away from Git, without a new server to operate and without forcing every collaborator to install a new tool.

## Value Proposition

- **History per entity, not per line**: `gedo blame <symbol>` follows a function through renames, moves and reformatting.
- **Zero cost for the rest of the team**: source files stay pure code, `refs/heads/*` stay canonical, and a teammate without Gedo clones, pushes and reviews exactly as before.
- **No server to buy or run**: events and node identities travel through Git itself, in `refs/gedo/*`.
- **Surgical code movement**: `gedo graft` moves a symbol and its minimal dependency closure into another stream.
- **Fewer textual conflicts**: concurrent changes are ordered and converged per syntax node instead of per line.
- **Offline-first**: everything works locally; synchronization happens whenever Git does.

## Business Model

Open source, no revenue intended. Dual-licensed under MIT and Apache-2.0, the convention of the Rust ecosystem. The repository starts **private** and is opened once Gedo has enough traction that contributing upstream is more attractive than forking; until then the license is in place but nothing is distributed. The name and logo "Gedo" are protected by a trademark policy so that forks cannot present themselves as the project.

Costs that matter: CI minutes on a private repository (Windows minutes count double; macOS is deferred partly for this reason), code-signing certificates for released binaries, and the development time of a small team.

## Success Metrics

| Indicator | Target |
|---|---|
| Dogfooding | The Levix team uses Gedo on at least one real repository for 4 consecutive weeks after the MVP |
| Standard Git invariance | Zero corruption or divergence for collaborators using plain Git, across the whole test suite and the dogfooding period |
| Identity accuracy | At least 95% of node identities preserved on a reference corpus of renames, moves and formatter runs |
| Publish latency | `gedo publish` completes in under 1 second on a medium repository (about 100k lines) |

## Scope

The complete product, as capabilities:

1. **Git object store bridge**: read and write blobs, trees and custom references under `refs/gedo/*` natively (no Git subprocess, no working-tree changes), with compare-and-swap reference updates for safe local concurrency, and transparent transport through standard `git push` / `git fetch`.
2. **Syntactic identity tracking**: incremental parsing with Tree-sitter, trivia normalization, a structural hash that ignores whitespace and comments, and the 3-tier matching algorithm (exact, signature and scope heuristics, spawn or tombstone) that keeps a node's identity through mutation, renames and moves, emitting semantic deltas.
3. **Language coverage**: Rust first, then the Levix stacks (TypeScript, Python), then about 30 programming languages ordered by worldwide usage (a combination of the GitHub Octoverse and the Stack Overflow Developer Survey), limited to languages with a mature Tree-sitter grammar.
4. **Data and markup formats**: structural tracking for JSON, YAML, TOML, SQL, Markdown and similar formats, after the programming languages.
5. **Event log and convergence**: an immutable local write-ahead log, event tapes per canonical commit, a tree CRDT with deterministic conflict resolution, semantic lanes (refactor and feature) and virtual streams that switch or isolate experiments in memory without `git stash` or `git checkout`.
6. **Symbol graph and contracts**: a directed dependency graph between node identities, LSP integration for precise types and diagnostics, the minimum-cut algorithm behind `gedo graft`, and detection of silent API contract breaks before compilation.
7. **Developer CLI and daemon**: the `gedo` command line (`init`, `status`, `publish`, `blame`, `graft`, and later `inbox`) and the `gedod` background daemon that watches the file system and feeds the parser and the WAL.
8. **Distributed control plane**: an asynchronous pub/sub bus for developer intents (`SymbolLocked`, `SymbolMutating`), an ingress queue with a deterministic sequencer that materializes canonical commits without textual conflicts, over Git remotes first and WebSocket or peer-to-peer in local networks later.
9. **Symbol inbox**: asynchronous negotiation and review per symbol instead of per pull request.
10. **IDE integration**: a language server and lightweight extensions (VS Code first) that show the tree status and semantic locks.
11. **Platforms and distribution**: Windows and Linux first, macOS later; signed binaries; installation through `cargo install`, release archives and the platforms' package managers.
12. **Pair programming (future)**: live collaboration on the same symbols, built on the control plane, much later.

## Non-Goals

- Replacing GitHub, GitLab or any Git hosting; Gedo works alongside them.
- A mandatory proprietary or self-hosted server; any optional relay must stay optional.
- A graphical client of its own; the surfaces are the CLI and IDE extensions.
- Real-time co-editing in the style of Google Docs; live pair programming is a distant, separate capability, not part of the core.
- Writing metadata, UUIDs or helper files into the working tree or into source files.
- Changing the content or meaning of `refs/heads/*`.

## Risks and Assumptions

| Risk or assumption | How it is reduced |
|---|---|
| The 3-tier matching does not reach the identity accuracy target on real code | Build the reference corpus first (renames, moves, formatters) and gate the AST layer on it; property-based tests; tune thresholds with data |
| Git hosts do not transfer or keep custom references (`refs/gedo/*`) as expected, or garbage-collect objects only they reference | Validate push and fetch against GitHub, GitLab and plain remotes in the first phase; document the refspec setup that `gedo init` writes |
| Event tapes and AST maps grow the repository too much | Compact binary serialization, tapes per commit, measure repository growth in the store phase and add compaction if needed |
| The pure-Rust Git implementation (gitoxide) lacks a needed write capability | Check the required operations in the store phase; isolate gitoxide behind the store interface so a fallback is possible |
| Windows specifics (CRLF, case-insensitive paths, file locking, file watching) break hashing or byte ranges | Windows in CI from the first phase; line-ending normalization defined in the structural hash |
| Tree-sitter grammar quality varies across languages | Only languages with mature grammars; a per-language conformance suite before a language is declared supported |
| Forks before the project has traction | Private repository until traction; trademark policy; reserve the `gedo` crate names early |
| A small team builds a large system | Strict bottom-up layers, each deterministic, offline-first and tested before the next; internal releases per phase |

## Open Questions

- When the repository becomes public, and which traction metric triggers it.
- The trademark policy and when to register the name.
- When to buy code-signing certificates (Windows Authenticode; Apple Developer Program when macOS starts).
- Whether repositories using SHA-256 object format are supported from the MVP or later.
