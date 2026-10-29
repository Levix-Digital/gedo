# Contract: Event Model

The interfaces this feature gives to the later layers (FEAT-065 to FEAT-069, the CLI and daemon of v0.4.0). No end-user command or JSON schema is part of this feature; the JSON schemas of ADR-0013 arrive with FEAT-088.

## gedo-crdt

```rust
pub fn event_id(event: &Event) -> EventId;            // research R2, also Event::id()
pub fn tape_id(tape: &Tape) -> TapeId;
pub fn fingerprint(email: &str) -> AuthorFingerprint; // research R4
pub fn generation(parent_generations: &[u64]) -> u64; // 1 for a root
pub fn live_clock(base_generation: Option<u64>, sequence: u32) -> Clock;
pub fn reconstructed_events(
    commit_generation: u64,
    author_email: &str,
    deltas: &[SemanticDelta],
    merges: &[IdentityMerged],
    result_hash: impl Fn(AstNodeId) -> Option<StructuralHash>,
) -> Vec<Event>;
pub fn sort_events(events: &mut [Event]);              // total order (clock, id)

impl RecordingContext {
    pub fn person(email: &str, labels: Labels) -> Self;
    pub fn agent(email: &str, agent: AgentId, labels: Labels) -> Self;
    pub fn from_texts(email: &str, agent: Option<&str>, labels: Option<&str>) -> Result<Self, LabelError>;
}
impl Labels { pub fn parse(text: &str) -> Result<Labels, LabelError>; pub fn for_recording(self) -> Result<Labels, LabelError>; }
impl EventFilter { pub fn matches(&self, event: &Event) -> bool; }
```

- Pure: no I/O, deterministic on every platform and input order.
- `LabelError` names the offending pair, its position in the text and the rule broken.

## gedo-app::events

```rust
pub struct EventLog<'a, O: ObjectStore, R: RefStore, W: WriteAheadLog, C: SymbolCache> { /* ... */ }

impl EventLog<'_, O, R, W, C> {
    pub fn open(objects, refs, wal, cache, identity: &IdentityIndex<..>) -> Result<Self, EventError>;
    pub fn record(&mut self, context: &RecordingContext, base: Option<GitOid>,
                  ops: &[(SemanticOp, Option<StructuralHash>)]) -> Result<Vec<EventId>, EventError>;
    pub fn publish(&mut self, commit: GitOid) -> Result<Option<TapeId>, EventError>;
    pub fn tapes_of(&self, commit: &GitOid) -> Result<Vec<TapeId>, EventError>;
    pub fn tape(&self, id: &TapeId) -> Result<Tape, EventError>;
    pub fn event(&self, id: &EventId) -> Result<Event, EventError>;
    pub fn events_of_commit(&self, commit: &GitOid) -> Result<CommitEvents, EventError>;
    pub fn filter(&self, filter: &EventFilter) -> Result<Vec<Event>, EventError>;
}

pub enum CommitEvents { Published(Vec<(TapeId, Vec<Event>)>), Reconstructed(Vec<Event>) }
```

- `publish` returns `None` when the log holds no event; it is idempotent and crash-safe (research R11).
- `events_of_commit` returns the published tapes when the index has entries, otherwise the reconstructed events (research R10); `NotIndexed` when the commit's identities are not indexed yet.
- `EventError` variants: `Store(StoreError)` (with context), `Label(LabelError)`, `TapeTooLarge { events, limit }`, `MissingTape { commit, tape }`, `MissingEvent { tape, event }`, `NotIndexed { commit }`.

## Storage (shared)

| Collection | Entry | Blob |
|---|---|---|
| `refs/gedo/events` | event id, 32 hex | event record, format 1 |
| `refs/gedo/tapes` | tape id, 32 hex | tape record, format 1 |
| `refs/gedo/commit-tapes` | commit hex + tape hex | empty record, format 1 |

Every release reads format 1 of each record (TECH_SPEC, Formats).
