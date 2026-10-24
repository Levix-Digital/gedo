use std::collections::BTreeMap;

use crate::ports::{ParseError, Parser};
use crate::syntax::{Language, ParsedFile};

/// A parser that returns prepared results by path, for tests of code that uses the port.
#[derive(Debug, Default)]
pub struct FixedParser {
    files: BTreeMap<String, ParsedFile>,
}

impl FixedParser {
    /// An empty parser: every path is unsupported.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `parsed` for its path from now on, whatever the bytes.
    pub fn insert(&mut self, parsed: ParsedFile) {
        self.files.insert(parsed.path.clone(), parsed);
    }
}

impl Parser for FixedParser {
    fn language_of(&self, path: &str) -> Option<Language> {
        self.files.get(path).map(|parsed| parsed.language)
    }

    fn parse(&self, path: &str, _source: &[u8]) -> Result<ParsedFile, ParseError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| ParseError::UnsupportedLanguage {
                path: path.to_owned(),
            })
    }
}
