//! Helpers shared by the conformance suite and the corpora.
#![allow(dead_code)]

use std::fmt::Write;
use std::path::{Path, PathBuf};

use gedo_core::ports::Parser;
use gedo_core::syntax::SyntaxMap;
use gedo_parse::TreeSitterParser;

/// The root of the Rust fixtures; paths below it are the files' repository paths.
pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust")
}

/// Every `.rs` fixture as (repository path, bytes), sorted by path, the `formatted/` variants
/// excluded.
pub fn fixtures() -> Vec<(String, Vec<u8>)> {
    let root = fixture_root();
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    files
}

fn collect(root: &Path, directory: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .expect("fixture directory is readable")
        .map(|entry| entry.expect("fixture entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        let relative = path.strip_prefix(root).expect("below the root");
        if path.is_dir() {
            if relative != Path::new("formatted") {
                collect(root, &path, files);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let repository_path = relative.to_string_lossy().replace('\\', "/");
            files.push((
                repository_path,
                std::fs::read(&path).expect("fixture is readable"),
            ));
        }
    }
}

/// Parses and normalizes one file.
pub fn syntax_map(path: &str, source: &[u8]) -> SyntaxMap {
    let parsed = TreeSitterParser::new()
        .parse(path, source)
        .expect("fixture parses");
    gedo_ast::normalize(&parsed, source).expect("fixture normalizes")
}

/// The snapshot text of a syntax map (`contracts/parser-port.md`).
pub fn render(map: &SyntaxMap) -> String {
    let mut text = format!(
        "language: {}\nunit: {}\nhash_format: {}\nentities:\n",
        map.language,
        map.unit,
        map.hash_format.number()
    );
    for entity in &map.entities {
        let container = entity.container.map_or("-".to_owned(), |index| index.to_string());
        let _ = write!(
            text,
            "  - kind: {}\n    language_kind: {}\n    qualified_path: {}\n    occurrence: {}\n    byte_range: {}..{}\n    container: {}\n    has_error: {}\n    structural_hash: {}\n    signature_hash: {}\n    body_hash: {}\n",
            entity.kind,
            entity.language_kind,
            entity.qualified_path,
            entity.occurrence,
            entity.byte_range.start,
            entity.byte_range.end,
            container,
            entity.has_error,
            entity.structural_hash,
            entity.signature_hash,
            entity.body_hash,
        );
    }
    text
}
