# Research: Syntax Model

Decisions taken while planning `specs/002-syntax-model`. Each was checked against the constitution (hexagonal core without I/O, ADR-0001 to ADR-0011, `docs/product/TECH_SPEC.md`).

## R1. Tree-sitter and the Rust grammar

- **Decision**: `tree-sitter` 0.27 and `tree-sitter-rust` 0.24, used only in the new adapter crate `gedo-parse`. The grammar crate exposes its language through `tree-sitter-language` 0.1, which `tree-sitter` 0.27 accepts, so the versions of the technical specification work together.
- **Verified in a spike** (2026-10-24, Linux container, 4 vCPUs): both crates build with Rust 1.97 and declare `rust-version = 1.90`, under the workspace MSRV of 1.95. Both are MIT, `tree-sitter-language` is MIT OR Apache-2.0, and the only other new dependency, `streaming-iterator`, is MIT OR Apache-2.0: all allowed by `deny.toml`. The C sources are compiled by `cc` at build time; no `unsafe` code is needed in Gedo (the workspace forbids it).
- **Speed**: a full parse of 519 files of crates in the local registry (298,775 lines) took 716 ms, about 2.4 ms per 1,000 lines; parse plus a naive walk and hash of a 6,044-line file took 19 ms at p50. A 2,000-line file therefore needs about 5 to 7 ms for parse, normalization and hash, inside the 10 ms of SC-001.
- **Alternatives considered**: `syn` (exact Rust parser, but fails on the first syntax error, which breaks User Story 4, and is Rust-only, which breaks the language-neutral design); `rust-analyzer`'s parser crates (error tolerant, but Rust-only and with unstable, frequently breaking APIs).

## R2. Grammar coverage

- **Decision**: the real-world sweep (SC-006) runs over the sources of every crate in the workspace's `Cargo.lock` (pinned and checksummed by Cargo, already downloaded by `cargo fetch`) plus the Gedo workspace; files that fail because of a known grammar gap are listed in `crates/adapters/gedo-parse/tests/known_grammar_gaps.toml` with the construct, and each gap has a backlog item.
- **Verified in the spike**: 519 registry files parsed with one error file, a `serde_json` UI test that is meant not to compile. The Gedo workspace (46 files) parsed with no error. Edition 2024 constructs `use<>` bounds, let chains, `#[unsafe(...)]` attributes, async closures, `&raw const` and generic associated types parse; `unsafe extern "C" { pub safe fn f(); }` (Rust 1.82) does not, and is the first known grammar gap (the error stays local to that block, see R6).
- **Alternatives considered**: vendoring crates into the repository (megabytes of test data); fetching from crates.io in CI (adds a network dependency to the tests).

## R3. Where each step lives

- **Decision**: three places, following ADR-0001 and the architecture pages:
  - `gedo-core` gains the neutral model (`Language`, `EntityKind`, `Token`, `TokenClass`, `ParsedFile`, `RawEntity`, `SyntaxMap`, `CodeEntity`, `StructuralHash`, `HashFormat`), the `Parser` port and `ParseError`.
  - The new core crate `gedo-ast` gains normalization and hashing: pure functions from a `ParsedFile` to a `SyntaxMap`. Feature 003 adds matching, identity derivation and deltas to the same crate.
  - The new adapter crate `gedo-parse` implements the port with Tree-sitter: language detection, grammar node to `EntityKind` mapping, token classification and Rust qualified paths.
- **Rationale**: the language-specific knowledge (which grammar node is a function, how a module path is built) stays in the adapter; the rules that make hashes stable and deterministic are language-neutral and pure, so they are tested without Tree-sitter, with property-based tests, and shared by every future language.
- **Alternatives considered**: hashing inside the adapter (simpler, but the determinism rules would be duplicated per language and could not be tested without the grammar); a separate `gedo-syntax` crate (no second user of the model besides `gedo-ast`, so the model stays in `gedo-core` with the other shared types).

## R4. Token model and trivia

- **Decision**: the adapter emits every leaf of the tree as a `Token { class, byte_range }` with a neutral `TokenClass`: `Identifier`, `Keyword`, `Literal`, `Punctuation`, `OpenDelimiter`, `CloseDelimiter`, `Separator`, `OptionalPunctuation`, `Comment`, `DocComment`, each with a fixed one-byte code (1 to 10) used by the hash. Whitespace is not a token (Tree-sitter does not produce it). Normalization drops `Comment` and `DocComment` tokens (clarification 1) and `OptionalPunctuation` tokens (R5), so whitespace, indentation, line breaks, comments and formatter punctuation never reach the hash.
- **Model types**: byte positions are a `ByteRange { start, end }` of `u32`, token positions in a file a `TokenRange`; the significant tokens of a `CodeEntity` are `NormalizedToken { class, text }`. `ParsedFile` and `SyntaxMap` carry a file-level `has_error` besides the per-entity flag, because an error can sit between entities.
- **Literal text**: line endings inside literals are normalized (CRLF and lone CR to LF) before hashing, so a file checked out with `core.autocrlf` on Windows hashes like its Linux checkout. A leading UTF-8 byte order mark is skipped by the adapter before parsing and byte ranges are reported against the original bytes.

