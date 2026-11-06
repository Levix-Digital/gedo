# Research: Virtual Streams

Decisions taken while planning `specs/009-virtual-streams`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0019, `docs/product/TECH_SPEC.md`) and the clarifications of 2026-11-06 (a stream isolates its work as a tree kept by Gedo with its context and its own events; streams stay on the machine; a publish writes the commit with the events as its tape, moves no branch, and the stream stays open on the new commit).

## R1. Where each part lives

- **Decision**:
  - `gedo-core` gains the model in a new module `stream`: `StreamName` (the name rule, R3), `Stream` (the record, R4) with its invariants, and a new port `ports::stream::StreamStore` (R2) with an in-memory adapter in `memory::stream` and contract tests.
  - `gedo-crdt` gains a pure module `streams`: `StreamFilter` (labels and agent, R8) and the choice of the recording context (`Stream::context(email)` and `recording_context(active, email, agent_text, labels_text)`, R6).
  - `gedo-app` gains the use cases in a new module `streams` (`Streams`, R5 to R11) over `ObjectStore`, `RefStore` and `StreamStore`, reusing `EventLog` (record, publish, pending) over each stream's own log.
  - `gedo-store` gains the file adapter `FileStreamStore` (R2).
- **Rationale**: the split of features 004 to 008 (model and ports in `gedo-core`, rules in `gedo-crdt`, orchestration in `gedo-app`, I/O in adapters, ADR-0001). `EventLog` already records live events with a context on top of a base and publishes a log as a tape with the guarantees of ADR-0014; a stream only needs it over another log.
- **Alternatives considered**: streams inside `EventLog` (it would grow a second responsibility and need a store it does not have); a new crate (the event layer already owns streams, ADR-0001 lists them in `gedo-crdt`).

## R2. The `StreamStore` port and where streams are kept

- **Finding**: the symbol cache is local *derived* data, rebuilt or dropped at will (ADR-0004, ADR-0012); a stream is not derived, so it cannot live there. `refs/gedo/*` is pushed (ADR-0011), and the clarification keeps streams on the machine. The write-ahead log adapter (`FileWal::open(dir)`) already gives a durable log with one writer at a time in any directory. `git gc` removes objects that no reference reaches (after `gc.pruneExpire`, two weeks by default), so a saved work tree that only a Gedo file names would be lost.
- **Decision**: a new port, opaque like the WAL port (the core encodes and checks the records):

  ```rust
  pub trait StreamStore {
      type Log: WriteAheadLog;
      fn names(&self) -> Result<Vec<StreamName>, StoreError>;
      fn read(&self, name: &StreamName) -> Result<Option<Vec<u8>>, StoreError>;
      fn create(&self, name: &StreamName, record: &[u8]) -> Result<bool, StoreError>; // false: exists
      fn replace(&self, name: &StreamName, record: &[u8]) -> Result<(), StoreError>;
      fn remove(&self, name: &StreamName) -> Result<(), StoreError>;            // record, log, pin
      fn open_log(&self, name: &StreamName) -> Result<Self::Log, StoreError>;   // one writer, waits
      fn pin(&self, name: &StreamName, commit: Option<GitOid>) -> Result<(), StoreError>;
      fn read_state(&self) -> Result<Option<Vec<u8>>, StoreError>;              // this working tree
      fn write_state(&self, state: &[u8], outside: Option<GitOid>) -> Result<(), StoreError>;
  }
  ```

  The file adapter keeps each stream in `.git/gedo/streams/<key>/` (the common Gedo directory, shared by every working tree): the record in `stream` (written to a temporary file and renamed; `create` uses an exclusive create, so of two concurrent creations exactly one succeeds) and the stream's log in `wal.log` through `FileWal`. `<key>` is the hexadecimal form of the name's bytes, so no name depends on the file system's rules (`/`, case, `con` and `nul` on Windows). The working tree's state is `gedo/stream-state` in that working tree's own Git directory (`.git` for the main one, `.git/worktrees/<id>` for a linked one). **Pins** are local references outside `refs/gedo/`, never pushed and never fetched (ADR-0011 pushes and fetches only `refs/gedo/*`): `refs/gedo-local/streams/<key>` for a stream and `refs/gedo-local/worktrees/<id>` (`main` for the main working tree) for the outside work; each points to a **work commit** (R7) so `git gc` keeps the work tree and its base.
- **Rationale**: one directory per stream gives each stream its own log with the WAL's durability and locking, and its removal is one directory; the hexadecimal key makes every valid name storable on every platform; references are Git's own way to keep objects alive, so no Gedo-specific `gc` rule is needed.
- **Alternatives considered**: one file for every stream (a single writer for all streams, so two agents would wait on each other); streams under `refs/gedo/streams` (pushed with the rest, against the clarification); records in the symbol cache (derived data, may be dropped); readable directory names (`dc/m2` as two directories, `CON` refused on Windows, `A` and `a` one directory on Windows and macOS).

## R3. Stream names

