# Research: History Re-attachment

Decisions taken while planning `specs/005-history-reattachment`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0015, `docs/product/TECH_SPEC.md`) and the clarifications of 2026-10-30 (tape attachment rule, identity link as local derived data with the ADR-0007 rule, detachment date from the local state).

## R1. Where each step lives

- **Decision**:
  - `gedo-crdt` gains the pure logic in a new module `reattach`: the semantic signature of a commit (R3), the matching of a commit's changes against candidate tapes (R4 to R6), the attachment rule (R7) and the identity links (R6). Inputs are plain values (deltas, result hashes, events grouped by tape), outputs a `Reattachment` value; no I/O.
  - `gedo-app::events` gains the orchestration in a new module `events/reattach.rs` (an `impl` block of `EventLog` plus the retention report): read the candidate tapes and their events, read the commit's changes from `IdentityIndex`, call `gedo-crdt`, write the index entries, keep the retention record and fill the cache.
  - `EventLog::events_of_commit` uses the same matching to return the remainder of a commit whose tapes do not account for all its changes (R8).
- **Rationale**: the split of feature 004 (model in `gedo-core`, rules in `gedo-crdt`, orchestration in `gedo-app`); the matching is the part that needs property-based tests without Git.
- **Alternatives considered**: matching inside `gedo-app` (it would mix the rule with storage and make the property tests need a repository); a new crate (the event layer already lives in `gedo-crdt`, and the CRDT of FEAT-066 reuses the matching of entity states).

## R2. Entry points and their inputs

- **Decision**: four use cases on `EventLog`, all taking the `IdentityIndex` the caller has indexed for the commits involved (assumption of the spec):
  - `reattach_rewritten(pairs: &[(GitOid, GitOid)], identities)`: the `post-rewrite` path (FR-001).
  - `reattach_new(commits: &[GitOid], tips: &[GitOid], identities)`: commits that arrived by fetch, without pairs; `tips` are the history tips the caller considers (the refs outside `refs/gedo/*`), as `IdentityIndex::index_history` takes them.
  - `detached(tips, now, grace)`: the retention report (FR-010, FR-011); `now` is a Unix time in seconds and `grace` a number of days, both given by the caller because the core has no clock and no configuration.
  - `fill_cache()`: reads every stored event the cache does not hold and keeps it there (FR-012).
  Each returns a report value (attached tapes per commit, fast path used, unmatched changes; detached tapes; events cached) that the hooks and the daemon log or print.
- **Rationale**: Git's `post-rewrite` hook gives exact pairs; the fetch path only knows new commits. Taking tips as input, like `index_history`, keeps the `RefStore` port limited to `refs/gedo/*` (it cannot list other refs, by design).
- **Alternatives considered**: a new port method listing every ref (widens a port that is deliberately narrow; the CLI and daemon already resolve tips for indexing); one entry point that guesses pairs (Git already knows them for local rewrites).

## R3. The fast path: equal semantic signatures

- **Finding**: ADR-0008 names Git's patch-id as the fast path for local rebases. A patch-id needs a line diff of every changed file; the core has no diff of its own (feature 003 diffs trees, then parses), and a patch-id differs as soon as a rebase resolves a conflict or shifts context, which is exactly when the old and new commits still make the same semantic changes.
- **Decision**: the **semantic signature** of a commit is the list of its deltas and identity merges (the identity results of feature 003, ids as recorded) with each entity's structural hash after the change, in the canonical order of the delta list, hashed with `BLAKE3` in key-derivation mode with the context `"gedo semantic signature v1"`. For a pair whose old commit is indexed, equal signatures attach every tape of the old commit to the new one without matching events (FR-002). When the old commit is not indexed (it was never indexed on this machine), the pair goes to matching.
- **Rationale**: a rebase that does not touch a commit's own changes keeps every id (entities are matched to the parent, ADR-0007) and every result hash, so the signatures are equal; computing it costs one read of the commit's identity record, already needed for matching. The signature never leaves the machine (it is a comparison, not stored data), so its derivation can change without a format change.
- **Alternatives considered**: Git's patch-id (needs a line diff in the core, fails on resolved conflicts); comparing trees of the changed files (differs after any rebase onto a changed base).
- **ADR**: ADR-0016 refines ADR-0008 on this point (R10).

## R4. Matching by entity state, not by operation