## R5. Formatter invariance

- **Decision**: three neutral rules beyond dropping trivia, each driven by information the adapter provides:
  1. A `Separator` token immediately followed by a `CloseDelimiter` is dropped (trailing commas, which `rustfmt` adds or removes with the line width).
  2. Entities of kind `Import` (Rust `use` declarations, and each item of a `use` list, which `rustfmt` sorts too) and module declarations without a body (`mod foo;`) are order-insensitive within their container: the container's hash covers its other tokens in order plus the sorted list of the hashes of those declarations. `Import` is a kind the adapter emits for normalization only; it is not returned as an entity of the syntax map.
  3. `OptionalPunctuation` tokens are dropped. The adapter gives this class to the punctuation that the language's formatter adds or removes without changing the meaning. For Rust (found by running the corpus over the workspace and about 300 registry files): the comma after a match arm and the trailing comma of a `where` clause; the braces of a match-arm or closure body whose block holds a single expression, jump statement or unterminated expression statement; and the `;` after `return`, `break` and `continue` when nothing but comments follows the statement in its block (in the middle of a block, `return; g()` and `return g()` differ). The angle brackets of generic parameter and argument lists are delimiters, so rule 1 covers their trailing commas. The comma of a one-element tuple (`(a,)`, a type `(u8,)`, a pattern `(a,)`) is ordinary punctuation and never dropped, since it is what makes the tuple. The walk decides "last child" and "nothing follows" from its cursor, never by asking a node for its next sibling, which Tree-sitter answers in time linear in the number of siblings.
