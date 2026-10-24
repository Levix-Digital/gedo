# Backlog

Work captured for later planning: bugs, features, improvements and adjustments, grouped under epics when the product is planned as a whole. Capturing an item takes no discovery; when the user decides to work on items, they are promoted into a plan (`plan-memory/backlog.py promote`), where the discovery and the planning happen. Open bugs come before new work.

Each item: `### <ID>: <title>` with the fields `Severity` (bugs: critical, high, medium, low) or `Priority` (others: high, medium, low), an optional `Epic`, `Status` (open, planned, in_progress), `Found`, `Plan` and `Description`. Epics (`EPIC-NNN`) group items of the complete product; they have no `Plan` and are never promoted: their items are. The roadmap (`docs/ROADMAP.md`) says which items go into which release. Closed items (done, dropped, duplicate, superseded by better defined items) move to `Closed` with a `Closed` note and keep their ids, which are never reused. Edit this file with `plan-memory/backlog.py` or by hand, and check it with `backlog.py validate`.

## Bugs

## Features

### FEAT-001: Object store and reference ports with in-memory adapters
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Ports in gedo-core for objects, references, WAL and cache, each with an in-memory adapter for core tests.

### FEAT-002: gitoxide blob and tree adapter
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Read and write blobs and trees in .git/objects with gix, without spawning git or touching the working tree.

