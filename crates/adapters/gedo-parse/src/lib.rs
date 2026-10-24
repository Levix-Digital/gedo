//! Gedo's parsing adapter: implements the core's [`Parser`] port with Tree-sitter.
//!
//! A file becomes a [`ParsedFile`]: every token with its neutral class and every code entity with
//! its kind, qualified path and body. `gedo_ast::normalize` turns that into hashed entities.
//! Rust is the supported language (research R7 of `specs/002-syntax-model`).
#![forbid(unsafe_code)]

mod rust;
mod walk;

use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use gedo_core::ports::{ParseError, Parser};
use gedo_core::syntax::{Language, ParsedFile};

/// Largest file accepted: generated files above it are not hand-edited code.
pub const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;
/// Deepest syntax tree accepted. The walk is iterative, so this bounds memory, not the stack; long
/// or-pattern chains in generated tables reach about 750 levels.
pub const MAX_NESTING_DEPTH: u32 = 4_096;
/// Longest a single parse may take.
pub const PARSE_TIMEOUT_MS: u64 = 2_000;

const UTF8_BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

/// The Tree-sitter parser. It holds no state: each call builds its own Tree-sitter parser, so one
/// value can be shared between threads.
#[derive(Clone, Copy, Debug, Default)]
pub struct TreeSitterParser;

impl TreeSitterParser {
    /// A parser for every supported language.
    pub fn new() -> Self {
        Self
    }
}

impl Parser for TreeSitterParser {
    fn language_of(&self, path: &str) -> Option<Language> {
        path.ends_with(".rs").then_some(Language::Rust)
    }

    fn parse(&self, path: &str, source: &[u8]) -> Result<ParsedFile, ParseError> {
        let language = self.language_of(path).ok_or_else(|| {
            tracing::debug!(path, "file refused: no supported language");
            ParseError::UnsupportedLanguage {
                path: path.to_owned(),
            }
        })?;
        let result = parse_supported(language, path, source);
        match &result {
            Ok(parsed) => {
                let errors = parsed.entities.iter().filter(|entity| entity.has_error).count();
                if errors > 0 {
                    tracing::debug!(
                        path,
                        entities_with_errors = errors,
                        "file parsed with syntax errors"
                    );
                }
            }
            Err(error) => tracing::warn!(path, %error, "file refused"),
        }
        result
    }
}

fn parse_supported(language: Language, path: &str, source: &[u8]) -> Result<ParsedFile, ParseError> {
    let bytes = source.len() as u64;
    if bytes > MAX_SOURCE_BYTES {
        return Err(ParseError::TooLarge {
            path: path.to_owned(),
            bytes,
            limit: MAX_SOURCE_BYTES,
        });
    }
    // The byte order mark is not code; ranges stay relative to the original bytes.
    let offset = if source.starts_with(UTF8_BYTE_ORDER_MARK) {
        UTF8_BYTE_ORDER_MARK.len()
    } else {
        0
    };
    let code = &source[offset..];
    let tree = parse_tree(language, path, code)?;
    let (tokens, entities) = walk::collect(&tree, code, offset as u32, path)?;
    Ok(ParsedFile {
        language,
        path: path.to_owned(),
        tokens,
        entities,
        has_error: tree.root_node().has_error(),
    })
}

fn parse_tree(language: Language, path: &str, code: &[u8]) -> Result<tree_sitter::Tree, ParseError> {
    let mut parser = tree_sitter::Parser::new();
    let grammar = match language {
        Language::Rust => tree_sitter_rust::LANGUAGE,
    };
    parser
        .set_language(&grammar.into())
        .map_err(|error| ParseError::Internal {
            path: path.to_owned(),
            detail: format!("grammar rejected by Tree-sitter: {error}"),
        })?;
    let deadline = Instant::now() + Duration::from_millis(PARSE_TIMEOUT_MS);
    let mut timed_out = false;
    let mut progress = |_: &tree_sitter::ParseState| {
        if Instant::now() < deadline {
            ControlFlow::Continue(())
        } else {
            timed_out = true;
            ControlFlow::Break(())
        }
    };
    let options = tree_sitter::ParseOptions::new().progress_callback(&mut progress);
    let mut read = |offset: usize, _| code.get(offset..).unwrap_or_default();
    let tree = parser.parse_with_options(&mut read, None, Some(options));
    match tree {
        Some(tree) => Ok(tree),
        None if timed_out => Err(ParseError::Timeout {
            path: path.to_owned(),
            limit_ms: PARSE_TIMEOUT_MS,
        }),
        None => Err(ParseError::Internal {
            path: path.to_owned(),
            detail: "Tree-sitter returned no tree".to_owned(),
        }),
    }
}
