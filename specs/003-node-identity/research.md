# Research: Node Identity

Decisions taken while planning `specs/003-node-identity`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0011, `docs/product/TECH_SPEC.md`) and the clarifications of 2026-10-25 (change-set matching, whole history, accuracy on scripted edits).

## R1. Where each step lives

- **Decision**:
  - `gedo-core` gains the identity model: `AstNodeId`, `AnchoredNode`, `BlobAstMap`, `SemanticDelta`, `IdentityMerged`, `MatchFormat`, and a `unit` field on `ParsedFile` and `SyntaxMap` (R4).
  - `gedo-ast` gains the pure algorithms: `matching` (three tiers over a change set), `identity` (derivation), `delta` (classification and order), `alias` (reconciliation and resolution).
  - A new core crate **`gedo-app`** gains the first use case, `index` (history indexing): it walks commits through the `ObjectStore` port, parses through the `Parser` port, calls `gedo-ast` and keeps its results in the `SymbolCache` port. No new port is needed.
- **Rationale**: the algorithms stay pure and are tested with property-based tests without Git or Tree-sitter; the use case is the only place that combines ports, as ADR-0001 and the core page plan for `gedo-app`.
- **Alternatives considered**: putting indexing in `gedo-ast` (it would need the ports, mixing I/O orchestration with algorithms); putting it in `gedo-store` (an adapter would then hold business rules).

## R2. Identity results are local derived data, keyed by commit

