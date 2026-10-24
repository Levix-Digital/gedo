---
title: Parsing Adapter
summary: Turns source files into the core's neutral syntax model with Tree-sitter.
code: [crates/adapters/gedo-parse]
---

# Parsing Adapter

Turns source files into the core's neutral syntax model with Tree-sitter.

## Responsibilities

- Implements the `Parser` port of `gedo-core` with Tree-sitter: a file's path and bytes become a `ParsedFile` with every token and every code entity.
- Maps each language's grammar nodes to the core's entity kinds, token classes and qualified paths.
- Never fails on syntax errors: they flag the entities that contain them and the file.
- Holds one grammar crate per supported language and its conformance suite.

## Structure

| Module | Role |
|---|---|
| `lib.rs` | `TreeSitterParser`: language detection by extension, limits, the parse call with its timeout |
| `walk.rs` | One iterative cursor walk that emits tokens and entities, attaches attributes and comments to the item after them, and tracks ancestors (asking Tree-sitter for a parent walks down from the root) |
| `rust/kinds.rs` | Rust grammar node to `EntityKind`, body of each entity, atomic nodes, optional blocks |
| `rust/tokens.rs` | Rust leaf to `TokenClass` |
| `rust/paths.rs` | Module path from the file path, `impl` and method paths |

The walk calls the `rust` module directly; it becomes a per-language interface when a second language arrives.

## Behavior

**Errors**: an entity is flagged when its tokens, attached attributes and comments included, contain a syntax error.

**Rust entities**: modules, functions, methods and associated functions, structs, enums, unions, traits, implementations, constants, statics, type aliases, associated constants and types, and `macro_rules!` definitions. Enum variants and fields stay in their parent. `use` declarations, and each item of a `use` list with several items, are emitted as `Import` entities, which normalization uses for order and never returns. Items inside function bodies are entities whose container is the function.

**Qualified paths** start with `crate` followed by the module path of the file (below the nearest `src/`, else `tests/`, `benches/` or `examples/`; `lib.rs`, `main.rs`, `build.rs`, `src/bin/*` and single files in `tests/`, `benches/`, `examples/` are crate roots; the other files of a `src/bin/<name>/` binary are its modules; `mod.rs` names its directory), the inline modules and the item name. Inherent `impl` blocks are `<module>::<impl Type>` and their items `<module>::Type::item`; trait implementations are `<module>::<Type as Trait>` with their items below. Types and traits are their tokens joined with a space only between two words (`Stack<T>`, `From<&'a[T]>`). Two entities with the same kind and path (`#[cfg]` variants) get an occurrence index. `#[path]` attributes are not followed, because each file is parsed on its own.

**Token classes**: comments and documentation comments are trivia. Literals, lifetimes and labels are one token each. Brackets, parentheses, braces and the angle brackets of generic lists are delimiters; `,` is a separator, except in a one-element tuple, where it is what makes the tuple. The braces and comma of a `use` list with a single item (`std::{io}`) are optional, since `rustfmt` removes them. Punctuation that `rustfmt` adds or removes without changing the meaning is optional: the comma after a match arm or at the end of a `where` clause, the braces of a match arm or closure body that holds a single expression, and the `;` after a `return`, `break` or `continue` that ends its block. The walk knows whether a node is the last child from its cursor; it never asks a node for its next sibling, which would make long lists quadratic.

**Limits**: files over 16 MiB are refused (`TooLarge`), trees deeper than 4,096 levels (`TooDeep`), files whose parse and walk take longer than 2 s (`Timeout`; `TreeSitterParser::with_timeout` sets another limit). A UTF-8 byte order mark is skipped; byte ranges stay on the original bytes.

**Languages** are added in this order: Rust; then TypeScript and Python (the Levix stacks); then about 30 languages ordered by worldwide usage (GitHub Octoverse and Stack Overflow rankings combined with Reciprocal Rank Fusion), limited to mature grammars. A language is declared supported only when its conformance suite passes.

**Rust conformance suite** (`tests/`): fixtures with reviewed `insta` snapshots that are the cross-platform reference; a `rustfmt` corpus (`cargo xtask format-corpus`, committed) where every entity keeps its hashes; one-token changes that reach only the entity and its containers; error injection; arbitrary input; determinism across threads; and `cargo xtask parse-sweep`, which parses the workspace and every locked dependency in CI. Constructs the grammar does not parse are listed in `tests/known_grammar_gaps.toml` and tracked by IMP-001. The `fuzz/` directory holds a `cargo-fuzz` target run by the manual `fuzz` workflow.

## Interfaces

- Implements the `Parser` port of `gedo-core`; the identity layer normalizes its output with `gedo_ast::normalize`.
- `tracing` events: a warning for each refused file, a debug event for each file with syntax errors.

## Decisions

- [ADR-0002](../adr/0002-rust-workspace-with-gitoxide-tree-sitter-blake3-and-postcard.md): Tree-sitter, isolated in this crate.
- `specs/002-syntax-model/research.md` (R1, R2, R4, R5, R7, R8): grammar versions, coverage, token classes, formatter rules, Rust mapping and limits.
