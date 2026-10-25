//! The language-neutral model of a parsed source file: its tokens and its code entities.
//!
//! A [`Parser`](crate::ports::Parser) turns a file into a [`ParsedFile`]; `gedo-ast` normalizes it
//! into a [`SyntaxMap`] whose entities carry structural hashes.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A language Gedo can parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Language {
    /// Rust (`.rs`).
    Rust,
}

impl Language {
    /// The language's lowercase name.
    pub fn name(self) -> &'static str {
        match self {
            Language::Rust => "rust",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The language-neutral category of a code entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EntityKind {
    /// A module or namespace.
    Module,
    /// A free function.
    Function,
    /// A function that belongs to a type or a trait.
    Method,
    /// A struct or record type.
    Struct,
    /// An enumeration.
    Enum,
    /// An untagged union.
    Union,
    /// A trait or interface.
    Trait,
    /// An implementation block.
    Impl,
    /// A constant.
    Constant,
    /// A static variable.
    Static,
    /// A type alias.
    TypeAlias,
    /// A constant that belongs to a type or a trait.
    AssociatedConstant,
    /// A type that belongs to a trait or its implementation.
    AssociatedType,
    /// A macro definition.
    Macro,
    /// An import declaration. Its order within its container carries no meaning; normalization
    /// uses it and never returns it in a [`SyntaxMap`].
    Import,
}

impl EntityKind {
    /// The kind's name in snake case.
    pub fn name(self) -> &'static str {
        match self {
            EntityKind::Module => "module",
            EntityKind::Function => "function",
            EntityKind::Method => "method",
            EntityKind::Struct => "struct",
            EntityKind::Enum => "enum",
            EntityKind::Union => "union",
            EntityKind::Trait => "trait",
            EntityKind::Impl => "impl",
            EntityKind::Constant => "constant",
            EntityKind::Static => "static",
            EntityKind::TypeAlias => "type_alias",
            EntityKind::AssociatedConstant => "associated_constant",
            EntityKind::AssociatedType => "associated_type",
            EntityKind::Macro => "macro",
            EntityKind::Import => "import",
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The language-neutral class of a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TokenClass {
    /// A name: of a variable, a type, a lifetime, a primitive type.
    Identifier,
    /// A reserved word.
    Keyword,
    /// A literal value, as one token even when it spans lines.
    Literal,
    /// An operator or other punctuation.
    Punctuation,
    /// `(`, `[`, `{` and the language's other opening delimiters.
    OpenDelimiter,
    /// `)`, `]`, `}` and the language's other closing delimiters.
    CloseDelimiter,
    /// A list separator (`,`); dropped when it directly precedes a closing delimiter.
    Separator,
    /// Punctuation the language lets a formatter add or remove without changing the meaning.
    OptionalPunctuation,
    /// A comment.
    Comment,
    /// A documentation comment.
    DocComment,
}

impl TokenClass {
    /// Whether the token carries meaning after normalization.
    pub fn is_significant(self) -> bool {
        !matches!(
            self,
            TokenClass::Comment | TokenClass::DocComment | TokenClass::OptionalPunctuation
        )
    }

    /// One byte that identifies the class in the hash encoding. Never change a value: it would
    /// change every hash (a new `HashFormat` is needed).
    pub fn code(self) -> u8 {
        match self {
            TokenClass::Identifier => 1,
            TokenClass::Keyword => 2,
            TokenClass::Literal => 3,
            TokenClass::Punctuation => 4,
            TokenClass::OpenDelimiter => 5,
            TokenClass::CloseDelimiter => 6,
            TokenClass::Separator => 7,
            TokenClass::OptionalPunctuation => 8,
            TokenClass::Comment => 9,
            TokenClass::DocComment => 10,
        }
    }
}

/// A half-open byte range `[start, end)` in the original source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ByteRange {
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
}

impl ByteRange {
    /// The range `[start, end)`.
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// The range as `usize` bounds, for slicing the source.
    pub fn as_usize(self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// One leaf of a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// The token's class.
    pub class: TokenClass,
    /// Where it is in the original bytes.
    pub byte_range: ByteRange,
}

/// A half-open range `[start, end)` of token indices in a [`ParsedFile`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TokenRange {
    /// First token.
    pub start: u32,
    /// One past the last token.
    pub end: u32,
}

/// An entity as the parser found it, before normalization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawEntity {
    /// The neutral kind.
    pub kind: EntityKind,
    /// The grammar's own name for it (`function_item`), for display.
    pub language_kind: String,
    /// The entity's own name (`total`), or empty for an unnamed entity.
    pub name: String,
    /// The language's full name of the entity (`crate::billing::Invoice::total`).
    pub qualified_path: String,
    /// Position among the entities of the file with the same kind and qualified path.
    pub occurrence: u32,
    /// The entity's bytes, attributes and documentation included.
    pub byte_range: ByteRange,
    /// Index of the enclosing entity in [`ParsedFile::entities`]; `None` at file level.
    pub container: Option<u32>,
    /// The entity's tokens.
    pub tokens: TokenRange,
    /// Index of the token where the body starts; `None` for an entity without a body.
    pub body_start: Option<u32>,
    /// Whether the entity's tokens contain a syntax error.
    pub has_error: bool,
}

/// What a parser returns for one version of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFile {
    /// The file's language.
    pub language: Language,
    /// The file's path in the repository, `/`-separated.
    pub path: String,
    /// The repository path of the file's compilation unit (a crate's source directory), which
    /// tells apart equal qualified paths of different crates. Not hashed.
    pub unit: String,
    /// Every token, in source order.
    pub tokens: Vec<Token>,
    /// Every entity, in source order; containers come before their children.
    pub entities: Vec<RawEntity>,
    /// Whether the file contains a syntax error anywhere, inside an entity or between them.
    pub has_error: bool,
}

/// The version of the normalization and hash rules that produced a hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HashFormat {
    /// The first rules (`specs/002-syntax-model/research.md`, R4 to R6).
    V1,
}

