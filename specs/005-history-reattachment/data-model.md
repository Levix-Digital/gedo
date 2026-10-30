# Data Model: History Re-attachment

No new shared record and no change of a stored format: re-attachment only adds commit index entries of feature 004 (`commit-tapes`, format 1, empty payload). New values are in memory or local derived data.

## In memory (gedo-crdt)

### SemanticSignature

- 32 bytes: `BLAKE3` key derivation, context `"gedo semantic signature v1"`, over the postcard encoding of `(deltas, merges, result hashes)` of one commit, in the canonical order of the delta list (research R3).
- Never stored; compared between the old and new commit of a pair.

### CandidateTape

- `tape: TapeId`, `events: Vec<Event>` in tape order.
- `paired: bool`: the tape belongs to the old commit of a rewritten pair (attachment rule of research R7).

### EntityState (fold of the candidate events of one entity)

- `target: AstNodeId`
- `last: Option<StructuralHash>` (`None` after a tombstone)
- `created: Option<(EntityKind, qualified_path: String)>` when the creation is among the candidates, updated by later moves and renames
- `last_event: EventId`, `events: Vec<EventId>` in total order
- `neutral: bool` (research R5)

### ChangeToMatch (one change of the new commit)

- `delta: SemanticDelta` (or an identity merge), `result_hash: Option<StructuralHash>`, `index: usize` in the commit's delta list.

### Reattachment (result for one commit)

- `attached: Vec<TapeId>`, sorted by id
- `accounted: BTreeSet<usize>`: the changes of the commit that attached events account for
- `links: Vec<IdentityMerged>`: kept is the lower id (research R6)
- `unmatched: Vec<usize>`: changes no candidate accounts for (logged at debug, FR-016)
- `fast_path: bool`

Rules: `attached` is a pure function of the commit's changes and the candidates, independent of the order of the candidates (FR-008).

## Use-case values (gedo-app)

### ReattachReport

- `commits: Vec<(GitOid, Reattachment)>` in processing order (generation, then id)
- `skipped: Vec<GitOid>`: commits that already had an index entry (FR-007)

### DetachedTape

- `tape: TapeId`, `commits: Vec<GitOid>` (unreachable), `events: usize`
- `detached_since: u64` (Unix seconds), `eligible: bool` (`now - detached_since >= grace * 86_400`)

### CacheFill

- `listed: usize`, `already_cached: usize`, `cached: usize`, `cache_failures: usize`

### CommitEvents (changed)

```text
Published { tapes: Vec<(TapeId, Vec<Event>)>, remainder: Vec<Event> }
Reconstructed(Vec<Event>)
```

`remainder`: reconstructed events (origin `Reconstructed`, no labels) of the changes no tape event accounts for, then the identity links as `IdentityMerged` events, with the clocks of reconstruction (generation of the commit, sequences after the changes). Empty for a commit whose tapes cover it.

## Local derived data (symbol cache)

### Retention record

- Namespace `retention`, key `detached`, value: a `RecordEnvelope` (format 1) of a sorted map `TapeId -> u64` (first seen detached, Unix seconds).
- Rewritten whole in one transaction by `detached` and by a re-attachment that attaches a listed tape.
- Lost or refused (a cleared cache, a newer format): treated as empty, which restarts the period (clarification 3); a decode failure is logged at `warn`.

### Event cache (feature 004, ADR-0015)

- Namespace `events`, unchanged; `fill_cache` writes to it.

## State transitions of a tape

```text
attached (indexed for a reachable commit)
   │  its commits leave the history (rewrite, deleted branch)
   ▼
detached (date = first seen by this machine)
   │  re-attachment links it again        │  grace period passes
   ▼                                       ▼
attached (date removed)              eligible for gedo gc (FEAT-087)
```
