---
title: Git Object Store Adapter
summary: Persists Gedo data as commit chains in the Git object store, with a local WAL and redb cache.
code: [crates/adapters/gedo-store]
---

# Git Object Store Adapter

Persists Gedo data in the Git object store, the local WAL and the redb cache.

## Responsibilities

- Implements the object store, reference store, write-ahead log and symbol cache ports of `gedo-core`.
- Reads and writes blobs, trees and commits in `.git/objects` and references under `refs/gedo/*` with gitoxide, without spawning `git`, without touching the working tree, the index, `HEAD` or any branch.
- Moves references only by compare-and-swap, so `gedod`, the CLI and several machines can write the same collection.
- Keeps the strictly local runtime in `.git/gedo/`: the write-ahead log and the symbol cache, neither ever pushed.
- Configures remotes so a plain `git fetch` brings Gedo data, and gives the pre-push hook the arguments that push it.

## Structure

```
.git/
├── objects/                      # blobs, trees and chain commits
├── refs/
│   ├── heads/main                # code; never written by Gedo
│   ├── gedo/<collection>         # head of one commit chain per collection (pushed)
│   └── gedo-remote/<remote>/     # each remote's chains as last fetched (never pushed)
└── gedo/                         # strictly local runtime
    ├── wal.log                   # micro-events not yet consolidated
    ├── wal.lock                  # held by the single writer
    └── symbol_cache.db           # redb index, derived data
```

The repository must use SHA-1 object ids; a SHA-256 repository is refused with `UnsupportedObjectFormat` before anything is written.

## Behavior

### Commit-chain collections

A collection (`tapes`, `ast-map`, `threads`, ...) is one reference, `refs/gedo/<collection>`, pointing to the head of a commit chain ([ADR-0011](../adr/0011-gedo-references-as-commit-chains-with-a-separate-fetch-names.md)). Each commit's tree holds every entry of the collection in a two-level fan-out by entry id, `ab/cdef...`, as Git notes do; entry ids are lowercase hex. A write rewrites only the fan-out directories it touches, commits with the previous head as parent and the repository's configured identity (refused with `MissingIdentity` when there is none), then moves the reference by compare-and-swap. A lost race reloads the head and retries, up to 8 attempts, then returns `Contended`.

Two chains that diverged are joined by a merge commit whose tree is the union of both; an entry id present on both sides with different content is reported as `Corrupted`, since entries are content-addressed.

All records are Postcard-encoded inside an envelope that starts with a `u16` format version; a reader that does not know the version returns `UnsupportedFormat` with the versions it supports. A blob or record over 64 MiB is refused with `TooLarge`.

### Push and fetch

- The remote setup adds `+refs/gedo/*:refs/gedo-remote/<remote>/*` to the remote's fetch refspecs, once, and changes nothing else in `.git/config`. A plain `git fetch` then mirrors each remote's chains into `refs/gedo-remote/<remote>/`, and `Collection::merge_remote` fast-forwards or merges them into the local chains. A fetch never overwrites a local chain that was not pushed yet.
- No push refspec is ever configured, so the branches a plain `git push` sends are unchanged. The pre-push hook runs the user's `git` with `push <remote> refs/gedo/*:refs/gedo/*`, fast-forward only and never forced, with `GEDO_INNER_PUSH=1` so the hook does not recurse. A rejected push is resolved by fetch, merge and push again.
- The scale study (`cargo xtask scale-study`) measures this layout against one reference per entry; the results are in ADR-0011.

### Write-ahead log

`.git/gedo/wal.log` starts with the magic `GEDOWAL\0` and a `u16` file format. Each frame is `length u32 | crc32 u32 | record format u16 | payload`, the checksum covering the format and the payload. Appends are buffered and synced by a group-commit thread at least every 100 ms, so a record is durable at most 100 ms after `append` returns. One process writes at a time: the writer holds an exclusive lock on `wal.lock`, and a second writer waits up to 2 s, then gets `WalBusy`. On open, a torn last frame (a crash mid-write) is truncated and reported in the open report; a bad frame followed by valid data is corruption, reported with its offset. A failed write is rolled back so the file never keeps half a frame. Consolidated records are dropped by rewriting the file through a temporary file and a rename.

### Symbol cache

`.git/gedo/symbol_cache.db` is a redb database with a `meta` table holding the schema version and one table per namespace. It is derived data: when it is missing, unreadable or of another schema version it is rebuilt from the repository on open, into `symbol_cache.db.tmp` that replaces it only once complete, so an interrupted rebuild never opens as valid. The open status says whether the cache was ready or rebuilt, and why.

### Platform behavior

On Windows, an operation on a file another program holds open (an antivirus, an indexer) is retried for up to 2 s before the error is returned. Reference updates wait up to 2 s for another writer's reference lock, then return `Contended` without writing anything.

### Later features

- Events are stored by id and indexed by commit; after a rewrite the index is rebuilt by re-attachment (ADR-0008). Unreachable events are kept for 90 days before `gedo gc`.
- Textual three-way merges for the merge driver's fallback are computed with gitoxide's merge, without a subprocess; `gedo init` registers the merge driver in `.git/config` and `.git/info/attributes` (local, never committed).

## Interfaces

- Implements ports defined in `gedo-core`; wired by the composition roots in `gedo-cli` and `gedod`.

## Decisions

- [ADR-0002](../adr/0002-rust-workspace-with-gitoxide-tree-sitter-blake3-and-postcard.md): gitoxide, no Git subprocess in the store ([ADR-0006](../adr/0006-git-command-pass-through-with-hook-based-augmentation.md) allows it for the CLI pass-through, ADR-0011 for the push from the pre-push hook).
- [ADR-0003](../adr/0003-git-object-store-as-the-canonical-datastore.md): the Git object store is the canonical datastore.
- [ADR-0004](../adr/0004-redb-for-the-local-symbol-cache.md): redb for the local cache.
- [ADR-0005](../adr/0005-serverless-operation-with-git-remotes-as-the-only-transport.md): refspecs carry the data through Git remotes.
- [ADR-0011](../adr/0011-gedo-references-as-commit-chains-with-a-separate-fetch-names.md): commit chains, the `refs/gedo-remote/` fetch namespace, push without a push refspec.
- [ADR-0008](../adr/0008-gedo-data-robust-to-history-rewrites.md): data robust to history rewrites.
- [ADR-0009](../adr/0009-semantic-merge-driver-and-shadow-merge.md): merge driver registration and fallback.
