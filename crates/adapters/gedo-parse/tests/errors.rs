//! Error tolerance and limits (User Story 4, SC-005): a syntax error stays local to the entities
//! that contain it, and no input crashes, hangs or exceeds the limits.

mod common;

use std::collections::HashMap;

use gedo_core::ports::{ParseError, Parser};
use gedo_core::syntax::{StructuralHash, SyntaxMap};
use gedo_parse::{MAX_NESTING_DEPTH, MAX_SOURCE_BYTES, TreeSitterParser};

/// SC-005: share of the entities outside the damaged region that keep kind, path and hashes.
const MIN_PRESERVED_SHARE: f64 = 0.95;
const TRUNCATION_STEPS: usize = 20;

type Key = (String, String, u32);

fn by_key(map: &SyntaxMap) -> HashMap<Key, (StructuralHash, StructuralHash, StructuralHash)> {
    map.entities
        .iter()
        .map(|entity| {
            (
                (
                    entity.kind.to_string(),
                    entity.qualified_path.clone(),
                    entity.occurrence,
                ),
                (entity.structural_hash, entity.signature_hash, entity.body_hash),
            )
        })
        .collect()
}

/// Counts the entities of `original` that do not contain any byte of `damaged_range` and are
/// found unchanged in `damaged`. Returns (preserved, considered).
fn preserved(
    original: &SyntaxMap,
    damaged: &SyntaxMap,
    damaged_range: std::ops::Range<usize>,
) -> (usize, usize) {
    let after = by_key(damaged);
    let mut counts = (0, 0);
    for entity in &original.entities {
        let range = entity.byte_range.as_usize();
        if range.start < damaged_range.end && damaged_range.start < range.end {
            continue;
        }
        counts.1 += 1;
        let key = (
            entity.kind.to_string(),
            entity.qualified_path.clone(),
            entity.occurrence,
        );
        if after.get(&key) == Some(&(entity.structural_hash, entity.signature_hash, entity.body_hash)) {
            counts.0 += 1;
        }
    }
    counts
}

fn parse(path: &str, source: &[u8]) -> SyntaxMap {
    let parsed = TreeSitterParser::new()
        .parse(path, source)
        .expect("damaged code still parses");
    gedo_ast::normalize(&parsed, source).expect("normalizes")
}

#[test]
fn deleted_delimiters_stay_local() {
    let (mut kept, mut considered, mut flagged) = (0, 0, 0);
    for (path, source) in common::fixtures() {
        let original = parse(&path, &source);
        for (position, _) in source
            .iter()
            .enumerate()
            .filter(|(_, byte)| b"{};(".contains(byte))
        {
            let mut damaged_source = source.clone();
            damaged_source.remove(position);
            let damaged = parse(&path, &damaged_source);
            flagged += usize::from(damaged.entities.iter().any(|entity| entity.has_error));
            let region = if b"{}".contains(&source[position]) {
                // An unbalanced brace decides which block every later item belongs to, up to the
                // end of the file (research R8).
                position..source.len()
            } else {
                position..position + 1
            };
            let (preserved_here, considered_here) = preserved(&original, &damaged, region);
            kept += preserved_here;
            considered += considered_here;
        }
    }
    let share = kept as f64 / considered as f64;
    assert!(
        share >= MIN_PRESERVED_SHARE,
        "preserved {kept} of {considered} ({share:.3})"
    );
    assert!(flagged > 0, "damaged entities are flagged");
}

#[test]
fn truncated_files_keep_the_entities_before_the_cut() {
    let (mut kept, mut considered) = (0, 0);
    for (path, source) in common::fixtures() {
        let original = parse(&path, &source);
        for step in 1..TRUNCATION_STEPS {
            let cut = source.len() * step / TRUNCATION_STEPS;
            let damaged = parse(&path, &source[..cut]);
            let (preserved_here, considered_here) = preserved(&original, &damaged, cut..usize::MAX);
            kept += preserved_here;
            considered += considered_here;
        }
    }
    let share = kept as f64 / considered as f64;
    assert!(
        share >= MIN_PRESERVED_SHARE,
        "preserved {kept} of {considered} ({share:.3})"
    );
}

#[test]
fn stray_tokens_stay_local() {
    let (mut kept, mut considered) = (0, 0);
    for (path, source) in common::fixtures() {
        let original = parse(&path, &source);
        for (position, _) in source.iter().enumerate().filter(|(_, byte)| **byte == b'\n') {
            let mut damaged_source = source.clone();
            damaged_source.splice(position..position, b"@@".iter().copied());
            let damaged = parse(&path, &damaged_source);
            let (preserved_here, considered_here) = preserved(&original, &damaged, position..position + 1);
            kept += preserved_here;
            considered += considered_here;
        }
    }
    let share = kept as f64 / considered as f64;
    assert!(
        share >= MIN_PRESERVED_SHARE,
        "preserved {kept} of {considered} ({share:.3})"
    );
}

#[test]
fn a_syntax_error_flags_the_entities_that_contain_it() {
    let source = b"fn good() {}\nfn bad() { let x = ; }\nfn also_good() {}\n";
    let map = parse("src/lib.rs", source);
    let flags: Vec<(&str, bool)> = map
        .entities
        .iter()
        .map(|entity| (entity.name.as_str(), entity.has_error))
        .collect();
    assert_eq!(flags, [("good", false), ("bad", true), ("also_good", false)]);
}

#[test]
fn unusual_inputs_never_fail() {
    let mut binary: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    binary.extend_from_slice(b"\xff\xfe\x00fn \xc3\x28() {}");
    for source in [
        &b""[..],
        b"\n\n",
        b"fn",
        b"}}}}",
        b"/* unterminated",
        b"\"unterminated",
        &binary,
    ] {
        parse("src/lib.rs", source);
    }
}

#[test]
fn files_over_the_size_limit_are_refused() {
    let source = vec![b' '; MAX_SOURCE_BYTES as usize + 1];
    assert!(matches!(
        TreeSitterParser::new().parse("src/lib.rs", &source),
        Err(ParseError::TooLarge { bytes, limit, .. }) if bytes == MAX_SOURCE_BYTES + 1 && limit == MAX_SOURCE_BYTES
    ));
}

#[test]
fn trees_over_the_depth_limit_are_refused() {
    let depth = MAX_NESTING_DEPTH as usize;
    let source = format!("const X: u8 = {}1{};", "(".repeat(depth), ")".repeat(depth));
    assert!(matches!(
        TreeSitterParser::new().parse("src/lib.rs", source.as_bytes()),
        Err(ParseError::TooDeep { limit, .. }) if limit == MAX_NESTING_DEPTH
    ));
    let shallow = format!(
        "const X: u8 = {}1{};",
        "(".repeat(depth / 4),
        ")".repeat(depth / 4)
    );
    assert!(
        TreeSitterParser::new()
            .parse("src/lib.rs", shallow.as_bytes())
            .is_ok()
    );
}
