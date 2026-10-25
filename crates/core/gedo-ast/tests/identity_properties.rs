//! Determinism properties of matching and derivation (User Story 2).

mod support;

use std::collections::BTreeSet;

use gedo_ast::{ChangeSet, NewFile, match_change};
use proptest::prelude::*;
use support::{blob, first, old_file, syntax};

/// A generated file: functions named from a small vocabulary with small bodies.
fn source(functions: &[(u8, u8, u8)]) -> String {
    const NAMES: [&str; 8] = [
        "alpha", "beta", "gamma", "delta", "omega", "sigma", "kappa", "theta",
    ];
    let mut text = String::new();
    let mut seen = BTreeSet::new();
    for &(name, arity, body) in functions {
        let name = NAMES[usize::from(name) % NAMES.len()];
        if !seen.insert(name) {
            continue;
        }
        let parameters: Vec<String> = (0..arity % 3).map(|index| format!("p{index}: u32")).collect();
        let statements: Vec<String> = (0..body % 4)
            .map(|index| format!("    let v{index} = {body} + {index};"))
            .collect();
        text.push_str(&format!(
            "pub fn {name}({}) -> u32 {{\n{}\n    {}\n}}\n\n",
            parameters.join(", "),
            statements.join("\n"),
            body
        ));
    }
    text
}

fn files() -> impl Strategy<Value = Vec<Vec<(u8, u8, u8)>>> {
    proptest::collection::vec(proptest::collection::vec((0u8..8, 0u8..3, 0u8..6), 0..6), 1..4)
}

fn paths(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("src/m{index}.rs")).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn file_order_does_not_change_the_outcome(before in files(), after in files(), rotate in 0usize..4) {
        let paths = paths(before.len().max(after.len()));
        let old_sources: Vec<String> = before.iter().map(|functions| source(functions)).collect();
        let new_sources: Vec<String> = after.iter().map(|functions| source(functions)).collect();
        let pairs: Vec<(&str, &str)> = paths.iter().zip(&old_sources).map(|(path, text)| (path.as_str(), text.as_str())).collect();
        let (v1, _) = first(&pairs);

        let build = |order: &[usize]| {
            let mut change = ChangeSet::default();
            for &index in order {
                if let Some((map, syntax_map)) = v1.files.get(&paths[index]) {
                    change.old.push(old_file(map, syntax_map));
                }
            }
            for &index in order.iter().rev() {
                if let Some(text) = new_sources.get(index) {
                    change.new.push(NewFile { blob: blob(text), syntax: syntax(&paths[index], text) });
                }
            }
            match_change(&change, &BTreeSet::new())
        };
        let order: Vec<usize> = (0..paths.len()).collect();
        let mut rotated = order.clone();
        rotated.rotate_left(rotate % paths.len());
        prop_assert_eq!(build(&order), build(&rotated));
    }

    #[test]
    fn matching_a_version_against_itself_keeps_every_id(sources in files()) {
        let paths = paths(sources.len());
        let texts: Vec<String> = sources.iter().map(|functions| source(functions)).collect();
        let pairs: Vec<(&str, &str)> = paths.iter().zip(&texts).map(|(path, text)| (path.as_str(), text.as_str())).collect();
        let (v1, _) = first(&pairs);
        let changes: Vec<(&str, Option<&str>)> = pairs.iter().map(|&(path, text)| (path, Some(text))).collect();
        let (v2, outcome) = support::next(&v1, &changes);
        prop_assert_eq!(outcome.deltas, vec![]);
        prop_assert_eq!(v2.ids(), v1.ids());
    }

    #[test]
    fn ids_are_unique_and_matched_entities_keep_theirs(before in files(), after in files()) {
        let paths = paths(before.len().max(after.len()));
        let old_texts: Vec<String> = before.iter().map(|functions| source(functions)).collect();
        let new_texts: Vec<String> = after.iter().map(|functions| source(functions)).collect();
        let pairs: Vec<(&str, &str)> = paths.iter().zip(&old_texts).map(|(path, text)| (path.as_str(), text.as_str())).collect();
        let (v1, _) = first(&pairs);
        let changes: Vec<(&str, Option<&str>)> = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.as_str(), new_texts.get(index).map(String::as_str)))
            .collect();
        let (v2, outcome) = support::next(&v1, &changes);

        let mut seen = BTreeSet::new();
        for (map, _) in v2.files.values() {
            for node in &map.nodes {
                prop_assert!(seen.insert(node.id), "duplicate id {}", node.id);
            }
        }
        // Every old id is either still present or tombstoned, never both; every created id is new.
        let old_ids = v1.ids();
        for delta in &outcome.deltas {
            if support::kinds_of(&outcome, delta.id()) == ["created"] {
                prop_assert!(!old_ids.contains(&delta.id()));
            }
        }
        for id in &old_ids {
            let tombstoned = support::kinds_of(&outcome, *id).contains(&"tombstoned");
            prop_assert!(seen.contains(id) != tombstoned, "{id} present: {} tombstoned: {tombstoned}", seen.contains(id));
        }
    }
}
