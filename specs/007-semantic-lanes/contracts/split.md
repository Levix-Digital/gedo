# Contract: Semantic Lanes and Split Publish

The interfaces this feature gives to `gedo publish --split-lanes` (FEAT-083) and to stack proposals by lane (FEAT-113). No end-user command or JSON schema is part of this feature.

## gedo-crdt::lanes and gedo-crdt::split

```rust
pub enum Lane { Refactor, Feature }

pub fn classify(deltas: &[SemanticDelta], files: &[FileVersions<'_>]) -> LanePlan;
pub fn event_lane(plan: &LanePlan, event: &Event, resolve: impl Fn(AstNodeId) -> AstNodeId) -> Lane;

pub fn build(files: &[FileVersions<'_>], plan: &LanePlan) -> Result<Vec<RefactorFile>, BuildError>;
pub fn refactor_message(plan: &LanePlan) -> String;
```

- Pure: no I/O, deterministic on every platform.
- `classify` gives every delta exactly one lane; `build` returns, for every changed file, its refactor text or its absence.

## gedo-app::events

```rust
impl EventLog<'_, O, R, W> {
    pub fn publish_split<P: Parser + Sync, C: SymbolCache>(
        &mut self,
        request: &SplitRequest,
        identities: &mut IdentityIndex<'_, O, P, C>,
        parser: &P,
    ) -> Result<SplitOutcome, EventError>;
}
```

- The base must be indexed; the commits it writes are indexed by the call.
- Writes commits and `refs/gedo/*` only; never moves a branch. The last commit of the outcome always has `request.tree` as its tree.
- `Split`: refactor commit (parent `base`), feature commit (parent the refactor commit), one tape each for the lanes that have events, the log truncated once.
- `Single`: one commit (parent `base`, `request.message`) with every event, as `publish`; `reason` says why it was not split.
- Errors carry context; the log is kept when anything fails before the index entries are written.