impl HashFormat {
    /// The format's number.
    pub fn number(self) -> u16 {
        match self {
            HashFormat::V1 => 1,
        }
    }
}

/// A 32-byte BLAKE3 hash of normalized tokens.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StructuralHash(pub [u8; 32]);

impl fmt::Display for StructuralHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for StructuralHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StructuralHash({self})")
    }
}

/// A significant token after normalization.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NormalizedToken {
    /// The token's class.
    pub class: TokenClass,
    /// Its text, with line endings normalized to LF.
    pub text: String,
}

/// One named element of the code, with its hashes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeEntity {
    /// The neutral kind (never [`EntityKind::Import`]).
    pub kind: EntityKind,
    /// The grammar's own name for it.
    pub language_kind: String,
    /// The entity's own name.
    pub name: String,
    /// The language's full name of the entity.
    pub qualified_path: String,
    /// Position among the entities of the file with the same kind and qualified path.
    pub occurrence: u32,
    /// The entity's bytes, attributes and documentation included.
    pub byte_range: ByteRange,
    /// Index of the enclosing entity in [`SyntaxMap::entities`]; `None` at file level.
    pub container: Option<u32>,
    /// Whether the entity contains a syntax error.
    pub has_error: bool,
    /// The significant tokens before the body, nested entities excluded.
    pub signature_tokens: Vec<NormalizedToken>,
    /// The significant tokens of the body, nested entities excluded.
    pub body_tokens: Vec<NormalizedToken>,
    /// Hash of the whole entity, nested entities included.
    pub structural_hash: StructuralHash,
    /// Hash of the signature.
    pub signature_hash: StructuralHash,
    /// Hash of the body, nested entities included.
    pub body_hash: StructuralHash,
}

/// The code entities of one version of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxMap {
    /// The file's language.
    pub language: Language,
    /// The file's path in the repository.
    pub path: String,
    /// The file's compilation unit, as in [`ParsedFile::unit`].
    pub unit: String,
    /// The rules that produced the hashes.
    pub hash_format: HashFormat,
    /// The entities, in source order.
    pub entities: Vec<CodeEntity>,
    /// Whether the file contains a syntax error anywhere.
    pub has_error: bool,
}
