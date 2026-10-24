---
id: 5
title: Serverless operation with Git remotes as the only transport
status: accepted
date: '2026-10-23'
version: v0.1.0
subsystems: [net, store]
supersedes: []
superseded_by: []
amends: []
amended_by: [10]
---

# ADR-0005: Serverless operation with Git remotes as the only transport

## Context

The vision rules out a mandatory server. The blueprint describes a hybrid transport: Git remotes (`refs/gedo/*`) and, in corporate local networks, WebSocket or peer-to-peer. The MVP (Phases 1 to 4) is used by one team on GitHub.

## Decision

- Until the distributed control plane is built (after the MVP), the only transport is the repository's existing Git remotes, through the refspecs written by `gedo init` (ADR-0003). Any host that accepts custom references works: GitHub, GitLab, plain SSH or file remotes.
- `gedo-net` is the adapter crate for transports behind the core's transport port; WebSocket and peer-to-peer are later adapters and always optional.
- The project itself is hosted in a private GitHub repository with GitHub Actions running on Windows and Linux.

## Alternatives Considered

- **A relay server from the start**: enables real-time intents earlier, but adds an operated service and contradicts the serverless goal.
- **Peer-to-peer from the start**: hard on corporate networks (NAT, firewalls) and not needed by a single team in the MVP.

## Consequences

- Intents (`SymbolLocked`, `SymbolMutating`) are only as fresh as the last fetch until the real-time transports exist.
- No infrastructure to operate or pay for beyond the Git host and CI.
