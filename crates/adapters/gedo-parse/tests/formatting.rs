//! Formatting invariance (User Story 2, SC-002): every entity keeps its three hashes when a file
//! is reformatted, its line endings are converted or its comments are removed.

mod common;

use std::path::Path;

use gedo_core::ports::Parser;
use gedo_core::syntax::{SyntaxMap, TokenClass};
use gedo_parse::TreeSitterParser;

/// (kind, qualified path, occurrence) with the three hashes, for comparing two versions.
fn fingerprint(map: &SyntaxMap) -> Vec<String> {
    map.entities
        .iter()
        .map(|entity| {
            format!(
                "{} {} #{} {} {} {}",
                entity.kind,
                entity.qualified_path,
                entity.occurrence,
                entity.structural_hash,
                entity.signature_hash,
                entity.body_hash
            )
        })
        .collect()
}

fn corpus_files(variant: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut directories = vec![variant.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory).expect("corpus directory") {
            let path = entry.expect("corpus entry").path();
            if path.is_dir() {
                directories.push(path);
            } else {
                let relative = path.strip_prefix(variant).expect("below the variant");
                let repository_path = relative.to_string_lossy().replace('\\', "/");
                files.push((repository_path, std::fs::read(&path).expect("corpus file")));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn formatter_variants_keep_every_hash() {
    let corpus = common::fixture_root().join("formatted");
    let baseline = corpus_files(&corpus.join("default"));
    assert!(baseline.len() >= 9, "regenerate with `cargo xtask format-corpus`");
    let mut compared = 0;
    for variant in ["narrow", "wide", "tabs"] {
        for (path, source) in &baseline {
            let reformatted = std::fs::read(corpus.join(variant).join(path)).expect("variant exists");
            let expected = fingerprint(&common::syntax_map(path, source));
            let actual = fingerprint(&common::syntax_map(path, &reformatted));
            assert_eq!(expected, actual, "{variant}/{path}");
            compared += expected.len();
        }
    }
    assert!(compared > 300, "entities compared: {compared}");
}

#[test]
fn line_endings_keep_every_hash() {
    for (path, source) in common::fixtures() {
        let text = String::from_utf8(source.clone()).expect("fixtures are UTF-8");
        let crlf = text.replace("\r\n", "\n").replace('\n', "\r\n");
        let lf = text.replace("\r\n", "\n");
        let expected = fingerprint(&common::syntax_map(&path, lf.as_bytes()));
        assert_eq!(
            expected,
            fingerprint(&common::syntax_map(&path, crlf.as_bytes())),
            "{path}"
        );
    }
}

#[test]
fn removing_comments_keeps_every_hash() {
    for (path, source) in common::fixtures() {
        let parsed = TreeSitterParser::new().parse(&path, &source).expect("parses");
        let mut without_comments = source.clone();
        // Blank the comments out, keeping every other byte where it was.
        for token in &parsed.tokens {
            if matches!(token.class, TokenClass::Comment | TokenClass::DocComment) {
                for byte in &mut without_comments[token.byte_range.as_usize()] {
                    if *byte != b'\n' && *byte != b'\r' {
                        *byte = b' ';
                    }
                }
            }
        }
        assert_eq!(
            fingerprint(&common::syntax_map(&path, &source)),
            fingerprint(&common::syntax_map(&path, &without_comments)),
            "{path}"
        );
    }
}