- **Finding**: a squash replaces a sequence of operations by their net effect: a function created and then edited twice on the branch appears in the squash commit as one creation with the final content; a rename followed by an edit appears as a rename and a body change with the same final hash. Matching operation by operation (ADR-0008's "same target, operation and resulting structural hash") fails on these, which are the common case.
- **Decision**: for each entity, the candidate events are folded in their total order (clock, id) into the entity's **last state**: the result hash of its last event (or "gone" for a tombstone), and, when its creation is among the candidates, its kind and qualified path updated by later moves and renames. A change of the new commit on entity `X` with result hash `h` **matches** the last event of `X` when that event's result hash is `h` (both tombstones for a removal); every earlier candidate event of `X` is then **superseded**. Several deltas of one entity in the new commit (a rename and a body change) share the same result hash and the same match.
- **Rationale**: the result hash is the entity's whole structural hash after the change (ADR-0014), so equal last states mean the branch and the new commit leave the entity identical, whatever the path in between; this is the reading of "superseded by a later matched event of the same entity" of the clarification.
- **Alternatives considered**: operation by operation (fails on every squash that touches an entity twice); hashing the whole tree of the commit (cannot say which tapes it accounts for).

## R5. Neutral entities and identity merges

- **Decision**: an entity whose candidate events have no net effect, and on which the new commit has no change, is **neutral**: its events are accounted for. Two cases are detected: a creation followed by a tombstone among the candidates, and a run of body and signature changes whose first `before` hash equals the last `after` hash for each of the two hashes (a change and its revert). Live identity-merge events in a candidate tape are accounted for when the new commit's own merges contain the same pair, or when both ids resolve to one entity in the identity index; they never count as a match by themselves.
- **Rationale**: the edge cases of the spec (a revert on the branch, a helper created and deleted) must not stop a tape from attaching; identity merges are facts about identities, not changes of the new commit.
- **Alternatives considered**: treating unaccounted neutral events as unmatched (a squash of a branch with a revert would never re-attach without a pair).

## R6. Entities created on the branch: identity links

- **Finding**: the id of a new entity derives from its kind, qualified path and structural hash when it first appears (ADR-0007). An entity created on the branch and edited later has, in the squash commit, the id derived from its final content, not the id in the live events.
- **Decision**: a `Created` change of the new commit whose id has no candidate events matches the candidate entity whose last state has the same kind, qualified path and result hash and whose creation is among the candidates; ties go to the lowest id. The two ids form an **identity link**. Links are not stored: they are part of the events of the new commit as reconstructed `IdentityMerged` events (kept: the lower id, merged: the higher, ADR-0007), derived on demand like every reconstructed event (ADR-0012), so a machine that only fetched the index entries derives the same links from the same commit and tapes (clarification 2).
- **Rationale**: no new shared format; every consumer of events already applies identity merges, and the lower-id rule is the one every machine uses for merges found at merge commits.
- **Alternatives considered**: a shared `IdentityMerged` event in a new tape attached to the squash (a new shared write per squash, rejected in clarification 2); keeping the live id regardless of order (a second alias rule to keep in step with ADR-0007).

## R7. The attachment rule

- **Decision**: with the matches of R4 to R6, a candidate tape is attached to the new commit when:
  - **with a pair** (the tape belongs to the old commit): at least one of its events is matched or superseded (clarification 1);
  - **without a pair**: every one of its events is matched, superseded or neutral, and at least one is matched or superseded (a tape of neutral events alone attaches nowhere).
  Several new commits given in one call are processed in their generation order, then by id; a tape may attach to several commits (a cherry-pick, a rebase merge replaying a branch, two identical fixes), and FEAT-066 applies shared events once. A commit that already has an index entry is skipped (FR-007).
- **Rationale**: Git's pairs are strong evidence, so one match suffices; a fetched commit has none, so the whole tape must be explained. The order makes the result independent of the order the caller gives (FR-008).
- **Known limit**: without a pair, a detached tape of an unrelated branch whose every event touches only entities that the new commit leaves in the same state as a later matched event can attach too. It needs an unrelated branch that only made changes the squash overwrote with a later identical state; the scripted scenarios of SC-001 include one such branch to measure it, and the CRDT applies an attached event once, so the effect is an extra tape in the history of the entity, never a wrong code state.

## R8. The events of a commit with tapes: tapes, then the remainder

- **Decision**: `CommitEvents::Published` becomes `Published { tapes, remainder }`. The remainder is the reconstructed events of the commit's changes that no event of its tapes accounts for (R4, R5), followed by the identity links (R6), with the clocks and ids of reconstruction (feature 004, research R10), so the events of a commit with a re-attached tape are complete (FR-005). For a commit published live whose tape covers its changes, the remainder is empty. The remainder needs the commit's identities: a commit with tapes that is not indexed returns `IndexError::NotIndexed`, as a commit without tapes already does.
- **Rationale**: a pair whose new commit adds changes (a resolved conflict, an amend that edits code) and a squash with a link must show those events; one variant keeps every caller's code path the same.
- **Alternatives considered**: a third variant `Reattached` (the index cannot say whether an entry came from a publish or a re-attachment without a payload, and a payload would make the same entry differ between machines, which the union of collections refuses); leaving the remainder out (extra changes would be invisible).

## R9. Detached tapes and the retention record

- **Decision**: a tape is detached when none of the commits it is indexed for is reachable from the given tips. Reachability walks the history from the tips once per call and stops as soon as every indexed commit is found; while a tape is really detached the walk covers the whole history (a bound by generation is IMP-006, to measure on a large repository). The local state keeps a **retention record** in the symbol cache (namespace `retention`, one record: tape id to the Unix time it was first seen detached), rewritten in one transaction by `detached` and by every re-attachment that attaches a listed tape (which removes it). A tape without an entry gets `now`. The report lists every detached tape with its events, its date and whether `now - date` exceeds the grace period (FR-011). Nothing is removed.
- **Rationale**: the date is local (clarification 3): the symbol cache is local state under `.git/gedo/`, already open in every use case, and a lost cache only restarts the period, which is the safe side. One record keeps the update atomic without adding a delete to the cache port.
- **Alternatives considered**: a new local file and port (more code for the same guarantee); one cache key per tape (the cache port has no delete, so attached tapes would leave stale keys); a shared record (rejected in clarification 3).

## R10. ADR-0016

- **Decision**: a new ADR, "Re-attachment by entity state with a semantic-signature fast path", refines ADR-0008: the fast path compares semantic signatures instead of Git's patch-id (R3); matching compares entity states instead of single operations (R4); entities created on the branch are linked by reconstructed identity merges (R6); the retention date is local (R9). ADR-0008 stays accepted, amended by ADR-0016. The constitution is regenerated.
- **Rationale**: ADR-0008 states how re-attachment works; changing that needs a decision record.

## R11. Filling the event cache on new data

- **Decision**: `EventLog::fill_cache` lists the `events` collection, checks the cache for each id and reads, decodes and caches in one transaction the records it does not hold, in batches of 1,000. The fetch path (FEAT-091 hooks, FEAT-073 daemon) calls it after `Collection::merge_remote` brings new events; the use case does not depend on how the data arrived. Failures of the cache are logged at `warn` and counted in the report; failures of the store are errors (FR-012, ADR-0015).
- **Rationale**: the first `filter` already does this work on demand; doing it when the data arrives moves the 1.5 to 1.7 s of IMP-005 out of the first query. Checking the cache per id costs one key lookup, small next to reading a blob.
- **Alternatives considered**: diffing the collection's old and new heads to read only the new entries (needs the previous head from the caller and a tree diff for a saving the per-id lookup already gives); filling in the background in the daemon only (the CLI path without the daemon would stay slow).

## R12. Performance

- **Finding**: the budget is 100 ms at p95 to re-attach one fetched commit (TECH_SPEC). The work is: read the commit's changes and result hashes (feature 003 records), list the detached tapes (the commit index and a history walk, R9), read their events (cache, ADR-0015), fold and match (in memory).
- **Decision**: the walk and the detached set are computed once per call, not per commit; candidate events are grouped by target once per call; per commit, only the entities it changes are looked up. The benchmark `reattach` measures one fetched squash commit with 10,000 stored events of which 1,000 are detached, the pair path and `fill_cache` for 10,000 events (SC-004, SC-005).
- **Alternatives considered**: a persistent index of candidate events by target (not needed until measured).

## R13. Tests

- **Decision**:
  - `gedo-crdt/tests/reattach.rs`: property-based tests over generated branches of events (creations, edits, renames, moves, tombstones, reverts) and their squash: every tape attaches, the result does not depend on the order of candidates or commits, a shuffled unrelated branch never changes the result for the real one except in the known limit of R7, links follow the lower-id rule.
  - `gedo-app/tests/reattach.rs`: scripted histories on a real repository (rebase, amend of the message, amend with code, interactive squash and fixup, split, server squash, server rebase merge, cherry-pick, a deleted branch), crash injection between index writes with re-run equality, two machines joining their index entries, the retention report before and after the grace period and after a later attachment, `fill_cache` with an empty, a partial and a refusing cache.
  - Determinism: the index entries of the scripted histories are snapshotted with `insta`, identical on Windows and Linux (SC-002).
- **Rationale**: the spec's acceptance scenarios map one to one to these tests; the property tests cover the matching rule, the scripted ones the use cases.
