---
title: Node Identity
summary: Keeps a stable identity for each code entity across edits, renames and moves.
code: [crates/core/gedo-ast, crates/core/gedo-app/src/index]
---

# Node Identity

Keeps a stable identity for each code entity ac## Responsibilities

- Normalizes the neutral syntax tree: significant syntax is separated from trivia (whitespace, line breaks, formatting, comments).
- Computes the structural hash of each entity (BLAKE3 over the normalized tokens, line endings normalized).
- Matches the entities of a change set (the files changed by a commit) against their previous versions and keeps their `AstNodeId`s, also across files.
- Emits semantic deltas for what changed.
- Derives identities deterministically, so the same history gives the same ids on every machine and in every indexing order (ADR-0007), and reconciles two ids found for one entity at a merge.
- Indexes history made without Gedo (`gedo-app`'s `IdentityIndex`), keeping the results as local derived data (ADR-0012).
- Does not parse source text; it receives a `ParsedFile` from the parser port (`gedo-parse`).

## Structure

Pure functions over core types, no I/O, in `gedo-ast`:

| Module | Role |
|---|---|
| `validate.rs` | Checks the structural rules of a `ParsedFile`; a violation is a parser bug (`ParseError::Internal`) |
| `normalize.rs` | Keeps the significant tokens, with line endings normalized to LF |
| `hash.rs` | Signature, body and structural hash of each entity |
| `matching.rs` | `match_change(&ChangeSet, &known_ids) -> MatchOutcome`: the tiers below, one `BlobAstMap` per new file and the deltas |
| `identity.rs` | `derive_id`: the id of a new entity |
| `delta.rs` | The deltas of a continued entity |
| `alias.rs` | `AliasTable`: resolves every id to the lowest id it was merged with |

The use case lives in `gedo-app` (`src/index/`): `walk.rs` (the commits to index), `diff.rs` (the files that differ between two trees), `records.rs` (the stored records) and `mod.rs` (`IdentityIndex`).

## Behavior

**Normalization** keeps the significant tokens: comments, documentation comments and optional punctuation (a token class the parser gives to what a formatter adds or removes) are dropped, and so is a separator directly before a closing delimiter. CRLF and lone CR become LF; invalid UTF-8 is decoded lossily.

**Structural hash** (`HashFormat::V1`): BLAKE3 in derive-key mode with the context `gedo structural hash v1`. Each token is its class code, its length (`u32`, little endian) and its text. The signature hash covers the tokens before the body; the body hash covers the body, where each nested entity is replaced by its own structural hash, so a change reaches only the entity and its containers. Module declarations without a body and imports (each item of a `use` list is an import of its own) are hashed in sorted order, so their order carries no meaning. The structural hash combines the signature and body hashes. The format is stored with each syntax map, and a reader refuses a format it does not know.

**Matching** (`MatchFormat::V1`) compares the old entities of every file of a change set with the new ones, so an entity moved to another file keeps its id. Each tier only considers entities left unmatched by the previous ones, of the same kind:

1. **Exact**: same unit, qualified path, occurrence and structural hash. The id is kept even when the byte range moved.
2. Continuity and heuristics:
   - **(a) same place**: same unit, qualified path and occurrence, different content (an edited body or signature, or a syntax error inside the entity);
   - **(b) same content elsewhere**: the same structural hash, when it is unique on both sides (a move, a file rename);
   - **(c) similarity**: an integer score in thousandths, `450 × signature + 400 × body + 150 × scope`, where signature and body are Dice coefficients of the token multisets (the entity's own name excluded) and scope is 1 for the same container, 0.5 for the same unit. Pairs at 800 or more are taken best first, ties by old id, then new position. At most 50,000 pairs per kind are scored; above that, only pairs sharing a name, a body hash or a container;
   - **(d) containers by children**: an unmatched container (an `impl` whose type was renamed, a module) continues the old container that more than half of its children came from.
3. **New or tombstoned**: an unmatched new entity gets a new id; an unmatched old entity is tombstoned, except in a file with a syntax error, where it is **carried** as a detached node (no byte range) and matched again against the next version, so a half-typed save never loses identities.

**Derivation**: a new id is a UUID v8 made from BLAKE3 in derive-key mode (`gedo node id v1`) over the kind, unit, qualified path, occurrence, structural hash and a collision counter that grows only when the id is already taken in the same commit. An entity created again with the same place and content gets its old id back.

**Deltas**: a continued entity is `Moved` when its unit, file or container id changed (a renamed container does not move its children), `Renamed` when its name changed, `SignatureMutated` and `BodyModified` from the token sequences; new entities are `Created`, unmatched old ones `Tombstoned`. A list is ordered by variant, then id.

**History indexing**: `IdentityIndex::index_history(tips)` walks every ancestor of the tips to every root, skipping indexed commits, in topological order with ties by commit id. For each commit it diffs the tree against the first parent, parses the changed Rust blobs in parallel (in batches of 128 commits), and matches them against the first parent's results. At a merge, the other parents are matched over the files that differ from them; an entity that continues two different ids keeps the lower one and records an `IdentityMerged`. Each commit is written in one cache transaction (`commit/<oid>`, `map/<digest>`, a checkpoint every 64 first-parent commits, `merges`), so an interrupted run resumes where it stopped and a second run writes nothing. A file the parser refuses for its size or depth is left out of that commit; a parse timeout, which depends on the machine, stops indexing with `ParseTimeout` and records nothing for the commit. `map_at`, `files_at` and `changes_of` read the results of an indexed commit, with ids resolved through the alias table.

**Accuracy** is measured on a reference corpus of scripted edits of real Rust files (`gedo-app/tests/identity_corpus.rs`): 674 cases in 19 categories (body edits, signature changes, renames, moves, formatter runs, copies, splits, merges, replacements, syntax errors), 99.98% of identities preserved and no false continuation; the test fails below 95% overall, 90% per category or above 1% false continuations.

## Interfaces

- `normalize(&ParsedFile, &[u8]) -> Result<SyntaxMap, ParseError>` and `HASH_FORMAT`.
- `match_change`, `derive_id`, `AliasTable` and `EntityTokens` for the use cases of `gedo-app`.
- `IdentityIndex::open(&ObjectStore, &Parser, &mut SymbolCache)`, then `index_history`, `map_at`, `files_at`, `changes_of` and `resolve`; errors are `IndexError` (a store error with its context, a parse timeout, or a commit not indexed).
- Calls nothing outside the core; uses `gedo-core` types and ports.

 core; uses `gedo-core` types.

## Decisions

- [ADR-0001](../adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md): matching is pure core logic, tested with property-based tests.
- [ADR-0002](../adr/0002-rust-workspace-with-gitoxide-tree-sitter-blake3-and-postcard.md): BLAKE3 structural hash.
- [ADR-0007](../adr/0007-deterministic-node-identity-derived-from-history.md): deterministic identities and alias reconciliation.
- [ADR-0012](../adr/0012-identity-results-are-local-derived-data-computed-per-commit.md): identity results are local derived data computed per commit.
- `specs/002-syntax-model/research.md` (R4 to R6): normalization and hash format V1.
- `specs/003-node-identity/research.md`: matching, derivation, deltas, indexing and the corpus.
