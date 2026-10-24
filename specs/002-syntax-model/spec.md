# Feature Specification: Syntax Model

**Feature Branch**: `feat/002-syntax-model`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "Syntax model (release v0.2.0, first of two features, backlog items FEAT-009, FEAT-010, FEAT-016): a language-independent syntax tree of code entities and the parser port that produces it from source files; trivia normalization that separates significant syntax from whitespace, line endings, formatting and comments; the structural hash of each entity; and Rust as the first supported language, with its entity kinds, qualified paths and conformance suite. Node matching, identity derivation, semantic deltas, the identity corpus and the indexing of existing history (FEAT-011 to FEAT-015) follow in feature 003."

## Context

v0.2.0 (Node identity) is delivered in two features. This first one turns a source file into the list of **code entities** it contains (functions, types, modules, ...), each with its kind, its qualified path, its location in the file and a **structural hash** that changes when the entity's meaning changes and stays the same when only its formatting or comments change. Feature 003 then matches the entities of two versions of a file, keeps or derives their identities (ADR-0007) and emits semantic deltas.

Like v0.1.0, this feature has no end-user command. Its users are the **identity layer** (feature 003) and the **Levix developers** who build it. Everything that the identity metrics of the vision promise (95% of identities preserved through renames, moves and formatter runs; identical ids on every machine) depends on the hash being stable under formatting and identical across platforms, so those properties are proven here, before matching is built on them.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Turn a Rust file into its code entities (Priority: P1)

The identity layer hands Gedo the bytes of a Rust file and its path in the repository and receives the file's entities: for each, its kind (function, method, struct, enum, trait, implementation, module, constant, static, type alias, macro definition, union), its qualified path (for example `crate::billing::Invoice::total`), its byte range in the file, its lexical container and its structural hash, in source order.

**Why this priority**: every later capability (matching, identity, deltas, trace, the merge driver) works on entities; nothing in v0.2.0 can be built without them.

**Independent Test**: parse a fixture file that contains every supported entity kind, nested modules and several implementation blocks, and compare the returned entities with the expected list.

**Acceptance Scenarios**:

1. **Given** a Rust file with functions, types, traits, implementations and inline modules, **When** it is parsed, **Then** every entity is returned once, in source order, with the expected kind, qualified path, byte range and container.
2. **Given** `src/billing/invoice.rs` in a crate, **When** it is parsed, **Then** the qualified paths of its entities start with `crate::billing::invoice`.
3. **Given** a method inside `impl Invoice`, **When** it is parsed, **Then** its qualified path is `<module>::Invoice::<method>`; inside `impl Display for Invoice`, it is `<module>::<Invoice as Display>::<method>`.
4. **Given** a file of another language or an unknown extension, **When** it is submitted, **Then** Gedo answers "unsupported language" without failing.

---

### User Story 2 - Formatting and comments never change an entity's hash (Priority: P1)

A developer reformats a file (a formatter run, re-indentation, line endings converted by Git on Windows) or edits comments. The structural hash of every entity whose meaning did not change stays exactly the same, so the identity layer recognizes it at once.

**Why this priority**: the exact-match tier of the identity matching (O(1), the common case) relies on it, and the vision's identity accuracy target explicitly covers formatter runs.

**Independent Test**: take real Rust files, reformat them with the formatter under several configurations, convert their line endings, change indentation and rewrite comments, and check that every entity keeps its hash.

**Acceptance Scenarios**:

1. **Given** a file, **When** it is reformatted by the standard Rust formatter (default and non-default configurations), **Then** every entity keeps its hash.
2. **Given** a file with LF line endings, **When** it is converted to CRLF, **Then** every entity keeps its hash and its byte range covers the same text.
3. **Given** an entity, **When** a comment or documentation comment inside or before it is added, edited or removed, **Then** its hash is unchanged.
4. **Given** an entity, **When** a significant token changes (a name, an operator, a literal, a type, an attribute, the order of two statements), **Then** its hash changes.

---

### User Story 3 - The same file gives the same result everywhere (Priority: P1)

The same file parsed on Windows and on Linux, by different builds, in any order and on any number of threads, produces exactly the same entities and hashes.

**Why this priority**: identities are derived from these hashes (ADR-0007); a hash that differs between machines splits the history of an entity.

**Independent Test**: parse a fixed set of files on Windows and Linux in CI and compare the serialized results byte for byte with a committed reference snapshot.

**Acceptance Scenarios**:

1. **Given** the conformance fixtures, **When** they are parsed on Windows and Linux, **Then** the results equal the committed snapshot byte for byte.
2. **Given** a file, **When** it is parsed twice in one process, concurrently, or after other files, **Then** the results are identical.
3. **Given** a change to the normalization or the hash rules, **When** it would change any hash, **Then** it is shipped only under a new hash format version, and results of the old version stay readable and identifiable as such.

