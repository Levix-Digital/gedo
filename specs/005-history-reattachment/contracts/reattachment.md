# Contract: History Re-attachment

The interfaces this feature gives to the later layers (the hooks of FEAT-090 and FEAT-091, the daemon's background fetch FEAT-073, `gedo gc` FEAT-087, the hosting integration). No end-user command or JSON schema is part of this feature.

## gedo-crdt::reattach

```rust
pub fn semantic_signature(
    deltas: &[SemanticDelta],
    merges: &[IdentityMerged],
    result_hash: impl Fn(AstNodeId) -> Option<StructuralHash>,
) -> SemanticSignature;

pub struct CandidateTape { pub tape: TapeId, pub events: Vec<Event>, pub paired: bool }

pub fn match_commit(
    changes: &[SemanticOp],                       // the commit's deltas and merges, in order
    result_hash: impl Fn(AstNodeId) -> Option<StructuralHash>,
    candidates: &[CandidateTape],
    same_entity: impl Fn(AstNodeId, AstNodeId) -> bool, // identity merges known to the index
) -> Reattachment;
```

- Pure: no I/O, deterministic on every platform and for every order of `candidates`.
- `Reattachment` as in [data-model.md](../data-model.md).

## gedo-app::events (EventLog)

```rust
impl<O: ObjectStore, R: RefStore, W: WriteAheadLog> EventLog<'_, O, R, W> {
    pub fn reattach_rewritten<P, C>(&mut self, pairs: &[(GitOid, GitOid)], identities: &IdentityIndex<'_, O, P, C>)
        -> Result<ReattachReport, EventError>;
    pub fn reattach_new<P, C>(&mut self, commits: &[GitOid], tips: &[GitOid], identities: &IdentityIndex<'_, O, P, C>)
        -> Result<ReattachReport, EventError>;
    pub fn detached(&mut self, tips: &[GitOid], now: u64, grace_days: u32)
        -> Result<Vec<DetachedTape>, EventError>;
    pub fn fill_cache(&self) -> Result<CacheFill, EventError>;          // needs with_cache
    pub fn events_of_commit<P, C>(&mut self, commit: &GitOid, identities: &IdentityIndex<'_, O, P, C>)
        -> Result<CommitEvents, EventError>;                            // remainder added
}
pub const DEFAULT_GRACE_DAYS: u32 = 90;
```

- `pairs` as Git's `post-rewrite` hook writes them (old, new); a pair whose new commit already has an index entry is skipped.
- `commits` and the new commit of every pair must be indexed in `identities`; otherwise `EventError::Index(NotIndexed)`. An old commit that is not indexed only skips the fast path.
- `tips`: the commits of the refs outside `refs/gedo/*` that the caller considers part of the history (local branches, remote-tracking branches, tags, `HEAD`).
- Only commit index entries are written; events, tapes and existing entries never change. Every write is content-addressed and idempotent; a call interrupted at any point and run again gives the same data.
- `detached` keeps the retention record in the cache given with `with_cache`; without a cache every detached tape is dated `now`, so none is eligible (the safe side).
- Errors: `EventError::Missing` for a tape or event named by the index but absent (never treated as unmatched); store and index errors with context.

## Logs (tracing)

| Level | Event | Fields |
|---|---|---|
| info | `tapes re-attached` | `commit`, `tapes`, `events`, `fast_path` |
| info | `re-attachment done` | `commits`, `attached`, `skipped` |
| debug | `changes not matched` | `commit`, `changes` |
| info | `detached tapes` | `detached`, `eligible`, `grace_days` |
| info | `event cache filled` | `listed`, `cached`, `already_cached` |
| warn | `could not write the event cache` / `retention record refused` | `error` |
