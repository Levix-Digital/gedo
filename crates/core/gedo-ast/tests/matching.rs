//! Matching and deltas over real Rust sources (User Stories 1 and 4).

mod support;

use support::{first, kinds_of, next};

const BILLING: &str = r#"
pub struct Invoice {
    lines: Vec<u64>,
}

impl Invoice {
    pub fn total(&self) -> u64 {
        self.lines.iter().sum()
    }

    pub fn count(&self) -> usize {
        self.lines.len()
    }
}

pub fn tax(amount: u64) -> u64 {
    let rate = 20;
    amount * rate / 100
}
"#;

const TOTAL: &str = "crate::billing::Invoice::total";
const TAX: &str = "crate::billing::tax";
const PATH: &str = "src/billing.rs";

#[test]
fn unchanged_and_reformatted_entities_keep_their_ids_without_deltas() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let reformatted = format!(
        "// a comment above\n\n{}",
        BILLING
            .replace("    pub fn count", "    /// Docs.\n    pub fn count")
            .replace("\n\n", "\n\n\n")
    );
    let (v2, outcome) = next(&v1, &[(PATH, Some(&reformatted))]);
    assert_eq!(outcome.deltas, []);
    for path in [TOTAL, TAX, "crate::billing::Invoice"] {
        assert_eq!(v2.id(PATH, path), v1.id(PATH, path), "{path}");
    }
}

#[test]
fn an_edited_body_keeps_the_id_and_reports_body_modified() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let edited = BILLING.replace("let rate = 20;", "let rate = 25;\n    let unused = 1;");
    let (v2, outcome) = next(&v1, &[(PATH, Some(&edited))]);
    let id = v1.id(PATH, TAX);
    assert_eq!(v2.id(PATH, TAX), id);
    assert_eq!(kinds_of(&outcome, id), ["body"]);
    assert_eq!(outcome.deltas.len(), 1, "{:?}", outcome.deltas);
}

#[test]
fn a_changed_signature_keeps_the_id_and_reports_signature_mutated() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let edited = BILLING.replace(
        "pub fn tax(amount: u64) -> u64",
        "pub(crate) fn tax(amount: u64, rate: u64) -> u64",
    );
    let (v2, outcome) = next(&v1, &[(PATH, Some(&edited))]);
    let id = v1.id(PATH, TAX);
    assert_eq!(v2.id(PATH, TAX), id);
    assert_eq!(kinds_of(&outcome, id), ["signature"]);
}

#[test]
fn a_rename_keeps_the_id_with_or_without_a_body_edit() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let id = v1.id(PATH, TAX);

    let renamed = BILLING.replace("pub fn tax(", "pub fn vat(");
    let (v2, outcome) = next(&v1, &[(PATH, Some(&renamed))]);
    assert_eq!(v2.id(PATH, "crate::billing::vat"), id);
    assert_eq!(kinds_of(&outcome, id), ["renamed"]);

    let edited = renamed.replace("amount * rate / 100", "amount * rate / 100 + 1");
    let (v3, outcome) = next(&v1, &[(PATH, Some(&edited))]);
    assert_eq!(v3.id(PATH, "crate::billing::vat"), id);
    assert_eq!(kinds_of(&outcome, id), ["renamed", "body"]);
}

#[test]
fn a_move_to_another_module_of_the_file_keeps_the_id() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let id = v1.id(PATH, TAX);
    let tax_fn = "pub fn tax(amount: u64) -> u64 {\n    let rate = 20;\n    amount * rate / 100\n}\n";
    let moved = format!("{}\npub mod rates {{\n{tax_fn}}}\n", BILLING.replace(tax_fn, ""));
    let (v2, outcome) = next(&v1, &[(PATH, Some(&moved))]);
    assert_eq!(v2.id(PATH, "crate::billing::rates::tax"), id);
    assert_eq!(kinds_of(&outcome, id), ["moved"]);
}

#[test]
fn a_move_to_another_file_keeps_the_id() {
    let (v1, _) = first(&[(PATH, BILLING), ("src/rates.rs", "pub const BASE: u64 = 1;\n")]);
    let id = v1.id(PATH, TAX);
    let tax_fn = "pub fn tax(amount: u64) -> u64 {\n    let rate = 20;\n    amount * rate / 100\n}\n";
    let rates = format!("pub const BASE: u64 = 1;\n\n{tax_fn}");
    let (v2, outcome) = next(
        &v1,
        &[
            (PATH, Some(&BILLING.replace(tax_fn, ""))),
            ("src/rates.rs", Some(&rates)),
        ],
    );
    assert_eq!(v2.id("src/rates.rs", "crate::rates::tax"), id);
    assert_eq!(kinds_of(&outcome, id), ["moved"]);
    assert_eq!(
        v2.id("src/rates.rs", "crate::rates::BASE"),
        v1.id("src/rates.rs", "crate::rates::BASE")
    );
}