---

### User Story 4 - Code that is being edited still yields its entities (Priority: P1)

The daemon (v0.4.0) parses files on every save, often while the code is half-typed. A syntax error in one function must not hide the rest of the file.

**Why this priority**: most saves during active editing contain an error somewhere; losing every entity of the file at those moments would make the identity layer see mass deletions.

**Independent Test**: take valid fixtures, inject syntax errors at many positions (removed braces, truncated files, stray tokens), parse them, and check that entities outside the damaged region keep their kind, path and hash.

**Acceptance Scenarios**:

1. **Given** a file with a syntax error inside one function, **When** it is parsed, **Then** all other entities are returned unchanged and the damaged one is marked as containing errors.
2. **Given** an empty, truncated or binary file, **When** it is parsed, **Then** Gedo returns what it can (possibly nothing) and never crashes or hangs.
3. **Given** a file above the size limit or with nesting deeper than the limit, **When** it is parsed, **Then** Gedo refuses it with a clear "too large" or "too deep" result.

---

### User Story 5 - Rust is declared supported only after its conformance suite passes (Priority: P2)

The Rust support ships with a conformance suite: fixtures for every entity kind and path rule, the formatting invariance checks, error tolerance checks, and a sweep over real-world Rust code (the Gedo workspace and a pinned set of popular open-source crates) that must parse without crashes and with every entity accounted for.

**Why this priority**: the parsing architecture says a language is supported only when its suite passes; the suite is also the template for TypeScript and Python later.

**Independent Test**: run the suite in CI on Windows and Linux; break one mapping rule on purpose and see the suite fail.

**Acceptance Scenarios**:

1. **Given** the conformance suite, **When** it runs in CI, **Then** it passes on Windows and Linux on every pull request.
2. **Given** the real-world sweep, **When** it runs, **Then** no file crashes the parser and every file that the Rust compiler accepts is parsed without errors.

---

### Edge Cases

- Several `impl` blocks for the same type, generic implementations (`impl<T> Stack<T>`), and implementations of the same trait for different types: methods get distinct, stable qualified paths.
- Two entities with the same qualified path in one file (for example a function and a module of the same name, which live in different namespaces, or code under mutually exclusive `#[cfg]` attributes): both are returned, distinguishable by kind and, if needed, by occurrence order.
- Module paths: `lib.rs`, `main.rs`, `mod.rs`, `foo.rs` next to `foo/`, inline `mod` blocks, `#[path]` attributes, `build.rs`, and the separate crate roots in `tests/`, `benches/` and `examples/`.
- Items generated by macro invocations (`lazy_static! { ... }`, derive output) are not entities; macro definitions (`macro_rules!`) are.
- Entities nested in function bodies (an `fn` inside an `fn`): returned with the enclosing function as container.
- A UTF-8 byte order mark, non-ASCII identifiers, raw identifiers (`r#type`), raw strings with embedded line breaks.
- Files that are not valid UTF-8: parsed as far as possible, never crash.
- Very long single lines (minified or generated code) and very large files.
- Formatter options that restructure imports (merging or splitting `use` trees) change the tokens of the enclosing module; they are outside the formatting corpus of SC-002, and the identity layer handles them as an edit of the module.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The core MUST define a language-independent model of a parsed file: an ordered list of entities with kind, name, qualified path, byte range, lexical container, an error flag, and the separate significant token sequences of the entity's signature and body.
- **FR-002**: The core MUST define the parser port: it receives a file's bytes, its path in the repository and its language, and returns the model of FR-001 or a clear refusal (unsupported language, too large, too deep). Parsing MUST never fail because of syntax errors.
- **FR-003**: The language MUST be determined from the file path; an unknown or unsupported language MUST produce an "unsupported language" result, not an error.
- **FR-004**: Normalization MUST separate significant syntax from trivia: whitespace, line breaks, indentation, comments and documentation comments are trivia; identifiers, keywords, operators, literals, attributes and the order of tokens are significant. Line endings MUST be normalized (CRLF and CR to LF), including inside literals, and a leading byte order mark MUST be ignored.
- **FR-005**: Normalization MUST also remove language-specific syntax that a formatter can add or remove without changing meaning; for Rust, trailing commas before a closing delimiter, the separators that the standard formatter adds or removes, and the order of `use` declarations and of out-of-line `mod` declarations within one container, which the formatter sorts and which carries no meaning.
- **FR-006**: The structural hash of an entity MUST be computed from its normalized significant tokens (including those of its nested entities), MUST be 32 bytes, and MUST be domain-separated by a hash format version; the signature and body hashes MUST be available separately.
- **FR-007**: Results MUST be deterministic: the same bytes and path give the same entities and hashes on every supported platform, build, thread count and parse order.
- **FR-008**: Any change that alters a hash for some input MUST introduce a new hash format version; the version MUST be part of every result.
- **FR-009**: Rust MUST be supported with these entity kinds: module, function, method and associated function, struct, enum, union, trait, implementation, constant, static, type alias, associated constant and type, and `macro_rules!` definition. Enum variants and struct fields are part of their parent's hash, not separate entities.
- **FR-010**: Rust qualified paths MUST be derived from the file path and the source: `crate` followed by the module path of the file (from `src/`, honouring `lib.rs`, `main.rs`, `mod.rs` and `#[path]`), the inline modules, and the item names; methods are qualified by their type (`Type::method`) or, in trait implementations, by `<Type as Trait>::method`, with the type and trait written as in the source with whitespace normalized.
- **FR-011**: An entity that contains a syntax error MUST be flagged; entities outside the damaged region MUST keep the kind, path and hash they have in the valid file.
- **FR-012**: Parsing MUST enforce limits on file size and nesting depth, MUST NOT crash, hang or overflow the stack on any input, and the parser MUST be fuzzed.
- **FR-013**: Rust support MUST ship with a conformance suite (entity kinds, path rules, formatting invariance, error tolerance, real-world sweep) that runs in CI on Windows and Linux on every pull request, and a committed cross-platform snapshot of results.
- **FR-014**: Errors (unsupported language, limits exceeded, internal failure) MUST carry context (file path, language, limit) and MUST NOT be swallowed.
- **FR-015**: The core MUST NOT depend on the parsing library; the parsing library MUST be used only by the parsing adapter (ADR-0001, ADR-0002).