### FEAT-003: refs/gedo reference store with compare-and-swap
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Create, list and update references under refs/gedo/* atomically, safe for concurrent CLI and daemon writes.

### FEAT-004: Crash-safe write-ahead log adapter
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Append-only .git/gedo/wal.log with a checksum per record, fsync on consolidation and torn-record recovery.

### FEAT-005: redb symbol cache with rebuild
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Local symbol_cache.db with a schema version, rebuilt from refs/gedo/* on mismatch or corruption (ADR-0004).

### FEAT-006: Refspec setup and transport validation
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Refspecs for refs/gedo/* and validation of push and fetch against GitHub, GitLab and plain remotes, including host retention of custom references.

### FEAT-007: Reference packing measurement and consolidation
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Measure how many refs/gedo/* references hosts and fetch tolerate and consolidate into index trees if needed.

### FEAT-008: SHA-256 repository support
- **Priority**: low
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Support repositories using the SHA-256 object format (open question in the vision).

### FEAT-009: Neutral syntax tree model and parser port
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Language-independent syntax tree in gedo-core and the parser port implemented by gedo-parse.

### FEAT-010: Trivia normalization and structural hash
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Separate significant syntax from trivia and compute the BLAKE3 structural hash with line endings normalized.

### FEAT-011: Three-tier node matching
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Exact match, signature and scope heuristics (80% threshold as a tuned constant) and spawn or tombstone fallback.

### FEAT-012: Deterministic identity derivation and alias reconciliation
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: UUID v8 ids derived from content, versioned matching and IdentityMerged aliases at merge time (ADR-0007).

### FEAT-013: Semantic delta emission
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: NodeCreated, NodeMoved, NodeRenamed, SignatureMutated, BodyModified and NodeTombstoned deltas.

### FEAT-014: Reference identity corpus and benchmarks
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Corpus of renames, moves and formatter runs measuring accuracy (95% target) and determinism across machines.

### FEAT-015: Indexing of history made without Gedo
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Index existing repositories and commits made by teammates without Gedo, with the same ids on every machine.

### FEAT-016: Rust support
- **Priority**: high
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Rust.

### FEAT-017: TypeScript support
- **Priority**: medium
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for TypeScript.

### FEAT-018: Python support
- **Priority**: medium
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Python.

### FEAT-019: JavaScript support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for JavaScript.

### FEAT-020: Java support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Java.

### FEAT-021: Shell (Bash) support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Shell (Bash).

### FEAT-022: C# support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for C#.

### FEAT-023: C++ support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for C++.

### FEAT-024: PHP support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for PHP.

### FEAT-025: Go support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Go.

### FEAT-026: PowerShell support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for PowerShell.

### FEAT-027: C support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for C.

### FEAT-028: Kotlin support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Kotlin.

### FEAT-029: Lua support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Lua.

### FEAT-030: Assembly support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Assembly.

### FEAT-031: Ruby support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Ruby.

### FEAT-032: Dart support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Dart.

### FEAT-033: Swift support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Swift.

### FEAT-034: R support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for R.

### FEAT-035: Groovy support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Groovy.

### FEAT-036: Visual Basic .NET support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Visual Basic .NET.

### FEAT-037: VBA support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for VBA.

### FEAT-038: MATLAB support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for MATLAB.

### FEAT-039: Perl support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Perl.

### FEAT-040: GDScript support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for GDScript.

### FEAT-041: Elixir support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Elixir.

### FEAT-042: Scala support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Scala.

### FEAT-043: Delphi support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Delphi.

### FEAT-044: Lisp support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Lisp.

### FEAT-045: Zig support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Zig.

### FEAT-046: Erlang support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Erlang.

### FEAT-047: Fortran support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Fortran.

### FEAT-048: Ada support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Ada.

### FEAT-049: F# support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for F#.

### FEAT-050: OCaml support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for OCaml.

### FEAT-051: Gleam support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Gleam.

### FEAT-052: Prolog support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Prolog.

### FEAT-053: COBOL support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for COBOL.

### FEAT-054: Mojo support
- **Priority**: low
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Grammar mapping, qualified paths and conformance suite for Mojo.

### FEAT-055: JSON support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of JSON files with a conformance suite.

### FEAT-056: YAML support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of YAML files with a conformance suite.

### FEAT-057: TOML support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of TOML files with a conformance suite.

### FEAT-058: Markdown support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of Markdown files with a conformance suite.

### FEAT-059: SQL support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of SQL files with a conformance suite.

### FEAT-060: HTML and CSS support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of HTML and CSS files with a conformance suite.

### FEAT-061: HCL support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of HCL files with a conformance suite.

### FEAT-062: XML support
- **Priority**: low
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Structural tracking of XML files with a conformance suite.

### FEAT-063: Event model with Lamport clocks and content-addressed ids
- **Priority**: high
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Event, SemanticOp, Lamport clock and author fingerprint, with deterministic ids for reconstructed events.

### FEAT-064: Event tapes and commit index
- **Priority**: high
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Content-addressed events grouped in tapes, with a commit-to-tape index (ADR-0008).

### FEAT-065: Re-attachment after rebase, amend and squash
- **Priority**: high
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Rebuild the commit index after local and server-side rewrites by semantic diff and event matching; 90-day retention of unreachable events.

### FEAT-066: Tree CRDT with deterministic convergence
- **Priority**: high
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Set semantics for members of a structure, commutative move and edit, idempotent events; same result in any arrival order.

### FEAT-067: Semantic lanes and split publish
- **Priority**: high
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Classify events into refactor and feature lanes; gedo publish --split-lanes creates a refactor commit then a feature commit.

### FEAT-068: Virtual streams
- **Priority**: high
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: gedo stream <name>: switch or isolate work in memory without git stash or checkout.

### FEAT-069: Deterministic sequencer
- **Priority**: medium
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Order events from several developers with Lamport and CRDT rules, shared by the merge driver, the Action and the relay.

### FEAT-070: Semantic merge driver with textual fallback
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: gedo merge-driver merges per syntax node and falls back to gitoxide's textual merge whenever it is not certain.

### FEAT-071: Merge driver registration in local Git configuration
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: gedo init registers merge.gedo.driver in .git/config and the supported languages in .git/info/attributes.

### FEAT-072: Differential merge test suite on real histories
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Replay real merge histories through Git and the driver and compare results; merge safety requirement.

### FEAT-073: Background fetch in the daemon
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: gedod runs the user's git fetch periodically (default 5 minutes), paused offline or on metered networks.

### FEAT-074: Shadow merge and gedo radar
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: In-memory merge of uncommitted work and local commits with the upstream and related remote branches; collisions in gedo radar and notifications.

### FEAT-075: Semantic diff (gedo delta)
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Classify changes as moved, renamed, refactored or changed.

### FEAT-076: Post-merge contract check
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: post-merge and post-rewrite hooks report calls that no longer match a changed contract.

### FEAT-077: Symbol dependency graph from syntax references
- **Priority**: high
- **Epic**: EPIC-007
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Directed graph of AstNodeIds (calls, implements, uses) built from the neutral syntax tree.

### FEAT-078: Minimum-cut graft (gedo graft)
- **Priority**: high
- **Epic**: EPIC-007
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Move a symbol and the minimal closure of dependencies it needs into another stream.

### FEAT-079: Contract-break detection
- **Priority**: high
- **Epic**: EPIC-007
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Detect callers that no longer match a changed public signature before compilation.

### FEAT-080: Language server type oracle
- **Priority**: low
- **Epic**: EPIC-007
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: A port and LSP adapter that refines the graph with precise types and diagnostics.

### FEAT-081: CLI skeleton and composition root
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: gedo-cli with clap, the composition root and the use-case dispatch in gedo-app.

### FEAT-082: gedo init
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Run git init unchanged, then prepare .git/gedo/, refspecs, hooks, merge driver and the first AST maps.

### FEAT-083: gedo publish
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Consolidate the WAL into tapes and create the native Git commit.

### FEAT-084: gedo trace
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Semantic history of one entity across renames, moves and formatting.

### FEAT-085: Daemon with file watcher and ingest
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: gedod watches the working tree with notify and ingests each save (parse, identity, WAL) within the latency requirements.

### FEAT-086: gedo doctor and structured logs
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Diagnostics of versions, paths, hook chain, merge driver and overlay consistency; logs in .git/gedo/logs.

### FEAT-087: gedo gc
- **Priority**: medium
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Run git gc unchanged, then prune Gedo data past its retention.

### FEAT-088: Machine-readable and accessible output
- **Priority**: medium
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: --json output for Gedo commands; NO_COLOR and non-TTY handling; no information by color alone.

### FEAT-089: Command dispatcher and pure pass-through
- **Priority**: high
- **Epic**: EPIC-009
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Unknown commands and aliases run the user's git with the same arguments, streams and exit code.

### FEAT-090: Hook installation and chaining
- **Priority**: high
- **Epic**: EPIC-009
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Install Gedo hooks and chain existing ones (.git/hooks, core.hooksPath, Husky, lefthook, pre-commit) in order, respecting exit codes.

### FEAT-091: Hook-driven overlay sync
- **Priority**: high
- **Epic**: EPIC-009
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: post-commit, post-checkout, post-merge, post-rewrite, pre-push and reference-transaction keep the overlay in step; failures never break Git.

### FEAT-092: Git versus gedo end-to-end suite
- **Priority**: high
- **Epic**: EPIC-009
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: The same scenarios run through git and gedo with identical output, exit code and repository state.

### FEAT-093: gedo recover
- **Priority**: medium
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Restore any saved state of a file or stream from the WAL.

### FEAT-094: Operation log and gedo undo
- **Priority**: medium
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Record Gedo and Git operations and undo the last ones safely.

### FEAT-095: Force-push guard
- **Priority**: medium
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: pre-push blocks force pushes that would erase commits authored by others, with an explicit override.

### FEAT-096: GitHub Action with semantic diff and contract checks
- **Priority**: medium
- **Epic**: EPIC-011
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Post gedo delta and contract checks on pull requests.

### FEAT-097: Re-attachment after server-side merges
- **Priority**: medium
- **Epic**: EPIC-011
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: The Action re-attaches Gedo data after squash and rebase merges on the host.

### FEAT-098: Merge queue semantic independence check
- **Priority**: medium
- **Epic**: EPIC-011
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Check that pull requests in the host's merge queue are semantically independent.

### FEAT-099: GitLab CI integration
- **Priority**: low
- **Epic**: EPIC-011
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: The same checks and re-attachment for GitLab merge requests.

### FEAT-100: Cargo.lock merge regeneration
- **Priority**: medium
- **Epic**: EPIC-012
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Regenerate Cargo.lock from the merged manifests.

### FEAT-101: npm, pnpm and Yarn lockfile regeneration
- **Priority**: medium
- **Epic**: EPIC-012
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Regenerate JavaScript lockfiles from the merged manifests.

### FEAT-102: Python lockfile regeneration
- **Priority**: medium
- **Epic**: EPIC-012
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Regenerate uv, Poetry and pip-tools lockfiles from the merged manifests.

### FEAT-103: Generated file detection
- **Priority**: low
- **Epic**: EPIC-012
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Detect generated files and regenerate or take one side instead of merging text.

### FEAT-104: Developer intents model
- **Priority**: medium
- **Epic**: EPIC-013
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: SymbolLocked and SymbolMutating intents published from the daemon.

### FEAT-105: Optional self-hostable relay
- **Priority**: medium
- **Epic**: EPIC-013
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: A relay for intents, threads and leases with second-level latency; technology decided in its own ADR.

### FEAT-106: gedo peek
- **Priority**: medium
- **Epic**: EPIC-013
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Inspect a colleague's work in progress on a symbol.

### FEAT-107: Soft leases for binary files
- **Priority**: medium
- **Epic**: EPIC-013
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Leases with heartbeat and expiry instead of manual locks for files that cannot be merged.

### FEAT-108: Peer-to-peer in local networks
- **Priority**: low
- **Epic**: EPIC-013
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: The same messages without a relay in corporate networks.

### FEAT-109: Conflict-of-intent choices
- **Priority**: medium
- **Epic**: EPIC-014
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: On a colliding change: adopt the other change, propose a unification or keep your own for now.

### FEAT-110: Negotiation threads with executable snapshots
- **Priority**: medium
- **Epic**: EPIC-014
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Multi-turn threads anchored to a node, each message with a comment and a validated code snapshot.

### FEAT-111: gedo inbox and gedo accept
- **Priority**: medium
- **Epic**: EPIC-014
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: List pending proposals and accept one, grafting the agreed version on both sides (CONSENSUS_REACHED).

### FEAT-112: Contract micro-acknowledgment
- **Priority**: low
- **Epic**: EPIC-014
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Changes to public contracts ask their owners for a quick acknowledgment while the code is still being written.

### FEAT-113: Stack split proposal
- **Priority**: medium
- **Epic**: EPIC-015
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Propose how to split a change into a stack by lane and by symbol group from the dependency graph.

### FEAT-114: Stack branches and pull requests on the host
- **Priority**: medium
- **Epic**: EPIC-015
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: gedo stack creates and updates the branches and pull requests of a stack on GitHub.

### FEAT-115: Automatic restack
- **Priority**: medium
- **Epic**: EPIC-015
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Restack every level with the semantic merge when a lower level changes or merges.

### FEAT-116: gedo playback
- **Priority**: low
- **Epic**: EPIC-016
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Show the events and threads of a commit or range, also after squash merges.

### FEAT-117: Gedo language server
- **Priority**: medium
- **Epic**: EPIC-017
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: A language server exposing collisions, threads, leases and node identity to editors.

### FEAT-118: VS Code extension
- **Priority**: medium
- **Epic**: EPIC-017
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: A lightweight extension over the Gedo language server.

### FEAT-119: CI on Windows and Linux
- **Priority**: high
- **Epic**: EPIC-018
- **Status**: planned
- **Found**: 2026-10-24
- **Plan**: gedo_store_foundation
- **Description**: Build, tests, benchmarks, check_design.py, cargo-deny and cargo-audit on every pull request on both platforms.

### FEAT-120: Release pipeline and crate name reservation
- **Priority**: high
- **Epic**: EPIC-018
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Release archives for the MVP targets, cargo install and early reservation of the gedo crate names.

### FEAT-121: Windows code signing
- **Priority**: medium
- **Epic**: EPIC-018
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Authenticode signing of Windows binaries.

### FEAT-122: winget package
- **Priority**: medium
- **Epic**: EPIC-018
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Publish Gedo to winget.

### FEAT-123: Linux packages
- **Priority**: low
- **Epic**: EPIC-018
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Packages for the main Linux distributions.

### FEAT-124: macOS support
- **Priority**: low
- **Epic**: EPIC-018
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: macOS targets, signing and notarization.

### FEAT-125: Windows on ARM
- **Priority**: low
- **Epic**: EPIC-018
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: aarch64-pc-windows-msvc target.

### FEAT-126: Live pair programming on symbols
- **Priority**: low
- **Epic**: EPIC-019
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Two developers working live on the same symbols through the relay.

### FEAT-127: Stable data formats and compatibility guarantee
- **Priority**: medium
- **Epic**: EPIC-018
- **Status**: open
- **Found**: 2026-10-24
- **Plan**: -
- **Description**: Freeze the formats of refs/gedo/*, the WAL and the cache, document the compatibility policy and test that every later release reads them (the 1.0 stability promise).

## Improvements

## Adjustments

## Epics

### EPIC-001: Git object store bridge
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Read and write Gedo data in the Git object store and refs/gedo/* natively, with compare-and-swap updates (scope 1).

### EPIC-002: Syntactic identity tracking
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Stable, deterministic identities for code entities across edits, renames, moves and formatting (scope 2).

### EPIC-003: Language coverage
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Order: Rust, then the Levix stacks (TypeScript, Python), then the Stack Overflow Developer Survey 2025 and GitHub Octoverse 2025 rankings combined with Reciprocal Rank Fusion (k = 60); a language is supported only with a mature Tree-sitter grammar and a passing conformance suite.

### EPIC-004: Data and markup formats
- **Priority**: low
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Structural tracking for data, configuration and markup formats, after the programming languages (scope 4).

### EPIC-005: Event log and convergence
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Events, tapes robust to rewrites, a deterministic tree CRDT, semantic lanes and virtual streams (scope 5).

### EPIC-006: Semantic merge and early conflict detection
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Gedo takes part in every Git merge and finds collisions while they are born (scope 6, ADR-0009).

### EPIC-007: Symbol graph and contracts
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Dependencies between symbols, minimum-cut graft and contract-break detection (scope 7).

### EPIC-008: Developer CLI and daemon
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: The gedo command line and the gedod daemon over the gedo-app use cases (scope 8).

### EPIC-009: Full Git command compatibility
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Every Git command works through gedo; hooks keep the overlay correct when git is used directly (scope 9, ADR-0006).

### EPIC-010: Safety net
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Never lose work: recovery, undo and force-push protection (scope 10).

### EPIC-011: Hosting integration
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Gedo value on the host for every teammate, with or without Gedo installed (scope 11, ADR-0010).

### EPIC-012: Lockfiles and generated files
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Lockfiles and generated files are derived artifacts regenerated on merge, never merged as text (scope 12).

### EPIC-013: Distributed control plane
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Intents, peek and soft leases over three tiers: Git remotes, an optional relay and peer-to-peer (scope 13, ADR-0010).

### EPIC-014: Symbol inbox
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Conflicts negotiated per symbol, asynchronously, before the pull request (scope 14).

### EPIC-015: Stacked pull requests
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Small dependent pull requests without rebase pain (scope 15).

### EPIC-016: History playback
- **Priority**: low
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Event-by-event evolution of a delivery and its negotiations, keeping the main history clean (scope 16).

### EPIC-017: IDE integration
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Gedo in the editor: tree status, collisions, threads and leases (scope 17).

### EPIC-018: Platforms and distribution
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Windows and Linux first, macOS later; signed releases and package managers (scope 18).

### EPIC-019: Pair programming
- **Priority**: low
- **Status**: open
- **Found**: 2026-10-24
- **Description**: Live collaboration on the same symbols, built on the control plane, much later (scope 19).

## Closed
