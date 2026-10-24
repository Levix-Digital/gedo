//! Sensitivity to meaning (SC-003): changing one significant token changes the hash of its entity
//! and of the entity's containers, and no other hash.

mod common;

use gedo_core::syntax::SyntaxMap;

/// (what changes, text to replace in `src/lib.rs`, replacement, qualified path of the entity
/// that contains the change).
const CHANGES: &[(&str, &str, &str, &str)] = &[
    (
        "identifier",
        "helper(self.total)",
        "helper(self.lines.len() as u64)",
        "crate::Invoice::total",
    ),
    (
        "keyword",
        "pub fn new()",
        "pub(crate) fn new()",
        "crate::Invoice::new",
    ),
    (
        "integer literal",
        "LIMIT: usize = 16",
        "LIMIT: usize = 17",
        "crate::LIMIT",
    ),
    (
        "string literal",
        "String::from(\"priced\")",
        "String::from(\"Priced\")",
        "crate::Priced::describe",
    ),
    ("character literal", "'x'", "'y'", "crate::inline::r#type"),
    ("operator", "n + 1", "n - 1", "crate::inline::nested::deep"),
    (
        "type",
        "pub type Map = HashMap<String, u32>;",
        "pub type Map = HashMap<String, u64>;",
        "crate::Map",
    ),
    (
        "attribute",
        "#[derive(Debug, Clone)]",
        "#[derive(Debug)]",
        "crate::Invoice",
    ),
    (
        "delimiter",
        "Self { items: items.to_vec() }",
        "Self { items: (items.to_vec()) }",
        "crate::<Stack<T> as From<&'a[T]>>::from",
    ),
    (
        "separator",
        "lines: Vec<Line>,\n}",
        "lines: Vec<Line>, extra: u8\n}",
        "crate::Invoice",
    ),
    ("enum variant", "Paid(u64),", "Paid(u32),", "crate::Status"),
    ("macro rule", "$x * $x", "$x + $x", "crate::square"),
    (
        "lifetime",
        "fmt::Formatter<'_>",
        "fmt::Formatter<'static>",
        "crate::<Invoice as Display>::fmt",
    ),
    (
        "statement order",
        "let closure = |x: u8| x + 1;\n    closure(callback())",
        "let closure = |x: u8| x + 1;\n    let _ = 0;\n    closure(callback())",
        "crate::later",
    ),
];

fn map(source: &str) -> SyntaxMap {
    common::syntax_map("src/lib.rs", source.as_bytes())
}

#[test]
fn a_change_reaches_the_entity_and_its_containers_only() {
    let original = std::fs::read_to_string(common::fixture_root().join("src/lib.rs")).expect("fixture");
    let before = map(&original);
    for (what, from, to, target) in CHANGES {
        assert_eq!(
            original.matches(from).count(),
            1,
            "{what}: `{from}` must occur once"
        );
        let after = map(&original.replacen(from, to, 1));
        assert_eq!(before.entities.len(), after.entities.len(), "{what}: entities");
        let target_index = before
            .entities
            .iter()
            .position(|entity| entity.qualified_path == *target)
            .unwrap_or_else(|| panic!("{what}: {target} not found"));
        let mut affected = vec![target_index];
        while let Some(container) = before.entities[*affected.last().expect("not empty")].container {
            affected.push(container as usize);
        }
        for (index, (old, new)) in before.entities.iter().zip(&after.entities).enumerate() {
            let changed = old.structural_hash != new.structural_hash;
            assert_eq!(
                changed,
                affected.contains(&index),
                "{what}: {}",
                old.qualified_path
            );
        }
    }
}

#[test]
fn swapping_two_statements_changes_the_hash() {
    let original = "fn f() {\n    a();\n    b();\n}\n";
    let swapped = "fn f() {\n    b();\n    a();\n}\n";
    assert_ne!(
        common::syntax_map("src/lib.rs", original.as_bytes()).entities[0].structural_hash,
        common::syntax_map("src/lib.rs", swapped.as_bytes()).entities[0].structural_hash
    );
}

#[test]
fn a_signature_change_keeps_the_body_hash_and_the_reverse() {
    let original = "fn f(a: u8) -> u8 {\n    a\n}\n";
    let new_signature = "fn f(a: u16) -> u8 {\n    a\n}\n";
    let new_body = "fn f(a: u8) -> u8 {\n    a + 1\n}\n";
    let entity = |source: &str| {
        common::syntax_map("src/lib.rs", source.as_bytes())
            .entities
            .remove(0)
    };
    let (original, signature, body) = (entity(original), entity(new_signature), entity(new_body));
    assert_ne!(original.signature_hash, signature.signature_hash);
    assert_eq!(original.body_hash, signature.body_hash);
    assert_eq!(original.signature_hash, body.signature_hash);
    assert_ne!(original.body_hash, body.body_hash);
}

fn first_hash(source: &str) -> gedo_core::syntax::StructuralHash {
    map(source).entities[0].structural_hash
}

#[test]
fn the_comma_of_a_one_element_tuple_is_significant() {
    for (tuple, parenthesized) in [
        ("fn f() { let x = (a,); }", "fn f() { let x = (a); }"),
        ("type A = (u8,);", "type A = (u8);"),
        ("fn f() { let (a,) = t; }", "fn f() { let (a) = t; }"),
    ] {
        assert_ne!(first_hash(tuple), first_hash(parenthesized), "{tuple}");
    }
}

#[test]
fn the_semicolon_of_a_jump_inside_a_block_is_significant() {
    for (with, without) in [
        ("fn f() { return; g() }", "fn f() { return g() }"),
        ("fn f() { loop { break; x } }", "fn f() { loop { break x } }"),
    ] {
        assert_ne!(first_hash(with), first_hash(without), "{with}");
    }
}
