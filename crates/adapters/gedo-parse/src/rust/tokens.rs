//! Neutral classes of Rust tokens (research R4, R5).

use gedo_core::syntax::TokenClass;
use tree_sitter::Node;

use super::kinds;

/// Strict and reserved keywords, including those the grammar exposes as named leaves (`self`,
/// `crate`, `mut`).
const KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "raw",
    "ref",
    "return",
    "safe",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "union",
    "unsafe",
    "use",
    "where",
    "while",
    "yield",
    "macro_rules",
    "default",
];

/// The class of a leaf or atomic node under `parent`. `optional_brace` says whether a `{` or `}`
/// belongs to a block that a formatter may remove.
pub fn classify(node: Node<'_>, parent: Option<Node<'_>>, text: &[u8], optional_brace: bool) -> TokenClass {
    match node.kind() {
        "line_comment" | "block_comment" => {
            return if node.child_by_field_name("doc").is_some() {
                TokenClass::DocComment
            } else {
                TokenClass::Comment
            };
        }
        "string_literal" | "raw_string_literal" | "char_literal" | "integer_literal" | "float_literal"
        | "boolean_literal" => return TokenClass::Literal,
        "lifetime" | "label" => return TokenClass::Identifier,
        _ => {}
    }
    let parent_kind = parent.map(|parent| parent.kind()).unwrap_or_default();
    match text {
        b"{" | b"}" if optional_brace => TokenClass::OptionalPunctuation,
        b"(" | b"[" | b"{" => TokenClass::OpenDelimiter,
        b")" | b"]" | b"}" => TokenClass::CloseDelimiter,
        b"<" if is_generic_list(parent_kind) => TokenClass::OpenDelimiter,
        b">" if is_generic_list(parent_kind) => TokenClass::CloseDelimiter,
        // A comma that ends its list without a closing delimiter after it (`where` clauses), or
        // that separates match arms.
        b"," if parent_kind == "match_arm" || node.next_sibling().is_none() => {
            TokenClass::OptionalPunctuation
        }
        b";" if parent.is_some_and(kinds::is_jump_statement) => TokenClass::OptionalPunctuation,
        b"," => TokenClass::Separator,
        _ if is_keyword(text) => TokenClass::Keyword,
        [first, ..] if first.is_ascii_alphanumeric() || *first == b'_' || !first.is_ascii() => {
            TokenClass::Identifier
        }
        _ => TokenClass::Punctuation,
    }
}

fn is_generic_list(parent_kind: &str) -> bool {
    matches!(parent_kind, "type_parameters" | "type_arguments")
}

fn is_keyword(text: &[u8]) -> bool {
    KEYWORDS.iter().any(|keyword| keyword.as_bytes() == text)
}
