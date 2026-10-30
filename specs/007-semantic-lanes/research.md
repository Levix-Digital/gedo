# Research: Semantic Lanes and Split Publish

Decisions taken while planning `specs/007-semantic-lanes`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0017, `docs/product/TECH_SPEC.md`) and the clarifications of 2026-10-30 (reference updates belong to the refactor lane, an unsafe split falls back to one commit with the reason, the refactor message is generated unless the caller gives one).

## R1. Where each step lives

- **Decision**:
  - `gedo-crdt` gains two pure modules. `lanes`: the lane of each change of a publish (R2 to R4) and of each event (R7). `split`: the source text of every file of the refactor version, built from the base and final sources, their parsed tokens and identity maps, and the lanes (R5); and the generated refactor message (R8). Inputs are plain values; no I/O.
  - `gedo-app::events` gains the use case in `events/split.rs` (an `impl` block of `EventLog`): `publish_split(request, identities, parser)` writes the commits, indexes and verifies them, assigns the events and publishes one tape per commit (R6, R9).
- **Rationale**: the split of features 004 to 006 (model in `gedo-core`, rules in `gedo-crdt`, orchestration in `gedo-app`). The text builder is the part that needs many small cases and property tests without Git.
- **Alternatives considered**: the builder in `gedo-app` (property tests would need a repository); a new crate (the lanes are part of the event layer, ADR-0001 lists them in `gedo-crdt`).

## R2. Lanes per part of an entity

- **Decision**: the changes of a publish are the deltas of a commit with the final tree on the base (R6), as the identity index computes them. Each delta is one part of one entity (ADR-0017): `Created` and `Tombstoned` the presence, `Moved` the place, `Renamed` the name, `SignatureMutated` the signature, `BodyModified` the body. Its lane:
  - `Moved` and `Renamed`: **refactor**;
  - `SignatureMutated` and `BodyModified`: **refactor** when the change is a reference follow-up (R3), **feature** otherwise;
  - `Created` of a module whose members in the final version are all moved in (a module split, R4): **refactor**; every other `Created` and every `Tombstoned`: **feature**.
