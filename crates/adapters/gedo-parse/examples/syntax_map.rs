//! Prints the syntax map of a source file: one line per entity with its kind, qualified path,
//! byte range and structural hash.
//!
//! `cargo run -p gedo-parse --example syntax_map -- <file>`

use std::process::ExitCode;

use gedo_core::ports::Parser;
use gedo_parse::TreeSitterParser;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: syntax_map <file>");
        return ExitCode::FAILURE;
    };
    let source = match std::fs::read(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let map = TreeSitterParser::new()
        .parse(&path, &source)
        .and_then(|parsed| gedo_ast::normalize(&parsed, &source));
    let map = match map {
        Ok(map) => map,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    for entity in &map.entities {
        let range = entity.byte_range;
        let error = if entity.has_error { " (syntax error)" } else { "" };
        println!(
            "{:<20} {:<60} {:>6}..{:<6} {}{error}",
            entity.kind.name(),
            entity.qualified_path,
            range.start,
            range.end,
            entity.structural_hash,
        );
    }
    ExitCode::SUCCESS
}
