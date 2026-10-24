---
title: Node Identity
summary: Keeps a stable identity for each code entity across edits, renames and moves.
code: [crates/core/gedo-ast]
---

# Node Identity

Keeps a stable identity for each code entity across edits, renames and moves.

## Responsibilities

- Normalizes the neutral syntax tree: significant syntax is separated from trivia (whitespace, line breaks, formatting, comments).
- Computes the structural hash of each entity (BLAKE3 over the normalized tokens, line endings normalized).
- Matches the entities of a new version of a file against the previous version and keeps their `AstNodeId`s.
- Emits semantic deltas for what changed.
- Derives identities deterministically, so the same history gives the same ids on every machine (ADR-0007), and reconciles two ids found for one entity at merge time.
- Does not parse source text; it receives trees from the parser port (`gedo-parse`).

## Structure

Pure functions over core types: normalization, hashing, matching and delta emission. No I/O.

## Behavior

**3-tier matching**, for each entity of the new version:

1. **Exact match, O(1)**: same `(qualified_path, structural_hash)`. The `AstNodeId` is kept even when the `byte_range` moved (lines added above, a formatter run).
2. **Signature and scope heuristics**: the hash changed; compare the lexical container (`parent_node_id`) and the structural similarity of the signature (parameter types, return type, visibility). At a similarity of 80% or more, the `AstNodeId` is kept and `SignatureMutated` or `NodeRenamed` is emitted.
3. **Spawn or delete fallback**: no plausible match; a new `AstNodeId` is created (`NodeCreated`) and the missing entity becomes `NodeTombstoned`.

A node created in tier 3 gets a UUID v8 derived from `BLAKE3("gedo-node-v1" ‖ kind ‖ qualified_path ‖ structural_hash)`; a matched node keeps its earlier id. The matching algorithm version is stored with each map, and published ids are never recomputed. When a merge finds two ids for one entity, the lower one is kept and the other becomes an alias (`IdentityMerged` event).

The result is a new `BlobAstMap` plus the list of deltas. The similarity threshold is a named constant tuned on the reference corpus (identity accuracy target in the vision: 95%).

## Interfaces

- Called by `gedo-app` when a file changes (from the daemon or the CLI).
- Calls nothing outside the core; uses `gedo-core` types.

## Decisions

- [ADR-0001](../adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md): matching is pure core logic, tested with property-based tests.
- [ADR-0002](../adr/0002-rust-workspace-with-gitoxide-tree-sitter-blake3-and-postcard.md): BLAKE3 structural hash.
- [ADR-0007](../adr/0007-deterministic-node-identity-derived-from-history.md): deterministic identities and alias reconciliation.
