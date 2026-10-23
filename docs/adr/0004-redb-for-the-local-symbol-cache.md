---
id: 4
title: redb for the local symbol cache
status: accepted
date: '2026-10-23'
version: v0.1.0
subsystems: [store]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0004: redb for the local symbol cache

## Context

The daemon and the CLI need a fast local index (`.git/gedo/symbol_cache.db`): from qualified path to `AstNodeId`, from `AstNodeId` to its current anchor, and from blob to its `BlobAstMap`. The cache is derived data and can be rebuilt from `refs/gedo/*`, but it must be crash-safe, embedded and work on Windows and Linux. The blueprint names redb or sled.

## Decision

Use **`redb`**, an embedded, pure-Rust, ACID key-value store with a stable file format, inside `gedo-store` as the adapter of the core's symbol cache port. The cache stores a schema version; on a mismatch or corruption it is deleted and rebuilt from the Git object store.

## Alternatives Considered

- **sled**: has stayed in beta for years without a stable release and with an unstable on-disk format.
- **SQLite (`rusqlite`)**: mature, but a C dependency and more than a key-value index needs.
- **In-memory index rebuilt at start-up**: simple, but slow start-up on large repositories for the CLI.

## Consequences

- One more dependency in the store adapter; the core sees only the cache port.
- A rebuild path from `refs/gedo/*` is required and tested, which also covers cache loss.
