# Contract: Virtual Streams

The interfaces this feature gives to the `gedo stream` commands (v0.4.0), agent streams (FEAT-130) and the radar (FEAT-074). No end-user command or JSON schema is part of this feature.

## gedo-core

```rust
pub struct StreamName(String);
impl StreamName { pub fn new(name: &str) -> Result<Self, ParseIdError>; pub fn as_str(&self) -> &str; pub fn key(&self) -> String; }

pub struct Stream { pub name: StreamName, pub agent: Option<AgentId>, pub labels: Labels,
                    pub base: Option<GitOid>, pub work: Option<GitOid>, pub publishing: Option<GitOid> }

pub trait StreamStore {
    type Log: WriteAheadLog;
    fn names(&self) -> Result<Vec<StreamName>, StoreError>;
    fn read(&self, name: &StreamName) -> Result<Option<Vec<u8>>, StoreError>;
    fn create(&self, name: &StreamName, record: &[u8]) -> Result<bool, StoreError>;
    fn replace(&self, name: &StreamName, record: &[u8]) -> Result<(), StoreError>;
    fn remove(&self, name: &StreamName, log: Self::Log) -> Result<(), StoreError>;
    fn open_log(&self, name: &StreamName) -> Result<Self::Log, StoreError>;
    fn pin(&self, name: &StreamName, commit: Option<GitOid>) -> Result<(), StoreError>;
    fn read_state(&self) -> Result<Option<Vec<u8>>, StoreError>;
    fn write_state(&self, state: &[u8]) -> Result<(), StoreError>;
    fn pin_outside(&self, commit: Option<GitOid>) -> Result<(), StoreError>;
}
```

## gedo-crdt::streams

```rust
pub struct StreamFilter { pub labels: Labels, pub agent: Option<AgentId> }
impl StreamFilter { pub fn matches(&self, stream: &Stream) -> bool; pub fn from_texts(labels: Option<&str>, agent: Option<&str>) -> Result<Self, LabelError>; }
pub fn stream_context(stream: &Stream, email: &str) -> Result<RecordingContext, LabelError>;
pub fn recording_context(active: Option<&Stream>, email: &str, agent: Option<&str>, labels: Option<&str>) -> Result<RecordingContext, LabelError>;
```

## gedo-app::streams

```rust
impl<'a, O: ObjectStore, R: RefStore, S: StreamStore> Streams<'a, O, R, S> {
    pub fn new(objects: &'a O, refs: &'a R, store: &'a S) -> Self;
    pub fn with_cache(self, cache: &'a dyn SymbolCache) -> Self;
    pub fn create(&self, name: &StreamName, agent: Option<AgentId>, labels: Labels, base: Option<GitOid>) -> Result<Stream, StreamError>;
    pub fn get(&self, name: &StreamName) -> Result<Stream, StreamError>;
    pub fn list(&self, filter: &StreamFilter) -> Result<Vec<Stream>, StreamError>;
    pub fn active(&self) -> Result<Option<Stream>, StreamError>;
    pub fn recording_context(&self, email: &str, agent: Option<&str>, labels: Option<&str>) -> Result<RecordingContext, StreamError>;
    pub fn record(&self, name: &StreamName, email: &str, ops: &[(SemanticOp, Option<StructuralHash>)]) -> Result<Vec<EventId>, StreamError>;
    pub fn pending(&self, name: &StreamName) -> Result<Vec<Event>, StreamError>;
    pub fn switch(&self, target: Option<&StreamName>, current: GitOid) -> Result<Option<GitOid>, StreamError>;
    pub fn publish(&self, name: &StreamName, tree: Option<GitOid>, message: &str) -> Result<StreamPublish, StreamError>;
    pub fn side(&self, name: &StreamName) -> Result<Side, StreamError>;
    pub fn discard(&self, name: &StreamName, force: bool) -> Result<Discarded, StreamError>;
}
```

- `switch` returns the target's saved work, its base's tree or the empty tree; out of streams, the saved outside work or `None` (keep `HEAD`'s tree).
- `publish` moves no branch; the caller moves one to the returned commit.
- No call writes the working tree, the index, `HEAD` or a branch.

## gedo-store

```rust
impl FileStreamStore { pub fn open(repository: &GitRepository) -> Result<Self, StoreError>; }
```
