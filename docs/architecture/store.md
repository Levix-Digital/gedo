---
title: Git Object Store Adapter
summary: Persists Gedo data in the Git object store, the local WAL and the redb cache.
code: [crates/adapters/gedo-store]
---

# Git Object Store Adapter

Persists Gedo data in the Git object store, the local WAL and the redb cache.

## Responsibilities

- Implements the object store, reference store, write-ahead log and symbol cache ports.
- Reads and writes blobs and trees in `.git/objects` and references in `.git/refs` with gitoxide, without spawning `git` and without touching the working tree.
- Manages the life cycle and atomicity of references under `refs/gedo/*` with compare-and-swap updates, so `gedod` and the CLI can write concurrently.
- Provides in-memory implementations of the same ports for tests.

## Structure

```
.git/
├── objects/                  # native blobs and trees (code and Gedo structures)
├── refs/
│   ├── heads/main            # canonical commit, consumed by GitHub/GitLab
│   └── gedo/                 # custom namespace, carried by a refspec
│       ├── tapes/                 # content-addressed events, tapes and the commit index
│       ├── ast-map/<blob-sha>     # binary map from syntax nodes to AstNodeIds
│       └── threads/<thread-uuid>  # intents and negotiations about a symbol
└── gedo/                     # strictly local runtime, never pushed
    ├── wal.log               # micro-events not yet consolidated
    └── symbol_cache.db       # redb index
```

## Behavior

- All records are Postcard-encoded with a format version prefix.
- Events are stored by id and indexed by commit; after a rewrite the index is rebuilt by re-attachment (ADR-0008). Unreachable events are kept for 90 days before `gedo gc`.
- Textual three-way merges for the merge driver's fallback are computed with gitoxide's merge, without a subprocess.
- The symbol cache is derived data: on a schema mismatch or corruption it is rebuilt from `refs/gedo/*`.
- `gedo init` writes the fetch and push refspecs for `refs/gedo/*` to the remote configuration, and registers the merge driver in `.git/config` and `.git/info/attributes` (local, never committed).

## Interfaces

- Implements ports defined in `gedo-core`; wired by the composition roots in `gedo-cli` and `gedod`.

## Decisions

- [ADR-0002](../adr/0002-rust-workspace-with-gitoxide-tree-sitter-blake3-and-postcard.md): gitoxide, no Git subprocess in the store ([ADR-0006](../adr/0006-git-command-pass-through-with-hook-based-augmentation.md) allows it only for the CLI pass-through).
- [ADR-0003](../adr/0003-git-object-store-as-the-canonical-datastore.md): the Git object store is the canonical datastore.
- [ADR-0004](../adr/0004-redb-for-the-local-symbol-cache.md): redb for the local cache.
- [ADR-0005](../adr/0005-serverless-operation-with-git-remotes-as-the-only-transport.md): refspecs carry the data through Git remotes.
- [ADR-0008](../adr/0008-gedo-data-robust-to-history-rewrites.md): data robust to history rewrites.
- [ADR-0009](../adr/0009-semantic-merge-driver-and-shadow-merge.md): merge driver registration and fallback.