#[test]
fn a_renamed_file_keeps_every_id() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let (v2, outcome) = next(&v1, &[(PATH, None), ("src/invoicing.rs", Some(BILLING))]);
    assert_eq!(
        v2.id("src/invoicing.rs", "crate::invoicing::tax"),
        v1.id(PATH, TAX)
    );
    assert_eq!(
        v2.id("src/invoicing.rs", "crate::invoicing::Invoice::total"),
        v1.id(PATH, TOTAL)
    );
    assert!(
        outcome
            .deltas
            .iter()
            .all(|delta| kinds_of(&outcome, delta.id()) == ["moved"])
    );
}

#[test]
fn an_unrelated_replacement_is_a_tombstone_and_a_creation() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let replaced = BILLING.replace(
        "pub fn tax(amount: u64) -> u64 {\n    let rate = 20;\n    amount * rate / 100\n}",
        "pub fn parse_header(input: &str) -> Option<(String, String)> {\n    let (key, value) = input.split_once(':')?;\n    Some((key.trim().to_owned(), value.trim().to_owned()))\n}",
    );
    let (v2, outcome) = next(&v1, &[(PATH, Some(&replaced))]);
    let old = v1.id(PATH, TAX);
    let new = v2.id(PATH, "crate::billing::parse_header");
    assert_ne!(old, new);
    assert_eq!(kinds_of(&outcome, old), ["tombstoned"]);
    assert_eq!(kinds_of(&outcome, new), ["created"]);
}

#[test]
fn a_copy_keeps_the_original_and_creates_the_copy() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let copy = format!(
        "{BILLING}\npub fn tax_copy(amount: u64) -> u64 {{\n    let rate = 20;\n    amount * rate / 100\n}}\n"
    );
    let (v2, outcome) = next(&v1, &[(PATH, Some(&copy))]);
    assert_eq!(v2.id(PATH, TAX), v1.id(PATH, TAX));
    let copied = v2.id(PATH, "crate::billing::tax_copy");
    assert_eq!(kinds_of(&outcome, copied), ["created"]);
}

#[test]
fn a_method_of_a_renamed_type_follows_its_impl() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let renamed = BILLING.replace("Invoice", "Bill");
    let (v2, _) = next(&v1, &[(PATH, Some(&renamed))]);
    assert_eq!(v2.id(PATH, "crate::billing::Bill::total"), v1.id(PATH, TOTAL));
    assert_eq!(
        v2.id(PATH, "crate::billing::Bill"),
        v1.id(PATH, "crate::billing::Invoice")
    );
}

#[test]
fn a_changed_method_does_not_report_its_impl_as_modified() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let edited = BILLING.replace("self.lines.len()", "self.lines.len() + 0");
    let (_, outcome) = next(&v1, &[(PATH, Some(&edited))]);
    assert_eq!(outcome.deltas.len(), 1, "{:?}", outcome.deltas);
}

#[test]
fn a_version_matched_against_itself_changes_nothing() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let (v2, outcome) = next(&v1, &[(PATH, Some(BILLING))]);
    assert_eq!(outcome.deltas, []);
    assert_eq!(v2.ids(), v1.ids());
}

#[test]
fn a_deleted_file_tombstones_its_entities() {
    let (v1, _) = first(&[(PATH, BILLING), ("src/lib.rs", "pub mod billing;\n")]);
    let (_, outcome) = next(&v1, &[(PATH, None)]);
    assert_eq!(outcome.deltas.len(), v1.files[PATH].0.nodes.len());
    assert!(
        outcome
            .deltas
            .iter()
            .all(|delta| kinds_of(&outcome, delta.id()) == ["tombstoned"])
    );
}

#[test]
fn a_syntax_error_carries_entities_instead_of_tombstoning_them() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    // An unbalanced brace swallows everything after it.
    let damaged = BILLING.replace("self.lines.iter().sum()\n    }", "self.lines.iter().sum()\n");
    let (v2, outcome) = next(&v1, &[(PATH, Some(&damaged))]);
    assert!(
        outcome
            .deltas
            .iter()
            .all(|delta| kinds_of(&outcome, delta.id()) != ["tombstoned"]),
        "{:?}",
        outcome.deltas
    );
    assert_eq!(v2.id(PATH, TOTAL), v1.id(PATH, TOTAL));

    // Fixed again: every entity has its old id back, placed.
    let (v3, outcome) = next(&v2, &[(PATH, Some(BILLING))]);
    for path in [TOTAL, TAX, "crate::billing::Invoice::count"] {
        assert_eq!(v3.id(PATH, path), v1.id(PATH, path), "{path}");
    }
    assert!(
        outcome
            .deltas
            .iter()
            .all(|delta| kinds_of(&outcome, delta.id()) != ["created"])
    );
    assert!(v3.files[PATH].0.nodes.iter().all(|node| !node.is_detached()));
}

