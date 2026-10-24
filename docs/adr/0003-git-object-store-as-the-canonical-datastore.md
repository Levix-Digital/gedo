---
id: 3
title: Git object store as the canonical datastore
status: accepted
date: '2026-10-23'
version: v0.1.0
subsystems: [store, events]
supersedes: []
superseded_by: []
amends: []
amended_by: [8, 11]
---

# ADR-0003: Git object store as the canonical datastore

## Context

Gedo must keep node identities, event tapes and negotiation threads for a repository, share them with teammates and survive clones, without a server, without files in the working tree and without changing `refs/heads/*` (the blueprint's non-negotiable principles). Git's object database is already a content-addressed store that every developer and every host replicates.

## Decision

- Shared state lives as ordinary Git blobs and trees referenced from a dedicated namespace:
  - `refs/gedo/tapes/<commit-sha>`: the CRDT event tape of that canonical commit;
  - `refs/gedo/ast-map/<blob-sha>`: the `BlobAstMap` anchoring syntax nodes of that blob to `AstNodeId`s;
  - `refs/gedo/threads/<thread-uuid>`: the history of intents and negotiations about a symbol.
- Strictly local runtime state lives in `.git/gedo/` (never pushed): `wal.log` (micro-events not yet consolidated) and `symbol_cache.db` (ADR-0004).
- `refs/heads/*` and the working tree are never written by Gedo, except when a use case materializes a canonical commit explicitly requested by the user (`gedo publish`).
- `gedo init` adds the fetch and push refspecs for `refs/gedo/*` to the repository's remote configuration, so standard `git fetch` and `git push` carry the namespace.
- Reference updates use compare-and-swap so the CLI and the daemon can write concurrently without losing updates.

## Alternatives Considered

- **Git notes (`refs/notes/*`)**: attached to commits only, awkward for per-blob and per-symbol data, and rewritten by some tools.
- **Files in the working tree (`.gedo/` folder)**: violates the zero code pollution principle and shows up in every diff.
- **An external database or service**: needs a server, breaks offline-first and the serverless goal (ADR-0005).

## Consequences

- One reference per commit, blob and thread can produce many references; packing and a possible consolidation into fewer index references are measured in Phase 1.
- Hosts must accept and keep custom references; this is validated against GitHub, GitLab and plain remotes in Phase 1.
- A clone without the refspec simply has no Gedo data, and plain Git keeps working.
