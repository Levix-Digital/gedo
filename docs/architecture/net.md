---
title: Transport Adapter
summary: Carries events and intents between developers.
code: [crates/adapters/gedo-net]
---

# Transport Adapter

Carries events and intents between developers.

## Responsibilities

- Implements the transport port: exchanging tapes, threads and developer intents (`SymbolLocked`, `SymbolMutating`).
- Three tiers (ADR-0010): Git remotes with background fetch (MVP); an optional self-hostable relay for intents, `gedo peek`, soft leases for binary files and inbox threads; peer-to-peer in local networks. Git remains the canonical store in every tier.
- Does not order or merge events; the deterministic sequencer is core logic (`gedo-crdt`).

## Structure

One module per transport. Asynchronous code with `tokio`, which stays out of the core.

## Behavior

Built after the MVP. Until then, Git remotes carry all shared data through standard `git fetch` and `git push`.

## Interfaces

- Implements the transport port of `gedo-core`; wired by `gedod`.

## Decisions

- [ADR-0005](../adr/0005-serverless-operation-with-git-remotes-as-the-only-transport.md): serverless, Git remotes first.
- [ADR-0010](../adr/0010-three-tiers-of-real-time-coordination-and-merge-queue-comple.md): three tiers of real-time coordination; the merge queue is complemented, not replaced.