#[test]
fn a_re_created_entity_gets_its_old_id_back() {
    let (v1, _) = first(&[(PATH, BILLING)]);
    let without = BILLING.replace(
        "pub fn tax(amount: u64) -> u64 {\n    let rate = 20;\n    amount * rate / 100\n}",
        "",
    );
    let (v2, _) = next(&v1, &[(PATH, Some(&without))]);
    assert!(v2.try_id(PATH, TAX).is_none());
    let (v3, _) = next(&v2, &[(PATH, Some(BILLING))]);
    assert_eq!(v3.id(PATH, TAX), v1.id(PATH, TAX));
}

#[test]
fn identical_creations_in_two_places_derive_the_same_id() {
    let (left, _) = first(&[(PATH, BILLING)]);
    let (right, _) = first(&[(PATH, BILLING)]);
    assert_eq!(left.ids(), right.ids());
}

#[test]
fn equal_paths_in_two_crates_get_different_ids() {
    let (v1, _) = first(&[
        ("crates/a/src/billing.rs", BILLING),
        ("crates/b/src/billing.rs", BILLING),
    ]);
    assert_ne!(
        v1.id("crates/a/src/billing.rs", TAX),
        v1.id("crates/b/src/billing.rs", TAX)
    );
}

#[test]
fn a_name_taken_by_a_live_entity_derives_another_id() {
    // `tax` is renamed to `vat`, and a new `tax` with the old content appears in another file.
    let (v1, _) = first(&[(PATH, BILLING), ("src/other.rs", "pub const X: u8 = 0;\n")]);
    let renamed = BILLING.replace("pub fn tax(", "pub fn vat(");
    let (v2, _) = next(&v1, &[(PATH, Some(&renamed))]);
    let (v3, _) = next(
        &v2,
        &[(
            PATH,
            Some(&format!(
                "{renamed}\n{}",
                "pub fn tax(amount: u64) -> u64 {\n    let rate = 20;\n    amount * rate / 100\n}\n"
            )),
        )],
    );
    assert_eq!(v3.id(PATH, "crate::billing::vat"), v1.id(PATH, TAX));
    assert_ne!(v3.id(PATH, TAX), v1.id(PATH, TAX));
}

#[test]
fn a_large_rewrite_stays_within_the_pair_bound() {
    let functions = |prefix: &str, count: usize| {
        (0..count)
            .map(|index| format!("pub fn {prefix}{index}(value: u32) -> u32 {{\n    value + {index}\n}}\n"))
            .collect::<String>()
    };
    let (v1, _) = first(&[("src/big.rs", &functions("old", 3_000))]);
    let started = std::time::Instant::now();
    let (v2, outcome) = next(&v1, &[("src/big.rs", Some(&functions("new", 3_000)))]);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(v2.files["src/big.rs"].0.nodes.len(), 3_000);
    assert!(!outcome.deltas.is_empty());
}

#[test]
fn outcomes_match_the_committed_snapshot() {
    // The same text on every platform: ids, deltas and the order of both (User Story 2).
    let (v1, first_outcome) = first(&[(PATH, BILLING), ("src/rates.rs", "pub const BASE: u64 = 1;\n")]);
    let edited = BILLING
        .replace("pub fn tax(", "pub fn vat(")
        .replace("self.lines.len()", "self.lines.len() + 0")
        .replace("Invoice", "Bill");
    let (v2, outcome) = next(&v1, &[(PATH, Some(&edited)), ("src/rates.rs", None)]);
    let mut text = String::new();
    for (label, outcome) in [("first", &first_outcome), ("next", &outcome)] {
        text.push_str(&format!("{label}:\n"));
        for delta in &outcome.deltas {
            text.push_str(&format!("  {:?}\n", delta));
        }
    }
    text.push_str("maps:\n");
    for (map, _) in v2.files.values() {
        for node in &map.nodes {
            text.push_str(&format!("  {} {} {}\n", map.path, node.qualified_path, node.id));
        }
    }
    insta::assert_snapshot!(text);
}
