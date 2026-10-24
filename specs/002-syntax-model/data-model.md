# Data Model: Syntax Model

Types live in `gedo-core` unless noted. None of them is persisted by this feature; feature 003 stores syntax maps inside `BlobAstMap` records.

## Language

- `Rust` (supported). The enum grows one variant per supported language.
- Detected by the adapter from the file extension (`.rs`); anything else is `ParseError::UnsupportedLanguage { path }`.

## EntityKind

Language-neutral category of a code entity: `Module`, `Function`, `Method`, `Struct`, `Enum`, `Union`, `Trait`, `Impl`, `Constant`, `Static`, `TypeAlias`, `AssociatedConstant`, `AssociatedType`, `Macro`, plus `Import`, which exists only between the adapter and normalization (research R5) and never appears in a `SyntaxMap`.

## TokenClass and Token

- `TokenClass`: `Identifier`, `Keyword`, `Literal`, `Punctuation`, `OpenDelimiter`, `CloseDelimiter`, `Separator`, `Comment`, `DocComment`.
- `Token { class, byte_range: (u32, u32) }`: a leaf of the source, located in the original bytes.

## ParsedFile (adapter output, normalization input)

- `language`, `path` (repository-relative, `/`-separated).
- `tokens`: every token of the file in source order.
- `entities`: `RawEntity` list in source order.
- `RawEntity { kind, language_kind, name, qualified_path, occurrence, byte_range, container: Option<usize>, tokens: (start, end), body_start: Option<usize>, has_error }`: `container` is the index of the enclosing entity; `tokens` is the half-open range of token indices; `body_start` is the token index where the body starts (none for entities without a body).

Validation (checked by `gedo-ast` before normalizing; a violation is an adapter bug reported as `ParseError::Internal`): token ranges are nested and inside the file; containers precede their children; byte ranges are within the source length.

## SyntaxMap (normalization output)

- `language`, `path`, `hash_format: HashFormat`.
- `entities: Vec<CodeEntity>`, in source order, without `Import` entities.

## CodeEntity

- `kind: EntityKind`, `language_kind: String` (`function_item`, ...), `name: String`, `qualified_path: String`, `occurrence: u32`.
- `byte_range: (u32, u32)`, `container: Option<usize>` (index in `entities`).
- `has_error: bool`.
- `signature_tokens` and `body_tokens`: the normalized significant tokens as `(TokenClass, String)` pairs, for the similarity of feature 003.
- `structural_hash`, `signature_hash`, `body_hash`: `StructuralHash([u8; 32])`.

Relationship: one `SyntaxMap` per (file path, file content); `AnchoredAstNode` of feature 003 is a `CodeEntity` plus an `AstNodeId`.

## HashFormat

- `V1`: rules of research R4 to R6, derive-key context `gedo structural hash v1`.
- Stored with every syntax map; a reader that does not know the version refuses it (FR-008, as for `RecordEnvelope` formats).

## ParseError

- `UnsupportedLanguage { path }`, `TooLarge { path, bytes, limit }`, `TooDeep { path, limit }`, `Timeout { path, limit_ms }`, `Internal { path, detail }`.
- Syntax errors are never a `ParseError`; they set `has_error` on entities.

## Limits (named constants in `gedo-parse`)

- `MAX_SOURCE_BYTES = 16 MiB`, `MAX_NESTING_DEPTH = 4096`, `PARSE_TIMEOUT_MS = 2_000`.
