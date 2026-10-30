---
id: 16
title: Re-attachment by entity state with a semantic-signature fast path
status: accepted
date: '2026-10-30'
version: v0.3.0
subsystems: [events]
supersedes: []
superseded_by: []
amends: [8]
amended_by: []
---

# ADR-0016: Re-attachment by entity state with a semantic-signature fast path

## Context

ADR-0008 makes Gedo data survive history rewrites: tapes and events name no commit, and a rewritten commit gets index entries for the tapes of the commits it replaces. It names Git's patch-id as the fast path for local rebases and matching on "the same target, operation and resulting structural hash" otherwise. Implementing it (`specs/005-history-reattachment`) showed three gaps. A patch-id needs a line diff of every changed file, which the core does not have (feature 003 diffs trees and parses), and it changes whenever a rebase resolves a conflict or shifts context, even when the commit still makes the same semantic changes. A squash replaces each sequence of operations on an entity by its net effect: a function edited twice appears as one body change, and a rename followed by an edit as a rename and a body change with the final hash, so operation-by-operation matching fails on the common case. An entity created on the branch has, in the squash commit, the id derived from its final content (ADR-0007), not the id in the live events. TECH_SPEC keeps detached events 90 days, but no machine can know when another first saw a tape detached.

## Decision

- **Fast path by semantic signature.** For a pair `(old, new)` reported by Git's `post-rewrite` hook, Gedo compares the semantic signatures of the two commits: BLAKE3 (`gedo semantic signature v1`) over their deltas and identity merges, in canonical order, with each entity's structural hash after the change. Equal signatures attach every tape of the old commit to the new one. The signature is never stored.
- **Matching by entity state.** Otherwise, the candidate events (the tapes of the old commits of the pairs, or, for commits fetched without pairs, the tapes indexed for no commit reachable from the given tips) are folded per entity, in the total order of events. A change of the new commit matches the latest event of the entity that leaves it in the same state (same structural hash, or removed); the entity's earlier events are superseded, its later ones are not accounted for. Entities with no net effect (created then removed, changed then changed back) on which the commit has no change are neutral. Identity merges in the candidates are accounted for when the commit or the identity index agrees with them.
- **Attachment rule.** With a pair, a tape is attached when one of its events is matched or superseded. Without a pair, every one of its events must be matched, superseded or neutral, and at least one matched or superseded. Commits of one call are processed in generation order, and a tape attached to an ancestor in the call is no candidate for its descendants. Only commit index entries are written.
- **Identity links.** A creation in the new commit with no candidate events matches the candidate entity created on the branch with the same kind, qualified path and final structural hash (the lowest id on ties). The two ids are linked as a reconstructed identity merge of the new commit, the lower id kept (ADR-0007), derived on demand like every reconstructed event (ADR-0012): nothing new is shared.
- **Events of a commit with tapes.** `events_of_commit` returns the tapes and, after them, the reconstructed events of the changes no tape event accounts for, then the links.
- **Local retention date.** A tape is detached when none of its commits is reachable from the tips the caller gives. The time this machine first saw it detached is kept in the symbol cache (namespace `retention`); a tape is eligible for removal by `gedo gc` (FEAT-087) after the grace period (90 days by default). A lost record restarts the period.

## Alternatives Considered

- **Git's patch-id**: needs a diff engine in the core and fails on resolved conflicts, which the semantic signature handles.
- **Operation-by-operation matching (ADR-0008 as written)**: fails on every squash that touches an entity more than once.
- **A shared identity-merge event for each link**: a new shared write per squash for a fact every machine can derive from the commit and its tapes.
- **Keeping the live id regardless of order**: a second alias rule beside ADR-0007's.
- **A shared retention date**: a new shared record and conflicts between machines for a date that only matters to the local `gedo gc`.
- **Requiring every event to be matched without a pair, with no supersession**: the intermediate saves and commits of a branch would never re-attach.

## Consequences

- `gedo trace` and `gedo playback` keep the reasoning of a branch after a server squash or rebase merge, with the original ids, authors, agents and labels.
- Known limit: without a pair, a detached tape of an unrelated branch whose events all concern entities that the new commit leaves in the state of a later matched event is attached too. The CRDT (FEAT-066) applies an attached event once, so the effect is an extra tape in an entity's history, never a wrong code state. Covered by a unit test of `gedo-crdt`.
- Re-attachment of one fetched commit takes about 19 ms at p95 on Linux with 10,000 stored events of which 1,000 are detached, and a rebased commit about 1.6 ms through the fast path (`cargo bench -p gedo-app --bench reattach`).
- Verified by the property tests of `gedo-crdt` (generated branches and their squash, candidate order, unrelated branches) and the scripted rewrites of `crates/core/gedo-app/tests/reattach.rs` with a snapshot of the index entries.