- **Finding**: an entity's identity depends on its history, not only on the blob that contains it. The same blob at the same path can be reached by two histories (a function renamed on one branch, written fresh on another) and carry different ids there. A record "immutable per blob" (`refs/gedo/ast-map/<blob-sha>` in TECH_SPEC) is therefore not well defined: two machines that index the branches in different orders would store different content under one key, and the union of collections (ADR-0011) would conflict.
- **Decision**: identities are computed **per commit**. A commit's ancestry is fixed by its hash, so the identities of its entities are a pure function of the commit, whatever the order of indexing (clarification 2). Results are **local derived data** in the symbol cache (namespace `identity`), like the rest of the cache: rebuilt by indexing again when missing, corrupted or of another format. Nothing is written to `refs/gedo/*` by this feature.
- **Why not share them**: a result can always be recomputed from the Git history, and recomputing is cheaper than transferring it. Parsing is about 2.4 ms per 1,000 lines (feature 002, R1) and only changed files are parsed (R7); an identity record of one changed file is about 5 KB of ids and hashes that do not compress, so sharing the results of a 10,000-commit history would add over 100 MB to `refs/gedo/*` to save seconds. Live identities (from saves, before a commit exists) travel as events in v0.3.0 (ADR-0007: "events recorded live keep their id when they travel"), which is where shared identity data belongs. If the indexing benchmark (R10) shows that `gedo init` misses its 30 s budget on the reference repository, sharing becomes a backlog item measured against that number.
- **Consequences**: an ADR (ADR-0012) records the decision and amends ADR-0003 (the `ast-map` namespace and the alias table are not shared collections) and the data table of TECH_SPEC; FR-010 and FR-011 of the spec are adjusted (local reuse, local versioned records). This changes the note given with clarification 2 ("results published in `refs/gedo` are reused"), which is brought to the user at plan approval.
- **Alternatives considered**: per-blob records with first-writer-wins (non-deterministic across machines); per-commit trees of maps in `refs/gedo` deduplicated by Git (deterministic, but needs tree-valued collection entries, a new store layout, and still grows with history); deriving ids from blobs only (loses renames, contrary to the product's purpose).

## R3. Matching works on a change set

- **Decision**: the unit of matching is a **change set**: for a commit, the Rust files that differ from the parent (added, deleted, modified, renamed); for a single file, that file alone. Unchanged files keep their results without being parsed. Matching compares the *old entities* of all files of the change set (anchored, with ids) with the *new entities* of all its files (syntax maps without ids). An entity can therefore leave one file and be matched in another (clarification 1). A file rename is a deletion and an addition in the change set, so its entities match through tier 1 or 2 and are reported as moved.
- **Tiers** (all deterministic, R6):
  1. **Exact**: same unit, kind, qualified path, occurrence and structural hash. Kept, no delta (the byte range may have moved).
  2. **Continuity and heuristics**, among entities not yet matched and of the same kind:
     - (a) **Same place**: same unit, kind, qualified path and occurrence, different hash. Kept; deltas from the hashes (body modified, signature mutated). This keeps identities through any edit of an entity that stays where it is, and keeps a damaged entity (with a syntax error) attached to its old identity (FR-008).
     - (b) **Same content elsewhere**: same structural hash, different qualified path. Kept when the pair is unique for that hash on both sides; the deltas say moved or renamed.
     - (c) **Similarity**: every remaining pair is scored (R5); pairs at or above `SIMILARITY_THRESHOLD = 0.80` are accepted greedily from the highest score down, each entity used once.
     - (d) **Containers by children** (added during implementation): an unmatched container (an `impl` whose type was renamed, a module) continues the unmatched old container of the same kind that more than half of its children came from, innermost containers first. Without it, renaming a type tombstoned its `impl` blocks, since their qualified path and hashes both change.
  3. **Fallback**: a new entity left unmatched gets a new id (created); an old entity left unmatched is tombstoned, except as in R8.
- **Containers first**: entities are matched in tiers over the whole change set, then their `container` links are resolved; the scope term of R5 compares the containers' ids, so a method whose `impl` was matched gets credit for staying in it.
- **Alternatives considered**: per-file matching (loses moves between files, rejected by clarification 1); matching every pair by score (quadratic and less predictable than exact tiers for the common case).

## R4. The unit of an entity

- **Decision**: the parser adds `unit` to `ParsedFile` (copied to `SyntaxMap`): the repository path of the compilation unit's root directory, from the same rule as the module path (feature 002, R7): the path up to and including the nearest `src/`, `tests/`, `benches/` or `examples/` directory, plus the target name for a file directly in `tests/`, `benches/` or `examples/` and for `src/bin/<name>`. For `crates/core/gedo-ast/src/hash.rs` it is `crates/core/gedo-ast/src`; for `tests/transport.rs` it is `tests/transport`. A file outside any such directory has the unit of its directory.
- **Rationale**: qualified paths start with `crate::` without the crate name (clarification 3 of feature 002), so two crates of a workspace can contain the same qualified path; the unit tells them apart without reading manifests. It is added to the model (an additive field, filled by the adapter) because the rule is language-specific and belongs next to the module path rule.
- **Consequence**: the conformance snapshots of `gedo-parse` gain the field. The hash format does not change (`unit` is not hashed).

## R5. Similarity score (tier 2c)

- **Decision**: for an old entity `o` and a new entity `n` of the same kind:

  `score = 0.45 × signature + 0.40 × body + 0.15 × scope`

  - `signature`: Dice coefficient of the multisets of signature tokens (class and text), with the entity's own name token excluded so a rename alone does not lower it.
  - `body`: 1.0 when the body hashes are equal; otherwise the Dice coefficient of the multisets of body tokens; 1.0 when both have no body.
  - `scope`: 1.0 when the containers resolve to the same id (or both are at file level in the same unit), 0.5 when they are in the same unit, 0 otherwise.

  The threshold `SIMILARITY_THRESHOLD = 0.80` and the weights are named constants in `gedo-ast::matching`, tuned on the corpus (R9). A rename with an unchanged body and scope scores 1.0. The weights are a starting point, not a measurement: the corpus (R9) reports accuracy and false continuations per category, and the weights and threshold are tuned there before release (a change after release is a new `MatchFormat`).
- **Bounds**: scoring is limited to `MAX_SCORED_PAIRS = 50,000` pairs per kind; above it, candidates are restricted to pairs sharing a name, a body hash or a container, so a rewritten file of thousands of entities stays linear in practice and never compares every pair without limit (spec edge case).
- **Tuning result (T029)**: the starting values were kept. On the corpus (674 cases over 19 categories, seed `0x6765646f2d696431`), identity accuracy is 99.98% overall and at least 99.61% in every category (formatter runs are the lowest), with 0 false continuations out of 110 entities that should be new (copies, splits, merges, replacements, additions, new modules). The gates pass with a wide margin, so no tuning was needed and `MatchFormat::V1` uses 450/400/150 thousandths and a threshold of 800.
- **Alternatives considered**: tree edit distance (precise but slow, and Tree-sitter trees are not in the core); LCS over token sequences (order-sensitive, quadratic per pair; Dice over multisets is linear and good enough at the entity level, to be confirmed by the corpus).

## R6. Determinism and tie-breaking

- **Decision**: every collection the algorithms iterate is ordered: entities by `(unit, qualified_path, kind, occurrence)`, scored pairs by `(score desc, old id asc, new key asc)`, scores compared as integers (the score is computed in fixed-point thousandths), so no floating-point difference between platforms can change a result. No hash map iteration order reaches a result.
- **Verification**: property-based tests shuffle the input order of entities and files; the history tests index in several orders; the snapshot of identities is compared on Windows and Linux (SC-003).

## R7. Deterministic identity derivation

- **Decision**: a new id is `AstNodeId` (16 bytes, UUID version 8, RFC 9562 layout) made from `blake3::Hasher::new_derive_key("gedo node id v1")` over: the kind code (one byte), the unit, the qualified path (each as a little-endian `u32` length and bytes), the occurrence (`u32`), the structural hash (32 bytes) and a collision counter (`u32`, 0 first). The first 16 bytes of the digest get the version (`8`) and variant (`10`) bits. The counter grows only when the derived id is already the id of a live entity of the same commit (an entity renamed away and another created with its old name and content); the next free value is used, so the rule stays deterministic. `MatchFormat::V1` names these rules together with R3, R5 and R6.
- **Resurrection**: an entity deleted and later created again with the same unit, path and content derives its old id (ADR-0007).
- **Older ids**: `AstNodeId` is 16 opaque bytes; ids of version 4 or 7 written by earlier formats are read as they are. No `uuid` crate is needed for that, so none is added (it is listed in TECH_SPEC for v0.3.0's `event_id`).
- **Alternatives considered**: UUID v5 (SHA-1 based, a second hash function); the full 32-byte digest as id (doubles every reference to a node, and the core types already use 16 bytes).

## R8. Syntax errors

- **Decision**: a damaged new entity is matched like any other; tier 2a keeps its old identity when it stays in place. When a file of the change set has `has_error`, its old entities that are not matched are **carried**, not tombstoned: they stay in the file's result without a byte range, flagged `detached`, and are matched again against the next version of that file. A damaged new entity that matches nothing gets an id and a `Created` delta like any other (changed during implementation: holding the delta back would need state across commits, and a later version without errors keeps the id through tier 1 or 2 anyway).
- **Rationale**: an unbalanced brace can swallow the rest of a file (feature 002, R8); without this rule a half-typed save would tombstone and recreate everything after it.

## R9. Reference identity corpus (FEAT-014)

- **Decision**: a generator in `gedo-app`'s tests applies scripted edits to real Rust files (the conformance fixtures of `gedo-parse` and the Gedo workspace sources at a pinned commit, committed as test data) with a fixed seed, and records for every case the expected outcome: which old entity each new entity continues, or none, and the expected deltas. Categories: body edit (one statement, a quarter, half of the body), signature change (parameter added, removed, type changed, return type, visibility), rename (alone, with a body edit), move (to another module of the same file, to another file, file renamed), formatter run (the committed `rustfmt` variants of feature 002), reorder, copy, split, merge of two entities, deletion, addition, and a syntax error injected into one entity.
- **As built**: the sources are the conformance fixtures and the rustfmt corpus of feature 002 (`crates/adapters/gedo-parse/tests/fixtures/rust/`), read in place; its `formatted/default` files are workspace sources pinned by feature 002, so no second copy is committed. Signature changes are covered by an added parameter and a visibility change; a replacement category (an unrelated function written where a deleted one was) was added because it is the hardest case for false continuations. The corpus checks identities only; expected deltas are checked case by case in `gedo-ast/tests/matching.rs`.
- **Measurement**: identity accuracy = preserved identities that should be preserved ÷ identities that should be preserved, per category and overall (SC-001), and false continuations = kept identities that should be new ÷ entities that should be new (SC-002). The test fails below 95% overall, 90% per category or above 1% false continuations, and prints the per-category table. The edits are text operations on the entities' byte ranges, so the generator needs only the parser.
- **Real histories** (clarification 3) measure determinism, not accuracy: the Gedo repository's own history (CI checks it out with full depth) and generated branchy histories (R10).
- **Alternatives considered**: a hand-labelled sample of real commits (clarification 3, not chosen); committing the generated cases (megabytes of derived data; the seed and the generator reproduce them).

## R10. History indexing (FEAT-015)

- **Decision**: `gedo_app::index::index_history(tips)` takes the commits to index (the caller chooses them: branch tips, fetched commits; reading `refs/heads/*` belongs to the CLI of v0.4.0). It:
  1. walks every ancestor from the given tips to every root (clarification 2), skipping commits already in the cache, and orders the rest topologically, parents first, ties by commit id;
  2. for each commit, diffs its tree against its first parent's (subtrees with equal ids are skipped), parses the changed Rust blobs (each blob and path parsed once per run, in parallel threads, since parsing does not depend on history), and matches the change set against the first parent's results (R3);
  3. for a merge commit, also matches the merge's entities against each other parent's results over the same change set; when an entity carries id `X` from the first parent and continues id `Y ≠ X` of another parent, the lower id is kept and an `IdentityMerged { kept, merged, commit }` record is produced (ADR-0007, R11);
  4. writes the commit's record to the cache in one transaction, so an interrupted run resumes at the first commit without a record (SC-006).
- **Commit record** (`identity` namespace, key `commit/<oid>`): the results of the changed files (`BlobAstMap` per path, or deleted), the deltas and the identity merges of that commit, in a `RecordEnvelope` of format 1. Results of a file at a commit are found by walking the first-parent chain to the last commit that changed it; every 64 first-parent commits a **checkpoint** record (`checkpoint/<oid>`) lists every Rust path with the digest of its map, so a lookup reads at most 64 records. Maps are stored once per content digest (`map/<digest>`).
- **Performance**: the spike numbers of feature 002 give about 2 to 3 ms of parsing for a typical commit (a few hundred changed lines); matching is linear in the changed entities. The non-blocking bench job indexes a pinned public Rust repository (ripgrep at a fixed tag, cloned by the CI step; its size is recorded with the results) and the Gedo repository, and reports the time per commit and in total (SC-005).
- **Alternatives considered**: indexing from a shared starting commit (clarification 2, not chosen); a commit-graph library (gix has one, but the core cannot depend on it; the walk needs only `read_commit`).

## R11. Alias reconciliation

- **Decision**: identity merges form an undirected relation between ids; every id resolves to the **lowest id of its connected component** (union-find with path compression, rebuilt from the records). The result does not depend on the order in which merges were found or on which machine, and a chain `A → B → C` resolves to the lowest of the three. An entity's id in a commit after a merge is the resolved one; results of earlier commits keep the ids they had, and lookups resolve them.
- **Where it is used now**: history indexing at merge commits (R10). The same function serves the merge driver of v0.4.0 and the `IdentityMerged` event of v0.3.0, which are out of scope.
- **Alternatives considered**: directed aliases with first-writer-wins (order-dependent); keeping the id of the first parent (asymmetric: indexing the merge from another parent order would give another answer).

## R12. Deltas

- **Decision**: `SemanticDelta` is one of `Created`, `Moved`, `Renamed`, `SignatureMutated`, `BodyModified`, `Tombstoned`, each with the `AstNodeId` and the details it needs (before and after qualified path, unit and file for moves and renames; before and after signature hash for signature changes; before and after body hash for body changes). For a kept entity: `Moved` when the container's path (unit, file or enclosing qualified path) changed, `Renamed` when the name changed, then `SignatureMutated` and `BodyModified` from the hashes; several can apply to one entity. The list is ordered by delta kind in the order above, then by id, so it is identical everywhere (FR-006).

## R13. Testing

- **Decision**:
  - `gedo-ast`: unit tests per tier and per delta, property-based tests (input order invariance, idempotence of matching a version against itself, derivation stable and collision-free in generated sets, alias resolution invariant under permutation and grouping).
  - `gedo-app`: history tests on the in-memory object store with generated histories (linear, branches, merges, octopus merges, several roots, interrupted runs), indexed in at least 3 orders (SC-003, SC-006); the identity corpus (R9); an end-to-end test over the Gedo repository's own history with a committed snapshot of the identities of selected commits, compared on Windows and Linux.
  - Fuzzing: a `fuzz/` target decodes arbitrary bytes as commit records and maps (SC-007), run by the existing `fuzz.yml`.
  - Benchmarks: `match_change` (2,000-line file against its previous version, SC-004) and `index_history` (R10, SC-005).
