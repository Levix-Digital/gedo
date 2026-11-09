---
title: Developer Surfaces
summary: The gdo CLI, the gedod daemon and the use cases they call.
code: [crates/core/gedo-app, crates/apps/gedo-cli, crates/apps/gedod]
---

# Developer Surfaces

The gdo CLI, the gedod daemon and the use cases they call.

## Responsibilities

- `gedo-app` (core): one use case per user action: `init`, ingest a file change, `publish`, `trace`, `graft`, `delta`, `radar`, `stream`, `doctor`, merge a file (merge driver), shadow merge, sync after a Git event (commit, checkout, merge, rewrite, push), and later `inbox`, `accept`, `peek`, `playback`, `recover`, `undo`, `stack`. The first one built is the indexing of history (`IdentityIndex`, see [Node Identity](identity.md)), which `init` and the sync after a fetch will call.
- `gedo-cli`: the binary, installed as `gdo` (the command people type) and as `gedo` (the name hooks, the merge driver and scripts call); dispatches each invocation to a use case or passes it through to Git, and renders results.
- `gedod`: the background daemon; watches the file system, and on each save calls the ingest use case (parse, identity, WAL); fetches the tracked remote branches in the background and runs the shadow merge.
- A GitHub Action (after the MVP): semantic diff and contract checks on pull requests, re-attachment after server-side merges, merge queue checks.
- Git hooks installed by `gdo init`: call the sync use case so the overlay stays correct when `git` is used directly.
- Each binary has one composition root that builds the adapters and wires them into the use cases. No surface keeps rules of its own.

## Structure

```mermaid
flowchart LR
    User[gdo ...] --> Disp{dispatcher}
    Disp -- Gedo command --> App[gedo-app]
    Disp -- any other command --> Git[user's git]
    Git -- hooks --> App
    Daemon[gedod] --> App
    App --> AST[gedo-ast]
    App --> CRDT[gedo-crdt]
    App --> Graph[gedo-graph]
    App --> Ports[ports in gedo-core]
```

## Behavior

**Gedo commands**

| Command | Use case |
|---|---|
| `gdo init` | Prepare `.git/gedo/`, write the `refs/gedo/*` refspecs, install and chain the hooks, build the first AST maps |
| `gdo publish [--split-lanes]` | Pack the WAL into canonical tapes and create the native Git commit; with `--split-lanes`, `EventLog::publish_split` writes a refactor commit followed by a feature commit (or one commit and the reason) and the command moves the branch to the last one |
| `gdo delta` | Semantic diff: moved, renamed, refactored or changed |
| `gdo stream <name>` | Switch to or create a virtual stream (`Streams::create`, `switch`); `gdo stream list --label key=value --json`, `publish` and `discard` call `Streams::list`, `publish` and `discard`, and the command writes the returned work tree to the working files |
| `gdo radar` | Collisions found by the shadow merge and overlay state |
| `gdo doctor` | Diagnostics: versions, paths, hook chain, merge driver, overlay consistency |
| `gedo merge-driver` | Called by Git for each file to merge; not typed by users |
| `gdo trace <symbol>` | Semantic history per entity, not per line |
| `gdo graft <symbol-path> --into <target-stream>` | Move a symbol and its minimal dependencies into another stream |
| `gdo inbox`, `gdo accept <proposal>` | After the MVP: asynchronous negotiation per symbol |
| `gdo peek @dev <symbol>` | After the MVP: a colleague's work in progress on a symbol (relay tier) |
| `gdo playback <commit>` | After the MVP: event-by-event evolution of a delivery |
| `gdo stack` | After the MVP: split work into stacked pull requests and restack them when a lower level changes or merges |
| `gdo recover`, `gdo undo` | After the MVP: restore saved states from the WAL; undo the last operations |

**Command names**: the CLI is one binary installed under two names. People type `gdo`, three letters like `git`; hooks, the merge driver and scripts call `gedo`. Both names behave the same; the daemon is `gedod` ([ADR-0021](../adr/0021-short-gdo-command-with-gedo-as-a-second-name-of-the-same-bin.md)).

`gdo init` and `gdo gc` share their names with Git, so they keep Git's behavior: they run `git init` or `git gc` first, unchanged, and then set up or prune the overlay.

**Git commands**: any other invocation, including unknown commands and the user's aliases, runs the user's `git` with the same arguments, inherited standard input, output and error, and the same exit code. A command with a Git name always keeps Git's behavior and output.

**Hooks**: `post-commit`, `post-checkout`, `post-merge`, `post-rewrite`, `pre-push` and `reference-transaction` call `gedo` (the long name of the same binary, ADR-0021) to sync the overlay. Hooks that existed before (in `.git/hooks`, a custom `core.hooksPath`, or from Husky, lefthook or pre-commit) are chained in their original order and their exit codes are respected.

A language server and VS Code extension are later driving adapters over the same use cases.

## Interfaces

- `gedo-app` calls the identity, events and graph crates and the ports of `gedo-core`.
- The binaries call only `gedo-app`, the adapter constructors and, for pass-through, the user's `git`.

## Decisions

- [ADR-0001](../adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md): surface parity over one canonical state.
- [ADR-0006](../adr/0006-git-command-pass-through-with-hook-based-augmentation.md): Git pass-through, hook-based augmentation and `gdo trace`.
- [ADR-0009](../adr/0009-semantic-merge-driver-and-shadow-merge.md): merge driver, shadow merge and background fetch.
- [ADR-0010](../adr/0010-three-tiers-of-real-time-coordination-and-merge-queue-comple.md): hosting integration and merge queue complement.
- [ADR-0013](../adr/0013-versioned-json-output-as-the-contract-for-tools-built-on-ged.md): versioned `--json` output, capabilities and exit codes as the contract for tools built on Gedo.
- [ADR-0021](../adr/0021-short-gdo-command-with-gedo-as-a-second-name-of-the-same-bin.md): the short `gdo` command, with `gedo` as a second name of the same binary for hooks, the merge driver and scripts.
