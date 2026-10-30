# Data Model: Tree CRDT

No new shared record and no change of a stored format: the convergence is derived data computed on demand (research R2, R9). Every value below lives in memory.

## In memory (gedo-crdt::tree)

### Part

The parts of an entity compared on their own (research R3), in this order: `Presence`, `Place`, `Name`, `Signature`, `Body`.

### Place

- `unit: String`, `path: String`, `container: String` (the qualified path without the entity's own name).

### EntityState

- `kind: EntityKind`
- `present: bool`
- `place: Place`
- `name: String`
- `signature: Option<StructuralHash>`, `body: Option<StructuralHash>` (`None` when an entity created on a side has not had them recorded by an event)
- `structural: Option<StructuralHash>` (`None` when unknown: parts converged from different sides, research R6)
- `qualified_path()`: the container and the name joined by the language's separator.

### TreeState

- `entities: BTreeMap<AstNodeId, EntityState>`
- `members(container_path, path) -> Vec<AstNodeId>`: the present entities of a file whose container is the given path, sorted by id (FR-013).
- Built from identity results by `TreeState::from_maps(&[BlobAstMap])` (base) and by the fold.

### SideState (fold of one side, research R4)

- `touched: BTreeMap<AstNodeId, EntityState>`: the entities the side's events touched, as it leaves them; the others are as in the base.
- `last_event: BTreeMap<AstNodeId, EventId>`: the side's last event about each entity.
- `set_by: BTreeMap<(AstNodeId, Part), EventId>`: the last event of the side that set each part.
- `created: BTreeSet<AstNodeId>`, `neutral: BTreeSet<AstNodeId>`.

### Conflict

- `entity: AstNodeId`, `kind: ConflictKind`: `Part(Part)` (sides set one part to distinct values; a removal against a change is on `Presence`), `AddedOnBothSides` (on the lowest id, the others in `others`) or `RemovedContainer` (the members in `others`)
- `base: Option<PartValue>` (`None` for an entity the base does not have)
- `values: Vec<ConflictValue>`, sorted by value: `value: PartValue`, `sides: Vec<usize>` (input positions, sorted), `events: Vec<EventId>` (one per side, in the order of `sides`)
- `others: Vec<AstNodeId>`: for a removed container, the members added or moved into it; for an added-on-both-sides conflict, the other entities at the same qualified path.

### PartValue

- `Presence(bool)`, `Place(Place)`, `Name(String)`, `Signature(Option<StructuralHash>)`, `Body(Option<StructuralHash>)`, and `Entity(AstNodeId)` for the values of an added-on-both-sides conflict.

### Convergence

- `state: TreeState`: the converged entities (conflicting parts at their base value; entities added on both sides with different content absent).
- `links: Vec<(AstNodeId, AstNodeId)>`: identities linked by research R7, `(kept, merged)` with the lower id kept, sorted.
- `conflicts: Vec<Conflict>`, sorted by entity then part.
- `is_clean()`: no conflict.

### TreeError

- `UnknownEntity { event: EventId, entity: AstNodeId }`: an event about an entity neither in the base nor created earlier on its side. A rename takes its name from the qualified path it records, so it cannot be inconsistent.
- `SideError { side, error }`: a `TreeError` with the position of its side.

## In memory (gedo-app::events::converge)

### Side

- `tip: GitOid`, `live: Vec<Event>` (events not published yet, given by the caller, may be empty).

### ConvergeReport

- `convergence: Convergence`
- `commits: Vec<Vec<GitOid>>` (the commits of each side after the base, in generation order), `events: Vec<usize>` (events folded per side)

### Errors

- `EventError::Index(IndexError::NotIndexed { commit })` for a base or side commit not indexed; `EventError::Tree { side, source: TreeError }` for a fold error, naming the side; existing `Missing` and `Store` variants for unreadable data.

## Shared and local data

Unchanged. Reads `events`, `tapes` and `commit-tapes` (ADR-0014) through `events_of_commit`, and the identity results (ADR-0012) through `IdentityIndex`. Writes nothing.