- **Known residual differences** (not normalized in V1): `rustfmt` merging two `#[derive]` attributes, and splitting `>>` inside macro token trees; neither appears in the corpus.
- **Checked against `rustfmt` 1.9 options** that change tokens: `trailing_comma` (rule 1), `reorder_imports` and `reorder_modules` (rule 2), `match_block_trailing_comma` (rule 1), `use_try_shorthand` and `imports_granularity` (change the meaning-equivalent syntax itself; out of the formatting corpus, see the spec's edge cases). The corpus uses the default configuration, `max_width = 60` and `max_width = 140` with `trailing_comma = "Never"`, plus line ending conversion and re-indentation.
- **Alternatives considered**: hashing a canonical pretty-print (a formatter per language, slow, and exactly what the token stream already avoids); hashing the tree shape instead of tokens (Tree-sitter node layouts change between grammar versions more often than tokens do).

## R6. Hash encoding and format version

- **Decision**: `HashFormat::V1`. Each hash is `blake3::Hasher::new_derive_key("gedo structural hash v1")` fed a part tag byte (0 structural, 1 signature, 2 body) and then a stream of items. A significant token is its class code (one byte), its text length as a little-endian `u32` and its normalized text bytes, which makes the encoding unambiguous (`ab`+`c` differs from `a`+`bc`). A nested entity is the tag `0xF0` followed by its structural hash, in place of its tokens, so a change reaches only the entity and its containers; an order-insensitive nested entity (R5 rule 2) is taken out of the stream and appended after it, sorted, each as `0xF1` and its hash. Three hashes per entity: `signature` over the items before the body, `body` over the body items, and `structural` = H(0 ‖ signature ‖ body). An entity without a body (a unit struct, a constant) has the body hash of zero items.
- **Version rule**: once a hash has been stored, any change to R4, R5 or this encoding that alters a hash for some input requires `HashFormat::V2`; the snapshot test of User Story 3 fails on such a change, which forces the bump. The fixes of the syntax model review (plan `gedo_syntax_model_review_fixes`: one-element tuples, use-list items, mid-block jumps) changed V1 itself, because no hash had been stored or released yet.
- **Alternatives considered**: a keyed hash with a constant key (same effect as `derive_key`, less explicit); SHA-256 (rejected in ADR-0002).

## R7. Rust entity kinds and qualified paths

- **Decision**: mapping from `tree-sitter-rust` 0.24 nodes:

  | Grammar node | `EntityKind` | Name |
  |---|---|---|
  | `mod_item` | `Module` | `name` field |
  | `function_item`, `function_signature_item` | `Function`, or `Method` inside `impl_item` / `trait_item` | `name` |
  | `struct_item`, `enum_item`, `union_item` | `Struct`, `Enum`, `Union` | `name` |
  | `trait_item` | `Trait` | `name` |
  | `impl_item` | `Impl` | see below |
  | `const_item`, `static_item` | `Constant`, `Static` (`AssociatedConstant` inside `impl` / `trait`) | `name` |
  | `type_item`, `associated_type` | `TypeAlias`, `AssociatedType` | `name` |
  | `macro_definition` | `Macro` | `name` |
  | `use_declaration` | `Import` (normalization only, R5) | none |

  Enum variants and fields stay in their parent's tokens (clarification 2). Items inside function bodies are entities whose container is the function.
- **Module path of a file**: the path below the nearest `src/`, `tests/`, `benches/` or `examples/` directory; `lib.rs`, `main.rs` and `build.rs` (outside those directories) are crate roots; `mod.rs` names its directory; `a/b.rs` is `a::b`; any file directly in `tests/`, `benches/` or `examples/` is its own crate root. A directory of `src/bin/` is one binary rooted in its `main.rs`, so its other files are modules of that binary (`src/bin/tool/helper.rs` is `crate::helper`). A subdirectory of `tests/`, `benches/` or `examples/` keeps its name as a module (`tests/common/mod.rs` is `crate::common`), because one file cannot tell shared modules from a multi-file target. The path always starts with `crate` (clarification 3). Path components that are not valid identifiers (a file `my-file.rs`) are kept as written; the path is a name, not something Gedo resolves.
- **Impl and method paths**: an inherent `impl` is named `<module>::<impl Type>` and its items `<module>::Type::item`; a trait `impl` is named `<module>::<Type as Trait>` and its items `<module>::<Type as Trait>::item`. `Type` and `Trait` are their source tokens joined without trivia (`Stack<T>`, `Vec<u8>`), so generic impls of the same type with different arguments stay distinct. Items of a `trait` are `<module>::Trait::item`.
- **Duplicates**: when two entities of one file have the same kind and qualified path (`#[cfg]` variants), each gets an `occurrence` index in source order (0 for the first); the identity layer uses it to tell them apart.

## R8. Error tolerance

- **Decision**: Tree-sitter always returns a tree; nodes that contain `ERROR` or `MISSING` nodes set the `has_error` flag of the entities that contain them. Entities outside the damaged region are mapped normally, so their tokens and hashes equal those of the valid file. The error-injection corpus deletes each `{`, `}`, `;` and `(` of the fixtures one at a time, truncates the files at every 1/20 of their length and inserts stray tokens. An unbalanced brace decides which block every later item belongs to, so its damaged region runs to the end of the file (measured: deleting the closing brace of a method moves every later item of the file into its trait); for the other edits the region is the edited token.
- **Limits**: `MAX_SOURCE_BYTES = 16 MiB` (larger files are refused with `TooLarge`; generated files of that size are not hand-edited code) and `MAX_NESTING_DEPTH = 4096` for the adapter's walk, which is iterative (a `TreeCursor`), so deep nesting cannot overflow the stack; trees deeper than the limit are refused with `TooDeep`. The sweep found a valid generated table (`unicode-normalization`) whose or-pattern chain is 741 levels deep, so the first limit of 512 was raised. A time limit of 2 s turns pathological inputs into `Timeout` instead of hanging. It covers the parse (Tree-sitter's progress callback) and the walk after it (the clock is read every 4,096 nodes), and `TreeSitterParser::with_timeout` sets another limit. A syntax error inside the attributes or comments attached to an entity flags that entity, since they are part of its tokens.

## R9. Testing, snapshots and fuzzing

- **Decision**:
  - Conformance fixtures in `crates/adapters/gedo-parse/tests/fixtures/rust/` with expected syntax maps as `insta` 1.x snapshots (YAML), reviewed in pull requests. The hashes in those snapshots are the cross-platform reference (User Story 3); the fixtures are checked out with `-text` in `.gitattributes`, so Windows tests the same bytes, and a separate test converts them to CRLF in memory.
  - Formatter corpus: the `xtask format-corpus` command runs `rustfmt` with the configurations of R5 over the fixtures and the workspace and writes the variants under `tests/fixtures/rust/formatted/`, which are committed; tests never spawn `rustfmt`.
  - Property-based tests (`proptest`) in `gedo-ast` for the normalization rules (dropping trivia never changes the hash; any significant token change does; order-insensitive groups are permutation invariant) and in `gedo-parse` for arbitrary bytes (never panics, always within limits).
  - Coverage-guided fuzzing with `cargo-fuzz` in a `fuzz/` directory outside the workspace (it needs nightly), run for 10 minutes on Linux by a manual and release-candidate workflow (SC-007).
  - `criterion` benchmark `parse_rust` over a 2,000-line file (SC-001).
- **Alternatives considered**: hand-written expected files (more work than reviewing `insta` snapshots); fuzzing on every pull request (needs nightly in the required checks and costs minutes; the property tests give the per-PR coverage).

## R10. Incremental parsing

- **Decision**: not in this feature. The `Parser` port takes no previous tree; full parsing fits SC-001 (R1). If the ingest benchmark of feature 003 needs it, incremental parsing is added behind the same port with an adapter-side cache keyed by file path.
