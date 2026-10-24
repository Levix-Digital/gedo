# Contract: Parser port and normalization

## `Parser` (port in `gedo-core`, implemented by `gedo-parse`)

```rust
pub trait Parser {
    /// The language of `path`, or `None` when no supported language matches.
    fn language_of(&self, path: &str) -> Option<Language>;

    /// Parses one version of one file. Syntax errors never fail the call.
    fn parse(&self, path: &str, source: &[u8]) -> Result<ParsedFile, ParseError>;
}
```

Guarantees of every implementation:

1. `parse` returns `UnsupportedLanguage` exactly when `language_of(path)` is `None`.
2. The result is a pure function of `(path, source)` and the adapter version: same input, same output, on every platform and thread.
3. Tokens cover every leaf of the source in order, never overlap, and their byte ranges index the original bytes (a byte order mark is not a token).
4. Each `RawEntity`'s token range starts at its first token (attributes and doc comments attached to it included) and ends at its last; nested entities' ranges are inside their container's.
5. `has_error` is set on an entity if and only if its token range contains a syntax error.
6. Limits are enforced before any work proportional to the input: `TooLarge` before parsing, `TooDeep` and `Timeout` without unbounded recursion or time.
7. Never panics on any input.

The adapter is tested against these guarantees with the conformance suite and property-based tests.

## `normalize` (in `gedo-ast`)

```rust
pub fn normalize(file: &ParsedFile, source: &[u8]) -> Result<SyntaxMap, ParseError>;
```

1. Validates the `ParsedFile` structure (data model) and returns `Internal` on a violation.
2. Drops `Comment` and `DocComment` tokens, normalizes line endings in token text and applies the formatter rules of research R5.
3. Computes the three hashes of each entity with `HashFormat::V1` (research R6) and removes `Import` entities from the result.
4. Pure and deterministic; no I/O.

## Snapshot format (conformance suite)

One YAML snapshot per fixture: `language`, `hash_format` and, per entity, `kind`, `language_kind`, `qualified_path`, `occurrence`, `byte_range`, `container`, `has_error` and the three hashes in lowercase hex. The snapshot is the cross-platform reference of User Story 3.
