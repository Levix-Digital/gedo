//! `cargo xtask arch-check`: no crate under the core path may depend, directly or transitively
//! through normal dependencies, on a crate the architecture forbids in the core (ADR-0001).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};

/// The architecture rule, from `.dev-cycle.json`.
#[derive(Debug, PartialEq, Eq)]
pub struct Rule {
    /// Directory holding the core crates, relative to the workspace root.
    pub core_path: String,
    /// Crate names the core must not depend on (`-` and `_` are equivalent).
    pub forbidden: BTreeSet<String>,
}

/// A core crate that reaches a forbidden crate, with the path of dependencies leading there.
#[derive(Debug, PartialEq, Eq)]
pub struct Violation {
    pub core_crate: String,
    pub path: Vec<String>,
}

/// Crates by name with their normal (not dev, not build) dependencies by name.
pub type DependencyGraph = BTreeMap<String, BTreeSet<String>>;

fn normalize(name: &str) -> String {
    name.replace('-', "_")
}

pub fn read_rule(dev_cycle_json: &str) -> Result<Rule, String> {
    let config: serde_json::Value =
        serde_json::from_str(dev_cycle_json).map_err(|error| format!(".dev-cycle.json: {error}"))?;
    let architecture = &config["architecture"];
    let core_path = architecture["core_path"]
        .as_str()
        .ok_or(".dev-cycle.json: architecture.core_path is missing")?
        .trim_end_matches('/')
        .to_owned();
    let forbidden = architecture["forbidden_core_imports"]
        .as_array()
        .ok_or(".dev-cycle.json: architecture.forbidden_core_imports is missing")?
        .iter()
        .map(|name| {
            name.as_str()
                .map(normalize)
                .ok_or(".dev-cycle.json: forbidden_core_imports must be strings")
        })
        .collect::<Result<_, _>>()?;
    Ok(Rule { core_path, forbidden })
}

/// Every core crate's shortest dependency path to a forbidden crate.
pub fn violations(
    graph: &DependencyGraph,
    core_crates: &BTreeSet<String>,
    forbidden: &BTreeSet<String>,
) -> Vec<Violation> {
    let mut found = Vec::new();
    for core_crate in core_crates {
        let mut parents: BTreeMap<String, String> = BTreeMap::new();
        let mut queue = VecDeque::from([normalize(core_crate)]);
        let mut seen = BTreeSet::from([normalize(core_crate)]);
        while let Some(current) = queue.pop_front() {
            if forbidden.contains(&current) && current != normalize(core_crate) {
                let mut path = vec![current.clone()];
                while let Some(parent) = parents.get(path.last().expect("non-empty")) {
                    path.push(parent.clone());
                }
                path.reverse();
                found.push(Violation {
                    core_crate: core_crate.clone(),
                    path,
                });
                break;
            }
            for dependency in graph.get(&current).into_iter().flatten() {
                let dependency = normalize(dependency);
                if seen.insert(dependency.clone()) {
                    parents.insert(dependency.clone(), current.clone());
                    queue.push_back(dependency);
                }
            }
        }
    }
    found
}

fn graph_from(metadata: &Metadata) -> Result<DependencyGraph, String> {
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or("cargo metadata has no dependency resolution")?;
    let names: BTreeMap<_, _> = metadata
        .packages
        .iter()
        .map(|package| (&package.id, package.name.to_string()))
        .collect();
    let mut graph = DependencyGraph::new();
    for node in &resolve.nodes {
        let name = names.get(&node.id).ok_or("unknown package in cargo metadata")?;
        let normal = node
            .deps
            .iter()
            .filter(|dependency| {
                dependency
                    .dep_kinds
                    .iter()
                    .any(|kind| kind.kind == DependencyKind::Normal)
            })
            .filter_map(|dependency| names.get(&dependency.pkg).map(|name| normalize(name)));
        graph.entry(normalize(name)).or_default().extend(normal);
    }
    Ok(graph)
}

pub fn run(workspace_root: &Path) -> Result<(), String> {
    let rule_text = std::fs::read_to_string(workspace_root.join(".dev-cycle.json"))
        .map_err(|error| format!("read .dev-cycle.json: {error}"))?;
    let rule = read_rule(&rule_text)?;
    let metadata = MetadataCommand::new()
        .manifest_path(workspace_root.join("Cargo.toml"))
        .exec()
        .map_err(|error| format!("cargo metadata: {error}"))?;
    let core_root = workspace_root.join(&rule.core_path);
    let core_crates: BTreeSet<String> = metadata
        .workspace_packages()
        .into_iter()
        .filter(|package| package.manifest_path.as_std_path().starts_with(&core_root))
        .map(|package| package.name.to_string())
        .collect();
    if core_crates.is_empty() {
        return Err(format!("no crate found under {}", rule.core_path));
    }
    let found = violations(&graph_from(&metadata)?, &core_crates, &rule.forbidden);
    if found.is_empty() {
        println!(
            "arch-check: {} core crate(s) free of forbidden dependencies",
            core_crates.len()
        );
        return Ok(());
    }
    for violation in &found {
        eprintln!(
            "arch-check: {} depends on a forbidden crate: {}",
            violation.core_crate,
            violation.path.join(" -> ")
        );
    }
    Err(format!("{} architecture violation(s)", found.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[(&str, &[&str])]) -> DependencyGraph {
        edges
            .iter()
            .map(|(name, dependencies)| {
                (
                    name.to_string(),
                    dependencies.iter().map(ToString::to_string).collect(),
                )
            })
            .collect()
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| normalize(name)).collect()
    }

    #[test]
    fn reads_the_rule_from_the_dev_cycle_configuration() {
        let rule = read_rule(
            r#"{"architecture":{"core_path":"crates/core/","forbidden_core_imports":["gix","tree-sitter"]}}"#,
        )
        .expect("valid");
        assert_eq!(
            rule,
            Rule {
                core_path: "crates/core".into(),
                forbidden: set(&["gix", "tree_sitter"])
            }
        );
        assert!(read_rule("{}").is_err());
    }

    #[test]
    fn a_clean_core_passes() {
        let graph = graph(&[
            ("gedo_core", &["serde", "thiserror"]),
            ("serde", &[]),
            ("gedo_store", &["gix"]),
        ]);
        assert!(violations(&graph, &set(&["gedo-core"]), &set(&["gix"])).is_empty());
    }

    #[test]
    fn direct_and_transitive_dependencies_are_reported_with_their_path() {
        let graph = graph(&[
            ("gedo_core", &["helper"]),
            ("helper", &["gix_hash"]),
            ("gix_hash", &[]),
            ("gedo_ast", &["tree-sitter"]),
        ]);
        let found = violations(
            &graph,
            &set(&["gedo-core", "gedo-ast"]),
            &set(&["gix-hash", "tree_sitter"]),
        );
        assert_eq!(
            found,
            vec![
                Violation {
                    core_crate: "gedo_ast".into(),
                    path: vec!["gedo_ast".into(), "tree_sitter".into()]
                },
                Violation {
                    core_crate: "gedo_core".into(),
                    path: vec!["gedo_core".into(), "helper".into(), "gix_hash".into()],
                },
            ]
        );
    }
}
