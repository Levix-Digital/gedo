//! Gedo's syntax algorithms: a [`ParsedFile`] from the parser port becomes a [`SyntaxMap`] whose
//! entities carry structural hashes that ignore formatting and comments.
//!
//! Feature 003 adds the matching of two versions of the code, the derivation of identities, the
//! semantic deltas and the reconciliation of identities (`specs/003-node-identity/research.md`).
//!
//! Everything here is pure: no I/O, no parsing library (ADR-0001). The rules are those of
//! `specs/002-syntax-model/research.md` (R4 to R6), versioned as [`HashFormat::V1`].
#![forbid(unsafe_code)]

pub mod alias;
mod delta;
mod hash;
pub mod identity;
pub mod matching;
mod normalize;
mod validate;

pub use alias::AliasTable;
pub use identity::{IdentitySeed, derive_id};
pub use matching::{ChangeSet, EntityTokens, MATCH_FORMAT, MatchOutcome, NewFile, OldFile, match_change};

use gedo_core::ports::ParseError;
use gedo_core::syntax::{HashFormat, ParsedFile, SyntaxMap};

/// The hash format this build produces.
pub const HASH_FORMAT: HashFormat = HashFormat::V1;

/// Normalizes a parsed file and hashes its entities.
///
/// `source` is the file the parser read. A `ParsedFile` that breaks the model's structural rules
/// is a parser bug, reported as [`ParseError::Internal`].
pub fn normalize(file: &ParsedFile, source: &[u8]) -> Result<SyntaxMap, ParseError> {
    validate::check(file, source.len()).map_err(|detail| ParseError::Internal {
        path: file.path.clone(),
        detail,
    })?;
    let tokens = normalize::significant_tokens(file, source);
    let entities = hash::hash_entities(file, tokens);
    Ok(SyntaxMap {
        language: file.language,
        path: file.path.clone(),
        unit: file.unit.clone(),
        hash_format: HASH_FORMAT,
        entities,
        has_error: file.has_error,
    })
}