- **Decision**: 1 to 64 bytes; segments separated by `/`, each non-empty, of `[a-z0-9._-]`, starting with a letter or digit, not `.` or `..` (so `dc/m2`, `agent/metrics`, `feat-retry` are valid; `Dc/M2`, `a//b`, `/a`, `a/`, `-a`, `a b` are not). `StreamName::new` returns the reason.
- **Rationale**: the agent identifier rule of FEAT-063 with `/` added for the names tools will use (`agent/<id>`, `dc/<milestone>`, the `UX_REFERENCE.md` examples); lower case only, so names never differ by case only.
- **Alternatives considered**: Git branch name rules (allow upper case and characters that make names differ only by case on Windows); no `/` (tools would encode their namespaces themselves).

## R4. The stream record

- **Decision**: the record is a format byte (`1`) followed by the postcard encoding of `{ name, agent: Option<AgentId>, labels: Labels, base: Option<GitOid>, work: Option<GitOid>, publishing: Option<GitOid> }`. `work` is the saved work tree, `None` when the stream's work is its base's tree; `publishing` is the commit of a publish in progress (R9). Reading checks the format byte, the decoding, that the name equals the stream it was read for, the label rules of FEAT-063 and that no label key is reserved; a refused record is an error with the stream's name. The working tree state is a format byte and `{ active: Option<StreamName>, outside: Option<GitOid> }`.
- **Rationale**: the stored-format style of features 001 to 008 (format byte, postcard, checked on read), frozen at v1.0.0 like the others; name, agent and labels are written once at creation and never rewritten by any use case, as FR-001 asks.
- **Alternatives considered**: JSON (another encoding next to postcard for no gain; the JSON contract of ADR-0013 is the CLI's output, not the stored form).

## R5. Creating, reading and listing

- **Decision**: `Streams::create(name, agent, labels, base)` validates labels with `Labels::for_recording` (reserved prefix refused) and creates the record with `StreamStore::create`; an existing name is `StreamError::Exists`. `get(name)` reads one stream (`StreamError::NotFound`); `list(filter)` reads every record from `names()` once and returns those the filter keeps, in name order.
- **Rationale**: the store's exclusive create makes "already exists" exact under concurrency; listing 1,000 streams is 1,000 small file reads (SC-003, under 100 ms).
- **Alternatives considered**: an index file of names and labels (a second place to keep consistent, for a list that is rarely large).

## R6. The recording context

- **Decision**: `Stream::context(email)` is `RecordingContext::agent(email, agent, labels)` for an agent stream and `RecordingContext::person(email, labels)` otherwise. `recording_context(active, email, agent_text, labels_text)` returns the active stream's context when a stream is given, and otherwise `RecordingContext::from_texts(email, agent_text, labels_text)` (today's `GEDO_AGENT` and `GEDO_LABELS` behavior); inside a stream the texts are ignored and a debug event says so when they were set. `Streams::recording_context(email, agent_text, labels_text)` reads the working tree's active stream and applies it.
- **Rationale**: the backlog item: events inside a stream inherit its author and labels; outside, the environment names them. Ignoring the environment inside a stream keeps a stream's events uniform, which is what a tool filtering by the stream's labels relies on.
- **Alternatives considered**: merging the environment's labels into the stream's (two sources for one label, and a stream's events would differ by process).

## R7. Switching and the work commits

- **Decision**: `Streams::switch(target: Option<&StreamName>, current: GitOid) -> Result<Option<GitOid>, StreamError>`, where `current` is the tree of the caller's working files:
  1. The target stream, when given, must exist (`NotFound`, nothing changes).
  2. The current work is saved where the working tree is: in the active stream (its log is opened first, R10; `work` becomes `current`, or `None` when `current` is its base's tree) or, without an active stream, as the working tree's outside work.
  3. Each saved work is pinned by a **work commit**: a commit with the saved tree, the stream's base (if any) as parent and the message `gedo stream work`, written with `ObjectStore::write_commit` and set as the pin; a stream whose work is its base is pinned by its base itself.
  4. The state is written with the target as the active stream, and the result is the target's saved work, its base's tree, the empty tree for a stream without a base, or, when switching out of streams, the saved outside work (`None` when nothing was saved: the caller keeps the tree of `HEAD`).
  Switching to the active stream saves `current` and returns it. Gedo writes no file of the working tree, no index, no `HEAD` and no branch.
- **Rationale**: the clarification: switching saves the current work and gives back the other, without stash or checkout; the CLI (v0.4.0) builds `current` from the working files and writes the returned tree. A commit as the pin keeps both the work tree and its base alive with one reference, the way `git stash` does.
- **Alternatives considered**: pinning the tree directly (a second reference would be needed for the base); saving work as blobs per file in the record (a second tree format next to Git's).

## R8. Filters

- **Decision**: `StreamFilter { labels: Labels, agent: Option<AgentId> }`; a stream matches when every label of the filter is present with the same value and, when an agent is given, the stream's agent is that agent. `StreamFilter::from_texts(labels_text, agent_text)` reads the `key=value,...` text form of FEAT-063, as the CLI's `--label` will. The empty filter keeps every stream.
- **Rationale**: the same matching rule as `EventFilter` for labels (every given pair, exact value), so a tool uses one rule for events and streams (ADR-0013).
- **Alternatives considered**: any-of matching (the dev-cycle needs "this plan and this milestone").

## R9. Publishing a stream

- **Decision**: `Streams::publish(name, tree: Option<GitOid>, message) -> Result<StreamPublish, StreamError>`, holding the stream's log for the whole call (R10):
  1. A publish in progress is completed first (step 4 and 5 with the recorded commit).
  2. The tree is `tree`, or the saved work, or the base's tree. With no pending event and a tree equal to the base's, the outcome is `NothingToPublish` and nothing is written.
  3. The commit is written (the tree, the base as only parent or none, the caller's message; author and committer are the repository identity as for every commit Gedo writes), then the record is replaced with `publishing: Some(commit)` and the pin set to the commit.
  4. `EventLog::publish(commit)` over the stream's log stores the events, the tape and the index entry and truncates the log (ADR-0014; nothing when the log is empty).
  5. The record is replaced with `base: Some(commit)`, `work: None`, `publishing: None`; the pin stays the commit.
  Every operation that opens a stream's log (record, switch, publish, discard) first completes a publish in progress, with a warning. No branch moves; the outcome gives the commit and the tape (or none, a commit without events).
- **Rationale**: a commit's id depends on its time, so a publish interrupted after the commit is written would otherwise write another commit when retried and the events could end on the wrong one; recording the commit before the tape makes the retry use the same commit, and every later step is idempotent (FR-010). The pin keeps the new commit alive until the caller moves a branch to it.
- **Alternatives considered**: closing the stream on publish (the clarification keeps it open); publishing into the repository's log (would mix the stream's events with the outside work's).

## R10. Concurrency

- **Decision**: a stream's log has one writer at a time (`FileWal`'s lock, waiting up to `WRITER_WAIT_MS` then `WalBusy`), and every use case that changes a stream's record holds the stream's log while it does, so changes of one stream are serialized by its log. Creation relies on the exclusive create. The working tree state is replaced atomically; two processes switching the same working tree at the same time end with the last one's state (the CLI runs one command per working tree at a time).
- **Rationale**: no new lock: the log a stream already needs is its lock. A process that keeps a stream's log open (the daemon, later) makes the others wait, the same rule as the repository's log.
- **Alternatives considered**: a lock file per stream next to the log (a second lock for the same resource).

## R11. Convergence and discard

- **Decision**: `Streams::side(name) -> Result<Side, StreamError>` returns `Side { tip: base, live: pending events }` for `EventLog::converge` (feature 006); a stream without a base is `StreamError::NoBase`. `Streams::discard(name, force)` opens the stream's log, counts its pending events and checks its saved work; without `force`, a stream with either is refused with `StreamError::HasWork { events, work }`; otherwise the store removes the record, the log and the pin, and the working tree's state is cleared when the stream was active there. Reading a state whose active stream no longer exists returns no active stream with a warning (a stream discarded from another working tree).
- **Rationale**: converging streams needs nothing new from feature 006; refusing to drop work by default keeps the product's "no work is lost" promise, and the explicit flag is what `gedo stream discard --force` will pass.
- **Alternatives considered**: enumerating every working tree on discard (the store would need to list the linked working trees; a lazy check is enough and handles a working tree that is offline, on a removable drive).

## R12. Tests, benchmark and observability

- **Decision**:
  - `gedo-core`: name rule unit and property tests (generated valid and invalid names), record encoding round trip, refused records, `StreamStore` contract tests run against the memory adapter and, in `gedo-store/tests/contract.rs`, against `FileStreamStore`.
  - `gedo-crdt`: filter and context rules, a property that every context of a stream carries its agent and labels.
  - `gedo-app/tests/streams.rs`: one scenario per acceptance scenario of the spec on temporary repositories (create and list across processes, record inside and outside, switch in both directions and in two working trees, publish with interruptions injected after each step, nothing to publish, converge with and without conflicts, discard with and without `force`, a discarded active stream), with an `insta` snapshot of the outcomes; a property that every switch returns the tree last saved for its target.
  - `gedo-store/tests/streams_gc.rs`: a stream's saved work and a published commit survive `git gc --prune=now` (skipped with a message when `git` is not installed, as the transport tests).
  - Benchmark `gedo-app/benches/streams.rs` (SC-003): create, switch and get; list 1,000 streams; publish of a stream against `publish` of the same events.
  - `tracing`: `stream created`, `stream switched`, `stream published` (commit, tape, events), `stream discarded` (info); `events recorded in a stream` (debug); `stream publish completed`, `stream record refused`, `active stream missing` (warn); `event_tracing.rs` asserts the info events.
- **Rationale**: the test layout of features 004 to 008; SC-001 and SC-002 are the scenario and property tests.

## R13. ADR

- **Decision**: **ADR-0020** "Virtual streams as local records with their own log and pinned work commits": streams kept per machine in `.git/gedo/streams`, one log per stream, the active stream per working tree, work saved as trees pinned by work commits under `refs/gedo-local/`, events inheriting the stream's author kind and labels (ADR-0013), the publish recorded before its tape. The constitution is regenerated.