- **Rationale**: the parts are what the identity layer and the tree CRDT already track, so a move and a body change of one entity land in different commits, exactly as ADR-0017 lets them commute; the classification needs nothing that is not already computed.
- **Alternatives considered**: lanes per entity (an entity moved and changed would be all feature, so most real refactors with any edit would never split); lanes per file (a rename touches every caller's file, which usually also has feature work).

## R3. Reference follow-ups: renamed names and moved paths

- **Finding**: without the symbol graph (EPIC-007, later) Gedo cannot resolve a reference, but it has the base and final tokens of every changed entity and the old and new names and paths of every moved and renamed entity.
- **Decision**: a **follow-up map** is built from the deltas: for each `Renamed`, the old name to the new name; for each `Moved` or `Renamed`, the old qualified path to the new one (without `crate::`). The tokens of the base and final versions of an entity (or of a file region, R5) are compared with a diff (Myers, over token texts; whitespace is not a token, comments are). A pair of base and final tokens compares **equal** when the texts are equal or the base text is renamed to the final text by the map. Each remaining hunk is a **path follow-up** when both sides are made only of identifiers and `::`, and the maximal path around the hunk on each side (the hunk extended over the equal path tokens next to it) is `p::s_old` and `p::s_new`, with the same prefix `p`, `s_old` a suffix of an old qualified path at a segment boundary and `s_new` the suffix of the matching new path with the same number of leading segments removed. A body or signature change is a follow-up when the diff has no hunk other than path follow-ups.
- **Rationale**: it accepts exactly what a rename or move forces on its references (`sum(x)` to `total(x)`, `billing::tax(x)` to `billing::tax::tax(x)`, `use crate::billing::tax;` to `use crate::billing::tax::tax;`) and nothing else; a wrong acceptance (a local variable that happens to share the old name) cannot produce a wrong split, because the refactor version is verified (R6).
- **Alternatives considered**: equal-length substitution only (misses every path change of a move); any change near a renamed name (too loose, would put feature edits in the refactor commit).

## R4. Text outside entities and module splits

- **Decision**: text outside entities (imports, attributes of the file, `mod` declarations, comments between items) is diffed like an entity's text (R5): equal and follow-up hunks take the final text, others keep the base text. A `use` declaration is an `Import` entity of the parser (never in a `SyntaxMap`); a whole `use` declaration added or removed in the final version whose path names the new (or old) path of a moved entity is a follow-up. A created module (inline, or `mod x;` with its file) whose final members are all moved in, with no created member, is refactor, so a module split lands whole in the refactor commit.
- **Rationale**: the clarification asks for a refactor commit that stands on its own; imports and module declarations are what a move needs besides the call sites.
- **Alternatives considered**: imports always from the final version (feature-only imports would leak into the refactor commit and could name items that do not exist there yet); imports always from the base (a moved function's callers would import a path that no longer exists).

## R5. Building the refactor version of a file

- **Decision**: for each file the publish changes, `split::build_file` takes the base source and parsed file (or none), the final source and parsed file (or none), the identity maps of both, the lanes and the follow-up map, and returns the refactor text (or none, the file absent). The tokens of each side become **units**: a token with the whitespace before it, except that every entity whose place is refactor-changed (moved within, into or out of the file) is one **block** unit keyed by its id. The two unit lists are diffed (R3 equality; blocks equal by id). Then:
  - equal units take the final text (follow-up substitutions included);
  - a block of the final side (equal or inserted) is emitted as the refactor text of that entity, built recursively from its base text (in whatever file it was) and its final text;
  - a block of the base side that is not equal is dropped (the entity moved away);
  - a follow-up hunk takes its final side; any other hunk keeps its base side, so created entities, removals, feature edits and formatting changes stay as in the base.
  A file present only in the final version whose refactor text has no unit is absent from the refactor tree; a file absent from the final version is absent when all its entities moved out, and otherwise keeps its base text minus the moved-out blocks.
- **Rationale**: one rule, applied to whole files and recursively to moved entities, gives the base plus exactly the refactor changes; blocks keep moved entities from being diffed against unrelated tokens.
- **Alternatives considered**: starting from the final text and undoing feature changes per entity (text outside entities, such as feature-only imports, would leak into the refactor commit); a three-way textual merge (no notion of lanes).

## R6. Commits, indexing and verification

- **Decision**: `publish_split` writes the **single commit** first (final tree, parent the base, the caller's message) and indexes it, which gives the deltas of R2 and is the fallback. When both lanes have changes, it builds the refactor tree (R5, blobs and trees through `ObjectStore`), writes the **refactor commit** (parent the base, R8 message) and the **feature commit** (the final tree, parent the refactor commit, the caller's message), and indexes them. The split is **verified**: every file of the refactor tree that changed parses without a syntax error; the parts changed by the refactor commit (its deltas, as (id, part) pairs) are exactly the refactor-lane parts and those of the feature commit exactly the feature-lane parts. On a failure the outcome is the single commit with the reason (the file, or the entity and part). The commits that are not used stay unreferenced objects, removed by `git gc` like any other.
- **Rationale**: the identity index is the authority on what a commit changed, so verifying with it guarantees that the split shows what the lanes say; writing commits is cheap and harmless because nothing references them until the caller moves the branch (FEAT-083).
- **Alternatives considered**: verification by parsing only (a builder slip could move a feature change into the refactor commit silently); refusing on failure (rejected in the clarification).

## R7. Lanes of events and the two tapes

- **Decision**: the events of the log are assigned by their operation: `Moved` and `Renamed` events go to the refactor tape; `SignatureMutated` and `BodyModified` to the refactor tape when the same part of the same entity (ids resolved through the index) is refactor in R2, otherwise to the feature tape; `Created`, `Tombstoned` and identity merges to the refactor tape when their entity's presence is refactor (a module split), otherwise to the feature tape. Events about entities the publish no longer changes (a save later undone) go to the feature tape. Publishing stores the events, then both tapes, then both index entries, and truncates the log once, through the last record read (ADR-0014 order, each step idempotent); with a single commit it is the plain `publish`. A lane with no event gets no tape.
- **Rationale**: each commit's tape then holds the events of its own changes, so `events_of_commit` returns them with their authors and labels, and the re-attachment of feature 005 finds them after a rebase of either commit.
- **Alternatives considered**: a lane label on events (would change event ids and the format, ADR-0014); one tape for both commits (the refactor commit would have no events of its own).

## R8. The generated refactor message

- **Decision**: `refactor: ` followed by up to three changes in a stable order (renames, then moves, each sorted by old qualified path), as `rename <old path> to <new name>` and `move <old path> to <new file>`, separated by `; `, with `and N more` when there are more; then a blank line and every change, one per line. The caller may give its own message instead.
- **Rationale**: the clarification; deterministic text keeps the commit content reproducible and reviewable.

## R9. Determinism and idempotence

- **Decision**: the lanes, the refactor texts and the messages are functions of their inputs (sorted maps, no hashing of pointers); the trees are content-addressed. The commit ids depend also on the repository identity and time `ObjectStore::write_commit` uses, so two calls give the same commits when those are equal; the tapes are content-addressed, so a second publish of the same events stores nothing new.
- **Rationale**: FR-013 and the crash safety of ADR-0014.

## R10. Tests and benchmarks

- **Decision**: unit and property tests in `gedo-crdt` (`tests/lanes.rs`): lanes per delta kind, follow-ups, module splits; for the builder, generated base and final files of renames, moves and feature edits whose refactor text, re-parsed, carries exactly the refactor changes, and property: the final tree is always the last commit's tree. Use-case tests in `gedo-app` (`tests/split.rs`) on temporary repositories for each acceptance scenario and each fallback reason, with an `insta` snapshot of the outcome. A `split` benchmark (100 files, 1,000 events) for SC-003, in the non-blocking bench job.
- **Rationale**: the pattern of features 004 to 006.
