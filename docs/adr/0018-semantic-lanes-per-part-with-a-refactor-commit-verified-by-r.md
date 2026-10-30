---
id: 18
title: Semantic lanes per part with a refactor commit verified by re-indexing
status: accepted
date: '2026-10-30'
version: v0.3.0
subsystems: [events]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0018: Semantic lanes per part with a refactor commit verified by re-indexing

## Context

The VISION promises clean commits without effort: `gedo publish --split-lanes` turns mixed work into a refactor commit followed by a feature commit, so a rename that touches fifty files no longer hides the three changes that matter. The identity layer (feature 003) gives the deltas between two versions per entity (created, moved, renamed, signature or body changed, removed), and the tree CRDT (ADR-0017) splits an entity into five parts compared on their own. There is no symbol graph yet (EPIC-007), so a reference cannot be resolved to the entity it names; Gedo does not compile code; and a commit that claims to be a refactor but carries a feature change would break the trust the split exists for. The user decided on 2026-10-30 that the refactor lane holds the reference updates a move or rename forces, that work which cannot be split safely is published as one commit with the reason, and that the refactor message is generated unless the caller gives one.

## Decision

- **Lanes per part.** Each delta of a publish is one part of one entity (ADR-0017). Moves (place) and renames (name) are refactor; creations, removals, signature and body changes are feature, except follow-ups. A module created to hold only entities moved into it (a module split) is refactor.
- **Follow-ups.** The base and final tokens of an entity, or of a whole file for the text outside entities, are diffed (Myers, `similar`). A base identifier compares equal to its new name when a rename of the publish changes it (a name renamed to two names follows neither), and a hunk made only of paths is a follow-up when, with the equal path tokens around it, it turns a suffix of an old qualified path of a moved or renamed entity into the matching suffix of its new path. A signature or body change with no other hunk is refactor; one that has both is feature and also changes in the refactor commit.
- **Refactor version.** Each changed file is cut into units (a token with the text before it, or a whole block for an entity the refactor lane moves or creates) and diffed: equal units keep the base's layout with the final tokens, follow-up hunks take the final text, every other hunk keeps the base text; a block is dropped where the base had it and rebuilt where the final version puts it, recursively from its base and final texts. Formatting and comment changes stay in the feature commit.
- **Verification by re-indexing.** `EventLog::publish_split` writes the single commit (the final tree on the base) and indexes it, which gives the deltas to classify and the fallback. With both lanes, it writes the refactor commit (the refactor version on the base) and the feature commit (the final tree on it) and indexes them; the split is published only when every refactor file parses and the parts each commit changes, by identity, are exactly its lane's (a container's body, which changes with its members, aside). Otherwise the single commit is published with the reason (`NotSplit`: one lane, a build error, a syntax error, a mismatch).
- **Tapes.** Each event of the log goes to the tape of its lane's commit (moves and renames to the refactor tape; signature and body events by the lane of their part; the rest to the feature tape); the events, then the tapes, then the index entries are stored and the log is truncated once (ADR-0014).
- **Interfaces.** `gedo_crdt::lanes` (`Lane`, `FollowUps`, `LanePlan`, `classify`, `event_lane`) and `gedo_crdt::split` (`build`, `refactor_message`) are pure; `EventLog::publish_split(request, identities, parser)` returns `SplitOutcome` (`Split`, `Single` with its reason, `Nothing`). No branch is moved: `gedo publish --split-lanes` (FEAT-083) does that. Nothing new is stored and no format changes.

## Alternatives Considered

- **Lanes per entity**: an entity moved and edited would be all feature, so most real refactors would never split.
- **Lanes per file**: a rename touches every caller's file, which usually has feature work too.
- **Moves and renames only, references in the feature commit**: simpler, but the refactor commit would not build; rejected by the user.
- **Starting from the final text and undoing feature changes per entity**: feature-only text outside entities (an import of a new item) would leak into the refactor commit.
- **Verifying by parsing only**: a builder slip could put a feature change in the refactor commit silently.
- **Refusing a split that cannot be verified**: rejected by the user; the work is published as one commit with the reason.
- **A lane label on events**: would change event ids and the frozen format.

## Consequences

- A rename with its callers, a move across files with the paths that name it, and a module split land in the refactor commit, and the feature commit always has the developer's tree; a split that cannot be verified never reaches the history.
- The commits that verification rejects stay unreferenced objects, removed by `git gc`; their identity results stay in the local cache, which is derived data.
- Known limits: references are matched by name and path, not resolved, so a reference the diff cannot tie to a move or rename (a re-export, a macro) keeps its base text and a split may fall back; whole `use` declarations added or removed for a move are kept as in the base; the refactor commit is not compiled.
- Split-publishing a rename whose callers span 100 files, with a feature edit in each and 1,000 events, takes about 215 ms at p50 and 246 ms at p95 on the Linux development container (`cargo bench -p gedo-app --bench split`).
- Verified by the scripted work of `crates/core/gedo-app/tests/split.rs` (renames, moves, module splits, tapes per lane, fallback, 32 generated mixes that always end at the final tree, a pinned outcome) and the unit tests of `gedo_crdt::lanes` and `gedo_crdt::split`.
