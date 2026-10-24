# Data Model: Syntax Model

Types live in `gedo-core` unless noted. None of them is persisted by this feature; feature 003 stores syntax maps inside `BlobAstMap` records.

## Language

- `Rust` (supported). The enum grows one variant per supported language.
- Detected by the adapter from the file extension (`.rs`); anything else is `ParseError::UnsupportedLanguage { path }`.

## EntityKind

Language-neutral category of a code entity: `Module`, `Function`, `Method`, `Struct`, `Enum`, `Union`, `Trait`, `Impl`, `Constant`, `Static`, `TypeAlias`, `AssociatedConstant`, `AssociatedType`, `Macro`, plus `Import`, which exists only between the adapter and normalization (research R5) and never appears in a `SyntaxMap`.

## TokenClass and Token

- `TokenClass`: `Identifier`, `Keyword`, `Literal`, `Punctuation`, `OpenDelimiter`, `CloseDelimiter`, `Separator`, `OptionalPunctuation`, `Comment`, `DocComment`, with fixed hash codes 1 to 10 (`code()`). `is_significant()` is false for `OptionalPunctuation`, `Comment` and `DocComment`.
- `ByteRange { start: u32, end: u32 }`: a half-open range of the original bytes.
- `Token { class, byte_range: ByteRange }`: a leaf of the source, located in the original bytes.
- `TokenRange { start: u32, end: u32 }`: a half-open range of token indices.

## ParsedFile (adapter output, normalization input)

- `language`, `path` (repository-relative, `/`-separated).
- `tokens`: every token of the file in source order.
- `entities`: `RawEntity` list in source order.
- `has_error`: whether the file contains a syntax error anywhere, inside an entity or between them.
- `RawEntity { kind, language_kind, name, qualified_path, occurrence, byte_range: ByteRange, container: Option<u32>, tokens: TokenRange, body_start: Option<u32>, has_error }`: `container` is the index of the enclosing entity; `body_start` is the token index where the body starts (none for entities without a body).

Validation (checked by `gedo-ast` before normalizing; a violation is an adapter bug reported as `ParseError::Internal`): token ranges are nested and inside the file; containers precede their children; byte ranges are within the source length.

## SyntaxMap (normalization output)

- `language`, `path`, `hash_format: HashFormat`.
- `entities: Vec<CodeEntity>`, in source order, without `Import` entities.
- `has_error`: copied from the `ParsedFile`.

## CodeEntity

- `kind: EntityKind`, `language_kind: String` (`function_item`, ...), `name: String`, `qualified_path: String`, `occurrence: u32`.
- `byte_range: ByteRange`, `container: Option<u32>` (index in `entities`).
- `has_error: bool`.
- `signature_tokens` and `body_tokens`: `Vec<NormalizedToken { class, text }>`, the significant tokens with line endings normalized, nested entities excluded, for the similarity of feature 003.
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