### Key Entities

- **Syntax map**: the result of parsing one version of one file: the file's language, the hash format version and its ordered entities. Feature 003 turns it into a `BlobAstMap` by attaching identities.
- **Code entity**: one named element of the code: kind, name, qualified path, byte range, container (another entity or the file), error flag, signature tokens, body tokens and the structural, signature and body hashes.
- **Entity kind**: a language-independent category (module, function, type, trait, implementation, ...) with the language's own kind name kept for display.
- **Qualified path**: the language's full name of an entity within its crate or package, built from the file path and the source.
- **Significant token**: a token that carries meaning after normalization (kind and text), the unit the hashes and the identity layer's similarity work on.
- **Hash format version**: the version of the normalization and hash rules that produced a hash.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Parsing, normalizing and hashing a Rust file of 2,000 lines takes under 10 ms at the 95th percentile on the reference machine, leaving at least half of the 20 ms ingest budget of the technical specification to matching and the log.
- **SC-002**: On the formatting corpus (real files reformatted with at least 3 formatter configurations, line ending conversion and re-indentation), 100% of entities keep their hash.
- **SC-003**: On the change corpus (one significant token changed per case, covering every token category), 100% of the changed entities change their hash.
- **SC-004**: The conformance snapshot is identical byte for byte on Windows and Linux on every pull request.
- **SC-005**: On the error-injection corpus, at least 95% of the entities outside the damaged region keep their kind, path and hash.
- **SC-006**: The real-world sweep (the Gedo workspace and at least 5 pinned popular crates, at least 100,000 lines in total) runs with zero crashes, and zero files accepted by the Rust compiler are reported with syntax errors.
- **SC-007**: At least 10 minutes of fuzzing per platform per release candidate find no crash, hang or stack overflow.

## Assumptions

- No end-user command is delivered; the capabilities are exercised through tests, benchmarks and the conformance suite. `gedo init` and the daemon arrive in v0.4.0.
- Identities (`AstNodeId`), matching, deltas, the identity corpus and history indexing are out of scope; they are feature 003 (FEAT-011 to FEAT-015).
- Full parsing of a file fits the time budget of SC-001; incremental reparsing with the previous tree is an optimization left for later and captured in the backlog if the measurements require it.
- The qualified path starts with `crate`; telling apart crates of a workspace (by their manifest) is the identity layer's concern when it derives ids, since a syntax map is always tied to one file path.
- Items inside macro invocations are not resolved; the invocation is part of its container's tokens.
- Documentation comments are trivia for the hash; their text stays available in the file for display, but changing them is not a change of the entity.
- The real-world sweep uses pinned versions of crates vendored as test data or fetched by a CI step with a checksum; no network access is added to Gedo itself.
- The reference machine of the technical specification (4 cores, SSD) is used for SC-001; CI benchmarks are reported but do not block pull requests.
