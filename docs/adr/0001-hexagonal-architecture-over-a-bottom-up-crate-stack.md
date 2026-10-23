---
id: 1
title: Hexagonal architecture over a bottom-up crate stack
status: accepted
date: '2026-10-23'
version: v0.1.0
subsystems: [core, identity, events, graph, store, parsing, net, surfaces]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0001: Hexagonal architecture over a bottom-up crate stack

## Context

The engineering blueprint (v1.0) splits Gedo into six layer crates, each depending only on the ones below: `gedo-store` (Git object store), `gedo-ast` (syntax and node identity), `gedo-crdt` (event log and convergence), `gedo-graph` (symbol graph), `gedo-net` (distributed control plane) and `gedo-cli` / `gedod` (developer surfaces). In that stack the lowest layer is infrastructure (gitoxide), so every domain algorithm depends, directly or transitively, on Git access.

The blueprint also requires every layer to be deterministic, offline-first and covered by unit and property-based tests before the next layer starts. The most valuable and riskiest logic (3-tier identity matching, tree CRDT convergence, minimum-cut graft, deterministic sequencing) must therefore be testable on plain data, thousands of times per second, with no repository on disk. Several surfaces (CLI, daemon, later a language server and IDE extensions) must behave identically on one canonical state.

## Decision

Gedo follows the **hexagonal (ports and adapters)** style. The blueprint's layers are kept as the **order of construction** (Phases 1 to 4 still go bottom-up), but the dependency direction points toward a pure core:

```
crates/
  core/                 # the hexagon: no I/O, no gix, tree-sitter, redb, tokio or notify
    gedo-core/          # shared model (GitOid, AstNodeId, BlobAstMap, events) and the ports (traits)
    gedo-ast/           # trivia normalization, structural hash, 3-tier matching, semantic deltas
    gedo-crdt/          # event tapes, Lamport clocks, tree CRDT, semantic lanes, virtual streams, sequencer
    gedo-graph/         # symbol dependency graph, minimum-cut graft, contract-break detection
    gedo-app/           # use cases: init, ingest change, publish, blame, graft, status
  adapters/             # driven adapters, implementing the core's ports
    gedo-store/         # gitoxide object and reference store, WAL file, redb cache, in-memory store
    gedo-parse/         # Tree-sitter grammars translated into the core's neutral syntax tree
    gedo-net/           # transports for the control plane (Git remotes; later WebSocket / P2P)
  apps/                 # driving adapters and composition roots
    gedo-cli/           # the `gedo` binary
    gedod/              # the `gedod` daemon (file-system watcher)
```

Rules:
- Crates under `crates/core` depend only on each other and on plain libraries (serialization, hashing, ids, collections). `architecture.forbidden_core_imports` lists the adapter crates and infrastructure libraries, and `software-design/check_design.py` enforces it.
- The ports live in `gedo-core`: object and reference store (with compare-and-swap reference updates), write-ahead log, symbol cache, parser, clock and transport. Each port has a real adapter and an in-memory adapter used by the core's tests.
- Every user action is one use case in `gedo-app`. The CLI and the daemon only parse input, call a use case and render its result; they keep no rules of their own. Both read and write the same canonical state: `refs/gedo/*` in the Git object store plus the local WAL.
- Each executable has one composition root that builds the adapters and wires them into the use cases.

## Alternatives Considered

- **Strict layered crate stack, as written in the blueprint, with a pure core inside each crate**: closest to the original document and with fewer crates, but domain crates would depend on `gedo-store` and therefore on gitoxide, so tests of the identity and CRDT logic would pull infrastructure in, and nothing would stop I/O from creeping into the algorithms.
- **One `gedo-core` crate holding the whole domain**: fewer crates, but it loses the per-concern boundaries of the blueprint and slows incremental compilation of the most frequently changed code.

## Consequences

- The core algorithms are tested on plain data with property-based tests and no repository on disk; adapters get their own integration tests against real repositories.
- More crates than the blueprint (ten instead of six) and a small amount of mapping between adapter types (gitoxide, Tree-sitter) and core types.
- New surfaces (language server, IDE extensions) and new transports are added as adapters without touching the core.
- Verification: `check_design.py` passes on every pull request; the core crates build without the adapter crates.
