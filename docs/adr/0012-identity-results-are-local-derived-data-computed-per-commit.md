---
id: 12
title: Identity results are local derived data computed per commit
status: accepted
date: '2026-10-25'
version: v0.2.0
subsystems: [identity, store]
supersedes: []
superseded_by: []
amends: [3, 7]
amended_by: []
---

# ADR-0012: Identity results are local derived data computed per commit

## Context

ADR-0003 and the data table of the technical specification store each blob's `BlobAstMap` in `refs/gedo/ast-map/<blob-sha>`, immutable per blob, and keep the alias table of ADR-0007 in `refs/gedo/`. Building the identity phase (feature `003-node-identity`, research R2) showed that an entity's identity depends on its history, not only on the blob that contains it: the same blob at the same path can be reached by two histories (a function renamed on one branch, written fresh on another) and carry different ids. A record keyed by blob would hold different content on two machines that indexed the branches in different orders, and the union of collections (ADR-0011) would conflict. Measured on ripgrep 14.1.1 (2,025 commits), indexing the whole history took about 11 s in a Linux development container, and one new commit about 3 ms at the 95th percentile; an identity record of one changed file is about 5 KB of ids and hashes that do not compress.

## Decision

- **Per commit.** Identities are computed per commit: a commit's ancestry is fixed by its hash, so the identities of its entities are a pure function of the commit, whatever the order of indexing. `gedo_app::IdentityIndex` walks every ancestor to every root, parses only the Rust files that differ from the first parent, and reconciles the other parents at merges.
- **Local derived data.** Results live in the local symbol cache (ADR-0004), namespace `identity`: `commit/<oid>` (changed files with the digest of their map, deltas and identity merges), `map/<digest>` (each `BlobAstMap` once), `checkpoint/<oid>` every 64 first-parent commits, and `merges` (every `IdentityMerged`, from which the alias table is rebuilt). Every value is a `RecordEnvelope` of format 1; a missing, damaged or foreign record is rebuilt by indexing again.
- **Nothing in `refs/gedo/` for identity.** The `ast-map` namespace of ADR-0003 and the shared alias table of ADR-0007 are not created. Identity data that must travel, ids of live edits made before a commit exists and `IdentityMerged` events, travels with the events of v0.3.0, as ADR-0007 already says of events recorded live.
- **Versioning.** Each map records `MatchFormat::V1` (with `HashFormat::V1`); a later format is a new version of the rules and of the records, and the cache refuses formats it does not know.

## Alternatives Considered

- **Per-blob records with first-writer-wins**: not deterministic across machines; the stored map depends on who indexed first.
- **Per-commit trees of maps in `refs/gedo/`**, deduplicated by Git: deterministic, but needs tree-valued collection entries, a new store layout and grows with history (over 100 MB for 10,000 commits) to save seconds of recomputation.
- **Ids derived from blobs only**: deterministic and shareable, but loses renames and moves, which is what identity exists for.

## Consequences

- Every machine computes the same ids from the same history without network traffic; determinism is checked by indexing in several orders and by a snapshot of the repository's own history on Linux and Windows.
- A fresh clone pays the first indexing (about 11 s for ripgrep's history); `gedo init` in v0.4.0 must stay within its budget, and sharing results becomes a backlog item only if a benchmark shows it does not.
- ADR-0003's namespace list loses `ast-map`; the technical specification's data table moves `BlobAstMap` and the alias table to the cache.
- The cache's `identity` namespace must be cleared when the matching rules change in a way records do not show; records carry their formats so this is detected, not guessed.
