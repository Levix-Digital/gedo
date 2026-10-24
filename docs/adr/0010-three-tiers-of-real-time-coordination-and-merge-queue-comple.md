---
id: 10
title: Three tiers of real-time coordination and merge queue complement
status: accepted
date: '2026-10-24'
version: v0.1.0
subsystems: [net, surfaces]
supersedes: []
superseded_by: []
amends: [5]
amended_by: []
---

# ADR-0010: Three tiers of real-time coordination and merge queue complement

## Context

The early discussions place a low-latency broker (for example NATS JetStream) at the center: intents published on save, collisions detected in milliseconds, and an ingress queue with a sequencer replacing direct pushes to the main branch. ADR-0005 chose serverless operation with Git remotes as the only transport, which keeps the product free of infrastructure but limits collision detection to the freshness of the last fetch. Meanwhile, GitHub and GitLab already offer merge queues, and the team merges through pull requests on the host.

## Decision

- **Three tiers, each optional above the first:**
  1. **Git remotes (MVP)**: background fetch and shadow merge (ADR-0009); latency of minutes; no infrastructure.
  2. **Optional relay**: a self-hostable relay for intents (`SymbolLocked`, `SymbolMutating`), `gedo peek`, soft leases for binary files and inbox threads; latency of seconds. The relay only accelerates coordination: Git remains the canonical store, and every machine works when the relay is down. The relay technology is chosen in its own ADR when the tier is built.
  3. **Peer-to-peer in local networks**: the same messages without a relay, for corporate networks.
- **Complement the host's merge queue, do not replace pushes.** The deterministic sequencer (core logic in `gedo-crdt`) is used by the merge driver, by the relay and by the hosting integration. On GitHub, a Gedo Action checks that the pull requests in the merge queue are semantically independent, runs the post-merge contract check and re-attaches Gedo data after server-side merges. Pushing to a branch keeps its normal Git meaning.
- This amends ADR-0005: Git remotes are the only transport of the MVP, and the later transports are the optional tiers above.

## Alternatives Considered

- **A mandatory broker from the start**: the lowest latency, but an operated service for every team and a single point of failure, against the vision's non-goals.
- **Git remotes forever**: no infrastructure, but intents and negotiation would never be close to real time.
- **Replacing pushes with Gedo's own ingress queue**: matches the early discussions, but duplicates the host's merge queue, bypasses branch protection and reviews, and forces every teammate onto Gedo.

## Consequences

- The MVP needs no infrastructure and still detects collisions within minutes.
- The relay and peer-to-peer tiers are adapters of the transport port in `gedo-net`; nothing in the core depends on them.
- The hosting integration becomes a product surface of its own (a GitHub Action first).
