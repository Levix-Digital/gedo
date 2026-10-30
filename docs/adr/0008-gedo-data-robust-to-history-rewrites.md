---
id: 8
title: Gedo data robust to history rewrites
status: accepted
date: '2026-10-24'
version: v0.1.0
subsystems: [store, events]
supersedes: []
superseded_by: []
amends: [3]
amended_by: [14, 16]
---

# ADR-0008: Gedo data robust to history rewrites

## Context

ADR-0003 stores the event tape of each canonical commit at `refs/gedo/tapes/<commit-sha>`. Commit ids change whenever history is rewritten: a local rebase or amend, and above all server-side operations Gedo does not see (GitHub's squash merge and rebase merge). After a squash, the tapes of the branch's commits point to commits that are no longer reachable, and the main history has a commit with no tape at all. Teams squash by default, so this is the normal case, not an edge case.

## Decision

- **Events are content-addressed and stored independently of commits.** Each event is stored by its id; tapes group event ids, and a separate index maps commits to the tapes that produced them.
- **AST maps stay keyed by blob id** (`refs/gedo/ast-map/<blob-sha>`), which is already content-addressed and survives any rewrite.
- **Re-attachment after a rewrite.** For a commit without an index entry (rewritten, squashed, or made without Gedo), Gedo derives its semantic delta from the parent tree to the commit tree, reconstructs the events deterministically (ADR-0007) and matches them against stored events with the same target, operation and resulting structural hash. Matched events keep their original id, author and threads; unmatched ones are recorded as reconstructed. A patch-id comparison is the fast path for local rebases.
- **Triggers.** The `post-rewrite` hook re-attaches after local rebases and amends; the fetch path (CLI, daemon and hosting integration) re-attaches commits that arrive from the server.
- **Retention.** Events no longer reachable from any indexed commit are kept for a grace period (90 days by default) before `gedo gc` removes them, so a squash never loses the history `gedo playback` needs.
- The reference layout of ADR-0003 is otherwise unchanged; how events, tapes and indexes are packed into few references is decided in Phase 1 from measurements.

## Alternatives Considered

- **Tapes keyed by commit only (ADR-0003 as written)**: simplest, but loses the history at every squash.
- **Commit trailers pointing to tapes**: survive some rewrites, but write Gedo metadata into commit messages, which the team sees and tools parse.
- **Asking teams not to squash**: shifts the problem to the users and contradicts "zero cost for the rest of the team".

## Consequences

- `gedo trace` and `gedo playback` keep working on squashed histories.
- Re-attachment costs one semantic diff per new commit on fetch; it is incremental and counts toward the performance requirements.
- Commits made without Gedo get full identity and history, with reconstructed events instead of live ones.
