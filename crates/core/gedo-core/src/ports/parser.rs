//! Port to a source parser: turns a file into the core's neutral syntax model.

use crate::syntax::{Language, ParsedFile};

/// Why a file could not be parsed. Syntax errors are never a `ParseError`: they flag the
/// entities that contain them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// No supported language matches the file.
    #[error("{path}: no supported language")]
    UnsupportedLanguage {
        /// The file.
        path: String,
    },

    /// The file is larger than the parser accepts.
    #[error("{path}: {bytes} bytes is over the limit of {limit} bytes")]
    TooLarge {
        /// The file.
        path: String,
        /// Its size.
        bytes: u64,
        /// The limit.
        limit: u64,
    },

    /// The file nests deeper than the parser accepts.
    #[error("{path}: nesting deeper than {limit} levels")]
    TooDeep {
        /// The file.
        path: String,
        /// The limit.
        limit: u32,
    },

    /// Parsing took longer than the parser allows.
    #[error("{path}: parsing took longer than {limit_ms} ms")]
    Timeout {
        /// The file.
        path: String,
        /// The limit.
        limit_ms: u64,
    },

    /// The parser produced a structure that breaks the model's rules (a parser bug).
    #[error("{path}: internal parser error: {detail}")]
    Internal {
        /// The file.
        path: String,
        /// What was wrong.
        detail: String,
    },
}

/// Turns source files into [`ParsedFile`]s.
///
/// The result is a pure function of the path and the bytes: the same input gives the same output
/// on every platform and thread (`specs/002-syntax-model/contracts/parser-port.md`).
pub trait Parser {
    /// The language of the file at `path`, or `None` when no supported language matches.
    fn language_of(&self, path: &str) -> Option<Language>;

    /// Parses one version of one file.
    fn parse(&self, path: &str, source: &[u8]) -> Result<ParsedFile, ParseError>;
}
