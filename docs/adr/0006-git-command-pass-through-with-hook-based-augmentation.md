---
id: 6
title: Git command pass-through with hook-based augmentation
status: accepted
date: '2026-10-23'
version: v0.1.0
subsystems: [surfaces, store]
supersedes: []
superseded_by: []
amends: [1, 2]
amended_by: []
---

# ADR-0006: Git command pass-through with hook-based augmentation

## Context

Developers should be able to use `gedo` as their only command: every Git command must work through it, transparently, while Gedo-specific commands are handled by the overlay. At the same time, teammates, IDEs and graphical clients keep calling `git` directly, and the overlay must stay correct when they do. ADR-0002 rules out spawning the `git` process, which was written for the store and the core, not for running commands the user typed. The blueprint also names a semantic command `gedo blame <symbol>`, which collides with `git blame`.

## Decision

- **Dispatcher in `gedo-cli`**: every invocation of `gedo` falls into one of three groups:
  - **Gedo commands** (`init`, `publish`, `trace`, `graft`, and later `inbox`): handled by the use cases in `gedo-app`.
  - **Augmented Git commands** (`commit`, `push`, `fetch`, `pull`, `checkout`, `switch`, `merge`, `rebase`, and others when needed): passed to Git unchanged; the overlay work happens in the Git hooks below, so the result is the same whether the user typed `gedo` or `git`.
  - **Pure pass-through**: any other command, including unknown commands and the user's Git aliases, runs the user's installed `git` with the same arguments, inherited standard input, output and error (colors, pager and prompts keep working) and the same exit code.
- **Names**: a command with a Git name always keeps Git's behavior and output. Gedo-specific behavior uses names Git does not have. The semantic history per entity is **`gedo trace <symbol>`** (the blueprint's `gedo blame <symbol>` and the `blame` use case named in ADR-0001 are renamed accordingly).
- **Hooks as the source of truth for augmentation**: `gedo init` installs Git hooks (`post-commit`, `post-checkout`, `post-merge`, `post-rewrite`, `pre-push`, `reference-transaction`) that call `gedo` to keep the overlay in step. Existing hooks are **chained**, never replaced: hooks already in `.git/hooks`, a custom `core.hooksPath` and hook managers (Husky, lefthook, pre-commit) keep running, in their original order, and their exit codes are respected. The `gedod` daemon covers working-tree changes between commands.
- **Subprocess scope (amends ADR-0002)**: spawning `git` is allowed only in the pass-through of `gedo-cli`. The store adapter and the core never spawn `git`.

## Alternatives Considered

- **Augmentation inside the wrapper only**: simpler, but the overlay drifts whenever someone runs `git` directly, which breaks the invariance principle.
- **Re-implementing Git commands on gitoxide**: huge scope and a permanent risk of behaving differently from the user's Git.
- **Shadowing `git` (an alias or a `git` executable earlier in `PATH`)**: invisible to the user, but surprising, hard to debug and fragile across shells and IDEs.
- **`gedo blame --symbol <symbol>`**: keeps the familiar word, but a flag that changes the meaning of a Git command breaks the rule that Git names keep Git behavior.

## Consequences

- `gedo` can replace `git` in daily use with no loss of functionality; Git remains a runtime requirement on the user's machine.
- Hook chaining must be tested with the common hook managers on Windows and Linux.
- Hook overhead counts toward the latency requirements of the technical specification.
- Verification: an end-to-end suite runs the same Git scenarios through `git` and through `gedo` and compares outputs, exit codes and resulting overlay state.
