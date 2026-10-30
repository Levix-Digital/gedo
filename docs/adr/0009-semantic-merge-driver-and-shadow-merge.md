---
id: 9
title: Semantic merge driver and shadow merge
status: accepted
date: '2026-10-24'
version: v0.1.0
subsystems: [events, surfaces, store]
supersedes: []
superseded_by: []
amends: [6]
amended_by: [11, 17]
---

# ADR-0009: Semantic merge driver and shadow merge

## Context

Gedo's promise of fewer and earlier conflicts only holds if Gedo takes part in merges and sees collisions before they reach a merge. With ADR-0006, `git merge`, `rebase` and `pull` pass straight to Git, which merges text; the sequencer of the blueprint never runs in the team's real workflow. The early discussions also describe a continuous "shadow merge": the daemon merging the developer's uncommitted work, in memory, with every advance of the main branch.

## Decision

- **Semantic merge driver.** `gedo init` registers a Git merge driver in the repository's local configuration (`merge.gedo.driver` in `.git/config`) and assigns it to the supported languages in `.git/info/attributes`, which is local and never committed. Git then calls Gedo for every file it must merge, whether the user typed `gedo` or `git`, from the CLI, an IDE or a GUI. A team may also commit a `.gitattributes` entry if it chooses; Gedo never writes one.
- **Merge rules.** The driver parses base, ours and theirs, matches identities and merges per syntax node with the CRDT rules: members of a structure (fields, methods, imports, enum variants) behave as a set, a move and an edit of the same node commute, trivia follow the destination's formatting, and an event present on both sides (a graft) is applied once.
- **Never hide a conflict.** When the language is not supported, a file does not parse, or two sides change the same node incompatibly, the driver produces Git's normal textual merge (computed with gitoxide's merge, no subprocess) with conflict markers limited to the conflicting node where possible, and records the conflict for the inbox.
- **Shadow merge.** `gedod` fetches the tracked remote branches in the background (every 5 minutes by default, configurable, paused on metered or offline networks) and merges, in memory, the working tree and local commits with the upstream branch and with every remote branch that touches the same nodes. Collisions are reported through `gedo radar`, a desktop notification and, later, the IDE. Nothing is written to the working tree or to local branches.
- **Semantic diff and post-merge check** use the same engine: `gedo delta` classifies changes as moved, renamed, refactored or changed, and the `post-merge` and `post-rewrite` hooks report calls that no longer match a changed contract.
- **Subprocess scope (amends ADR-0006).** The background fetch runs the user's `git fetch`, for the same reason as the pass-through: it must use the user's remotes, credentials and network configuration. Merging, diffing and the store still never spawn `git`.

## Alternatives Considered

- **Merging only inside `gedo` commands**: the overlay would be bypassed by every IDE, GUI and teammate habit.
- **Writing `.gitattributes` in the repository**: shares the setup automatically, but pollutes the working tree and affects teammates without Gedo.
- **Resolving every conflict automatically**: tempting, but a wrong silent merge is worse than a visible conflict.
- **Fetching through gitoxide**: avoids a subprocess, but credential helpers, SSH agents and corporate proxies are configured for the user's Git, and failures there would be silent.

## Consequences

- Teammates benefit from semantic merges only after running `gedo init` in their clone; the hosting integration covers merges done on the server.
- The driver is on the critical path of every merge: it must be fast, never crash Git and fall back on any internal error.
- Differential tests replay real merge histories through Git and through the driver and compare results.
- Background fetch updates remote-tracking references, exactly as a manual `git fetch` would.
