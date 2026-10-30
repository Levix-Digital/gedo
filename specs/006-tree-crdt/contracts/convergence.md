# Contract: Tree CRDT

The interfaces this feature gives to the later layers (the merge driver FEAT-070, the shadow merge FEAT-074, the sequencer FEAT-069, virtual streams FEAT-068, the relay). No end-user command or JSON schema is part of this feature.

## gedo-crdt::tree

```rust
pub enum Part { Presence, Place, Name, Signature, Body }

impl TreeState {
    pub fn from_maps(maps: &[BlobAstMap]) -> TreeState;
    pub fn entity(&self, id: AstNodeId) -> Option<&EntityState>;
    pub fn entities(&self) -> &BTreeMap<AstNodeId, EntityState>;
    pub fn members(&self, path: &str, container: &str) -> Vec<AstNodeId>;
}

pub fn fold(base: &TreeState, events: &[Event]) -> Result<SideState, TreeError>;

pub fn converge(base: &TreeState, sides: &[Vec<Event>]) -> Result<Convergence, SideError>;
pub fn converge_with(
    base: &TreeState,
    sides: &[Vec<Event>],
    resolve: impl Fn(AstNodeId) -> AstNodeId,      // identity merges the caller knows
) -> Result<Convergence, SideError>;
```

- Pure: no I/O, deterministic on every platform, for every order of the events of a side, for duplicated events and for events shared between sides.
- Permuting `sides` permutes the side positions in the conflicts and changes nothing else.
- `converge(base, &[side])` returns the side's state with no conflict; `converge(base, &[side, side])` and `converge(base, &[side, vec![]])` too.
- A fold error is returned with the position of its side.
- Values as in [data-model.md](../data-model.md).

## gedo-app::events::converge

```rust
pub struct Side { pub tip: GitOid, pub live: Vec<Event> }

impl EventLog<'_, O, R, W> {
    pub fn converge<P, C>(
        &mut self,
        base: &GitOid,
        sides: &[Side],
        identities: &IdentityIndex<'_, O, P, C>,
    ) -> Result<ConvergeReport, EventError>;
}
```

- The base and every commit of every side must be indexed in `identities` (`IndexError::NotIndexed` otherwise).
- The events of a side are those of its commits after `base` (through `events_of_commit`), in generation order, followed by its `live` events.
- Reads only; writes no shared or local data.

## Observability

- `gedo-crdt` stays free of logging (pure).
- `converge` logs at info `"sides converged"` with the base and the number of sides, events and conflicts, and at debug `"side read"` with the commits read per side (the core has no clock, so callers time it).
