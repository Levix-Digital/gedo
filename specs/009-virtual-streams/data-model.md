# Data Model: Virtual Streams

New stored formats are local to the machine and never pushed: the stream record, the working tree state and the pins. The event format does not change (ADR-0014).

## gedo-core::stream

| Type | Fields | Rules |
|---|---|---|
| `StreamName` | `String` | 1 to 64 bytes; `/`-separated segments of `[a-z0-9._-]`, each non-empty, starting with a letter or digit, not `.` or `..` (research R3); `key()` is the hexadecimal form of its bytes |
| `Stream` | `name: StreamName`, `agent: Option<AgentId>`, `labels: Labels`, `base: Option<GitOid>`, `work: Option<GitOid>` (saved work tree; `None`: the base's tree), `publishing: Option<GitOid>` (commit of a publish in progress) | Name, agent and labels never change after creation; labels follow FEAT-063 and have no `gedo.` key |
| `WorktreeState` | `active: Option<StreamName>`, `outside: Option<GitOid>` (saved outside work tree) | One per working tree |
| `StreamRecordError` | `Format`, `Decode`, `WrongName { expected, found }`, `Label(LabelError)` | A refused record names its stream |

Stored forms: `encode_stream(&Stream) -> Vec<u8>` and `decode_stream(name, &[u8]) -> Result<Stream, StreamRecordError>` (format byte `1`, then postcard); the same for `WorktreeState`.

## gedo-core::ports::stream

`StreamStore` (research R2): `names`, `read`, `create` (false when the name exists), `replace`, `remove` (record, log and pin), `open_log` (`type Log: WriteAheadLog`, one writer, waits then `WalBusy`), `pin(name, Option<GitOid>)`, `read_state`, `write_state(state, outside_pin)`.

## gedo-crdt::streams

| Type | Fields | Rules |
|---|---|---|
| `StreamFilter` | `labels: Labels`, `agent: Option<AgentId>` | Matches a stream with every label of the filter (same value) and, when given, that agent; `from_texts(labels, agent)` reads the FEAT-063 text forms |

Functions: `stream_context(stream, email) -> RecordingContext`, `recording_context(active: Option<&Stream>, email, agent_text, labels_text) -> Result<RecordingContext, LabelError>`.

## gedo-app::streams

| Type | Fields | Rules |
|---|---|---|
| `Streams<'a, O, R, S>` | `objects`, `refs`, `store`, `cache: Option<&dyn SymbolCache>` | The use cases (contracts/streams.md) |
| `StreamPublish` | `NothingToPublish` or `Published { commit, tape: Option<TapeId>, events: usize }` | `tape` is `None` for a commit without events |
| `Discarded` | `events: usize`, `work: bool` | What was dropped |
| `StreamError` | `Name`, `Label`, `Exists`, `NotFound`, `NoBase`, `HasWork { events, work }`, `Record { name, source }`, `Store { context, source }`, `Event(EventError)` | Every error has context |

## gedo-store::streams

| Path | Content |
|---|---|
| `.git/gedo/streams/<key>/stream` | The stream record (replaced through a temporary file and a rename; created with an exclusive create) |
| `.git/gedo/streams/<key>/wal.log`, `wal.lock` | The stream's log (`FileWal`) |
| `<git dir>/gedo/stream-state` | The working tree state (`.git` for the main working tree, `.git/worktrees/<id>` for a linked one) |
| `refs/gedo-local/streams/<key>` | The stream's pin: its work commit, base or publish commit |
| `refs/gedo-local/worktrees/<id>` | The outside work commit of a working tree (`main` for the main one) |
