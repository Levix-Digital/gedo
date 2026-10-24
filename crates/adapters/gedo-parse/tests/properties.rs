//! Arbitrary input never panics and always yields a well-formed result (User Story 4, SC-007).

use gedo_core::ports::Parser;
use gedo_parse::TreeSitterParser;
use proptest::prelude::*;

/// Rust-looking fragments, so the parser reaches deep states more often than with random bytes.
const FRAGMENTS: &[&str] = &[
    "fn ",
    "f",
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    "<",
    ">",
    ";",
    ",",
    ":",
    "::",
    "impl ",
    "for ",
    "struct ",
    "enum ",
    "trait ",
    "mod ",
    "use ",
    "pub ",
    "where ",
    "T",
    "'a",
    "&",
    "mut ",
    "=>",
    "match ",
    "|",
    "#[",
    "\"s\"",
    "r#\"",
    "'c'",
    "1",
    "//",
    "/*",
    "*/",
    "///",
    "\n",
    "\r\n",
    " ",
    "macro_rules! ",
    "$x",
    "unsafe extern \"C\" ",
    "async ",
    "move ",
    "?",
    "!",
    "@",
    "\u{feff}",
    "é",
];

fn check(source: &[u8]) -> Result<(), TestCaseError> {
    // Refusals are allowed; panics and malformed results are not.
    if let Ok(parsed) = TreeSitterParser::new().parse("src/lib.rs", source) {
        let map = gedo_ast::normalize(&parsed, source);
        prop_assert!(map.is_ok(), "{:?}", map.err());
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_bytes_never_panic(source in prop::collection::vec(any::<u8>(), 0..2048)) {
        check(&source)?;
    }

    #[test]
    fn token_soups_never_panic(pieces in prop::collection::vec(prop::sample::select(FRAGMENTS), 0..200)) {
        check(pieces.concat().as_bytes())?;
    }
}
