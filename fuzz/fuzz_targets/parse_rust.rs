//! Any bytes, parsed and normalized as a Rust file: no panic, hang or out-of-memory, and every
//! accepted file normalizes.
#![no_main]

use gedo_core::ports::Parser;
use gedo_parse::TreeSitterParser;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|source: &[u8]| {
    if let Ok(parsed) = TreeSitterParser::new().parse("src/lib.rs", source) {
        gedo_ast::normalize(&parsed, source).expect("a parsed file always normalizes");
    }
});
