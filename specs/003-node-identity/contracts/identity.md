# Contract: Node Identity

The interfaces this feature adds. Types: [data-model.md](../data-model.md).

## `gedo-ast` (pure, no I/O)

```rust
pub const MATCH_FORMAT: MatchFormat = MatchFormat::V1;
pub const SIMILARITY_THRESHOLD_THOUSANDTHS: u32 = 800;

/// Matches the old and new entities of a change set (research R3).
pub fn match_change(change: &ChangeSet, live_ids: &BTreeSet<AstNodeId>) -> MatchOutcome;

/// Derives the id of a new entity (research R7); `taken` holds ids already live in the commit.
pub fn derive_id(entity: &CodeEntity, unit: &str, taken: &BTreeSet<AstNodeId>) -> AstNodeId;

/// Resolves ids through identity merges (research R11).
pub struct AliasTable { /* union-find */ }
impl AliasTable {
    pub fn from_merges<'a>(merges: impl IntoIterator<Item = &'a IdentityMerged>) -> Self;
    pub fn add(&mut self, merge: &IdentityMerged);
    pub fn resolve(&self, id: AstNodeId) -> AstNodeId;
}
```

Guarantees:

- `match_change` is a pure function of its inputs: the same change set, given in any order of files and entities, gives the same `MatchOutcome` byte for byte when encoded, on every platform.
- Matching a version against itself keeps every id and produces no delta.
- Every new entity gets exactly one id; ids are unique within the outcome and never equal an id of `live_ids` unless the entity continues it.
- An old entity is either continued by exactly one new entity, carried (its file has a syntax error), or tombstoned.
- `match_change` never panics and never fails on syntax maps produced by `gedo_ast::normalize`.

## `gedo-app::index`

```rust
pub struct IdentityIndex<'a, O: ObjectStore, P: Parser, C: SymbolCache> { /* ports */ }

impl IdentityIndex<'_, O, P, C> {
    /// Indexes every commit reachable from `tips` that has no record yet (research R10).
    pub fn index_history(&mut self, tips: &[GitOid]) -> Result<IndexReport, IndexError>;
    /// The map of `path` at `commit`, ids resolved through aliases; `None` when the file is not
    /// Rust or does not exist there. The commit must be indexed.
    pub fn map_at(&self, commit: &GitOid, path: &str) -> Result<Option<BlobAstMap>, IndexError>;
    /// The deltas and identity merges of one indexed commit.
    pub fn changes_of(&self, commit: &GitOid) -> Result<CommitChanges, IndexError>;
    /// Resolves an id through every identity merge known to the index.
    pub fn resolve(&self, id: AstNodeId) -> AstNodeId;
}

pub struct IndexReport { pub commits_indexed: u64, pub commits_skipped: u64, pub files_parsed: u64, pub files_refused: Vec<(String, ParseError)> }
```

`IndexError` wraps `StoreError` (with the commit and path being read) and reports `NotIndexed { commit }` and `ParseTimeout { path, blob }`. A file the parser refuses for a reason of its own (`TooLarge`, `TooDeep`, `Internal`) is listed in `files_refused`, logged, and treated as a non-Rust file for that commit. A `Timeout` depends on the machine's speed, so it stops indexing with `ParseTimeout` before the commit is recorded (changed during implementation: recording it would make ids differ between machines); indexing again with a longer parser time limit completes it. A record of an unknown format or a corrupted record is a `StoreError::UnsupportedFormat` or `Corrupted`; the caller may clear the namespace and index again.

Guarantees:

- `index_history` is idempotent: a second call with the same tips writes nothing.
- Interrupted at any point and called again, it produces the same records as an uninterrupted run.
- The records are the same whatever the order of `tips`, the order in which branches were indexed, and the platform.
