# Data Model: Semantic Lanes and Split Publish

Nothing new is stored and no shared format changes (ADR-0014). Every type below is derived in memory.

## gedo-crdt::lanes

| Type | Fields | Rules |
|---|---|---|
| `Lane` | `Refactor`, `Feature` | Ordered: refactor first |
| `FollowUps` | `names: BTreeMap<String, String>` (old name to new name), `paths: Vec<(String, String)>` (old to new qualified path without `crate::`, sorted) | Built from the `Renamed` and `Moved` deltas of the publish (research R3) |
| `LanedChange` | `delta: SemanticDelta`, `part: Part` (ADR-0017), `lane: Lane` | One per delta of the single commit; `part` from the delta kind (R2) |
| `LanePlan` | `changes: Vec<LanedChange>` (delta order), `follow_ups: FollowUps` | `lane_of(id, part)`, `has(lane)`, `parts(lane) -> BTreeSet<(AstNodeId, Part)>` |
| `EntitySource<'a>` | `source: &'a [u8]`, `parsed: &'a ParsedFile`, `map: &'a BlobAstMap` | One version of one file, for the follow-up test and the builder |

Functions: `follow_ups(deltas)`, `classify(deltas, base, final) -> LanePlan` (base and final sources by path), `is_follow_up(base_tokens, final_tokens, follow_ups) -> bool`, `event_lane(plan, event, resolve) -> Lane`.

## gedo-crdt::split

| Type | Fields | Rules |
|---|---|---|
| `FileVersions<'a>` | `path`, `base: Option<EntitySource<'a>>`, `final_: Option<EntitySource<'a>>` | A changed file |
| `RefactorFile` | `path`, `text: Option<Vec<u8>>` | `None`: absent from the refactor tree |
| `BuildError` | `MovedEntityNotFound { id }`, `Overlap { path, id }` | Reported as fallback reasons |

Functions: `build(files, sources_by_id, plan) -> Result<Vec<RefactorFile>, BuildError>` (research R5), `refactor_message(plan) -> String` (R8).

## gedo-app::events::split

| Type | Fields | Rules |
|---|---|---|
| `SplitRequest` | `base: GitOid`, `tree: GitOid` (final), `message: String`, `refactor_message: Option<String>` | The caller's input (FEAT-083 later) |
| `SplitOutcome` | `Split { refactor: PublishedCommit, feature: PublishedCommit }`, `Single { commit: PublishedCommit, reason: Option<NotSplit> }`, `Nothing` | `Nothing`: the final tree equals the base tree and the log is empty |
| `PublishedCommit` | `commit: GitOid`, `tape: Option<TapeId>`, `changes: usize`, `events: usize` | |
| `NotSplit` | `OneLane(Lane)`, `Build(BuildError)`, `SyntaxError { path }`, `Mismatch { id, part, expected: Option<Lane> }` | Why one commit was published |

`EventError` gains no variant: store, index and tape errors keep theirs (`Index` wraps `IndexError`).
