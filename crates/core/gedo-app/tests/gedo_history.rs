//! The Gedo repository's own history, indexed through the Git store adapter: the identities of a
//! fixed commit are the same on every platform (User Story 2, SC-003).

mod support;

use std::path::Path;

use gedo_app::IdentityIndex;
use gedo_core::ids::GitOid;
use gedo_parse::TreeSitterParser;
use gedo_store::{GitObjectStore, GitRepository};

/// The merge of pull request #6, the last commit of feature 002.
const COMMIT: &str = "ad25ac3fa3f8fb1b0c7c025bc699f3e626872f9d";

/// Files whose identities the snapshot records.
const FILES: &[&str] = &[
    "crates/core/gedo-core/src/ids.rs",
    "crates/core/gedo-ast/src/hash.rs",
];

#[test]
fn the_identities_of_a_fixed_commit_match_the_snapshot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let Ok(repository) = GitRepository::open(&root) else {
        eprintln!("skipped: {} is not a Git repository", root.display());
        return;
    };
    let objects = GitObjectStore::new(repository);
    let commit = GitOid::from_hex(COMMIT).expect("valid id");
    let parser = TreeSitterParser::new();
    let cache = support::cache();
    let mut index = IdentityIndex::open(&objects, &parser, &cache).expect("index opens");
    match index.index_history(&[commit]) {
        Ok(report) => assert!(report.commits_indexed > 0),
        Err(error) => {
            // A shallow clone (the default checkout in CI) does not have the history.
            eprintln!("skipped: the history of {COMMIT} is not available ({error})");
            return;
        }
    }
    let mut text = String::new();
    for path in FILES {
        let map = index
            .map_at(&commit, path)
            .expect("indexed")
            .expect("file exists");
        for node in &map.nodes {
            text.push_str(&format!("{path} {} {}\n", node.qualified_path, node.id));
        }
    }
    let changes = index.changes_of(&commit).expect("indexed");
    text.push_str(&format!("deltas at {COMMIT}: {}\n", changes.deltas.len()));
    insta::assert_snapshot!(text);
}
