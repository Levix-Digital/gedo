//! History indexing (User Stories 2, 3 and 5).

mod support;

use gedo_app::IdentityIndex;
use gedo_core::identity::SemanticDelta;
use gedo_core::ids::GitOid;
use proptest::prelude::*;
use support::{History, files, generated};

const LIB: &str = "pub mod billing;\n";

fn billing(name: &str, rate: u32) -> String {
    format!(
        "pub struct Invoice {{\n    lines: Vec<u64>,\n}}\n\nimpl Invoice {{\n    pub fn total(&self) -> u64 {{\n        self.lines.iter().sum()\n    }}\n}}\n\npub fn {name}(amount: u64) -> u64 {{\n    amount * {rate} / 100\n}}\n"
    )
}

/// root -> rename -> move to another file, with a side branch that edits the body.
struct Linear {
    history: History,
    root: GitOid,
    renamed: GitOid,
    moved: GitOid,
}

fn linear() -> Linear {
    let history = History::default();
    let root = history.commit(
        &[],
        &files(&[
            ("src/lib.rs", LIB),
            ("src/billing.rs", &billing("tax", 20)),
            ("README.md", "x"),
        ]),
    );
    let renamed = history.commit(
        &[root],
        &files(&[
            ("src/lib.rs", LIB),
            ("src/billing.rs", &billing("vat", 20)),
            ("README.md", "y"),
        ]),
    );
    let without =
        billing("vat", 20).replace("pub fn vat(amount: u64) -> u64 {\n    amount * 20 / 100\n}\n", "");
    let moved = history.commit(
        &[renamed],
        &files(&[
            ("src/lib.rs", "pub mod billing;\npub mod rates;\n"),
            ("src/billing.rs", &without),
            (
                "src/rates.rs",
                "pub fn vat(amount: u64) -> u64 {\n    amount * 20 / 100\n}\n",
            ),
        ]),
    );
    Linear {
        history,
        root,
        renamed,
        moved,
    }
}

#[test]
fn renames_and_moves_across_commits_keep_ids() {
    let linear = linear();
    let mut cache = linear.history.index(&[&[linear.moved]]);
    let original = linear
        .history
        .id(&mut cache, &linear.root, "src/billing.rs", "crate::billing::tax");
    assert!(original.is_some());
    assert_eq!(
        linear.history.id(
            &mut cache,
            &linear.renamed,
            "src/billing.rs",
            "crate::billing::vat"
        ),
        original
    );
    assert_eq!(
        linear
            .history
            .id(&mut cache, &linear.moved, "src/rates.rs", "crate::rates::vat"),
        original
    );

    let index =
        IdentityIndex::open(&linear.history.objects, &linear.history.parser, &mut cache).expect("opens");
    let changes = index.changes_of(&linear.moved).expect("indexed");
    assert!(
        changes
            .deltas
            .iter()
            .any(|delta| matches!(delta, SemanticDelta::Moved { id, .. } if Some(*id) == original))
    );
    assert_eq!(
        index.files_at(&linear.root).expect("indexed"),
        ["src/billing.rs", "src/lib.rs"]
    );
}

#[test]
fn indexing_again_writes_nothing_and_resumes_where_it_stopped() {
    let linear = linear();
    let mut cache = support::cache();
    let mut index =
        IdentityIndex::open(&linear.history.objects, &linear.history.parser, &mut cache).expect("opens");
    assert_eq!(
        index
            .index_history(&[linear.renamed])
            .expect("indexed")
            .commits_indexed,
        2
    );
    assert_eq!(
        index
            .index_history(&[linear.moved])
            .expect("indexed")
            .commits_indexed,
        1
    );
    assert_eq!(
        index
            .index_history(&[linear.moved])
            .expect("indexed")
            .commits_indexed,
        0
    );
    let all = [linear.root, linear.renamed, linear.moved];
    let resumed = linear.history.render(&mut cache, &all);
    let mut fresh = linear.history.index(&[&[linear.moved]]);
    assert_eq!(linear.history.render(&mut fresh, &all), resumed);
}

#[test]
fn a_parse_timeout_stops_indexing_without_recording_the_commit() {
    let linear = linear();
    let slow = History {
        objects: linear.history.objects,
        parser: gedo_parse::TreeSitterParser::with_timeout(std::time::Duration::ZERO),
    };
    let mut cache = support::cache();
    let mut index = IdentityIndex::open(&slow.objects, &slow.parser, &mut cache).expect("opens");
    assert!(matches!(
        index.index_history(&[linear.root]),
        Err(gedo_app::IndexError::ParseTimeout { .. })
    ));
    assert!(matches!(
        index.map_at(&linear.root, "src/lib.rs"),
        Err(gedo_app::IndexError::NotIndexed { .. })
    ));
}

#[test]
fn unindexed_commits_are_reported() {
    let linear = linear();
    let mut cache = linear.history.index(&[&[linear.root]]);
    let index =
        IdentityIndex::open(&linear.history.objects, &linear.history.parser, &mut cache).expect("opens");
    assert!(matches!(
        index.map_at(&linear.moved, "src/rates.rs"),
        Err(gedo_app::IndexError::NotIndexed { .. })
    ));
}

/// base -> left (adds f with body 1), base -> right (adds f with body 2), merge keeps left's f.
struct Diverged {
    history: History,
    base: GitOid,
    left: GitOid,
    right: GitOid,
    merge: GitOid,
}

fn diverged(merge_keeps: u32) -> Diverged {
    let history = History::default();
    let base = history.commit(&[], &files(&[("src/lib.rs", "pub fn base() {}\n")]));
    let with = |body: u32| format!("pub fn base() {{}}\n\npub fn f() -> u32 {{\n    {body}\n}}\n");
    let left = history.commit(&[base], &files(&[("src/lib.rs", &with(1))]));
    let right = history.commit(
        &[base],
        &files(&[("src/lib.rs", &with(2)), ("src/extra.rs", "pub fn e() {}\n")]),
    );
    let merge = history.commit(
        &[left, right],
        &files(&[
            ("src/lib.rs", &with(merge_keeps)),
            ("src/extra.rs", "pub fn e() {}\n"),
        ]),
    );
    Diverged {
        history,
        base,
        left,
        right,
        merge,
    }
}

#[test]
fn two_ids_for_one_entity_are_reconciled_to_the_lower() {
    for keeps in [1, 2, 3] {
        let diverged = diverged(keeps);
        let mut cache = diverged.history.index(&[&[diverged.merge]]);
        // Each branch created its own id; lookups resolve them, so read the creations.
        let mut created = |commit: &GitOid| {
            let index = IdentityIndex::open(&diverged.history.objects, &diverged.history.parser, &mut cache)
                .expect("opens");
            index
                .changes_of(commit)
                .expect("indexed")
                .deltas
                .into_iter()
                .find_map(|delta| match delta {
                    SemanticDelta::Created { id, at, .. } if at.qualified_path == "crate::f" => Some(id),
                    _ => None,
                })
                .expect("f is created")
        };
        let (left, right) = (created(&diverged.left), created(&diverged.right));
        let merged = diverged
            .history
            .id(&mut cache, &diverged.merge, "src/lib.rs", "crate::f")
            .expect("f merged");
        let lower = left.min(right);
        assert_ne!(left, right);
        assert_eq!(merged, lower, "merge keeps {keeps}");

        let index = IdentityIndex::open(&diverged.history.objects, &diverged.history.parser, &mut cache)
            .expect("opens");
        let changes = index.changes_of(&diverged.merge).expect("indexed");
        assert_eq!(
            changes.merges.len(),
            1,
            "merge keeps {keeps}: {:?}",
            changes.merges
        );
        assert_eq!(changes.merges[0].kept, lower);
        assert_eq!(changes.merges[0].merged, left.max(right));
        assert_eq!(index.resolve(left.max(right)), lower);
        let _ = diverged.base;
    }
}

#[test]
fn identical_independent_creations_share_one_id() {
    let history = History::default();
    let base = history.commit(&[], &files(&[("src/lib.rs", "pub fn base() {}\n")]));
    let same = files(&[(
        "src/lib.rs",
        "pub fn base() {}\n\npub fn f() -> u32 {\n    1\n}\n",
    )]);
    let left = history.commit(&[base], &same);
    let mut other = same.clone();
    other.insert("README.md".into(), "different commit".into());
    let right = history.commit(&[base], &other);
    let mut cache = history.index(&[&[left, right]]);
    assert_eq!(
        history.id(&mut cache, &left, "src/lib.rs", "crate::f"),
        history.id(&mut cache, &right, "src/lib.rs", "crate::f")
    );
}

#[test]
fn octopus_merges_and_several_roots_are_indexed() {
    let history = History::default();
    let roots: Vec<GitOid> = (0..3)
        .map(|index| {
            history.commit(
                &[],
                &files(&[(
                    &format!("src/m{index}.rs") as &str,
                    &format!("pub fn f{index}() {{}}\n") as &str,
                )]),
            )
        })
        .collect();
    let mut all = std::collections::BTreeMap::new();
    for index in 0..3 {
        all.insert(format!("src/m{index}.rs"), format!("pub fn f{index}() {{}}\n"));
    }
    let octopus = history.commit(&roots, &all);
    let mut cache = history.index(&[&[octopus]]);
    for (index, root) in roots.iter().enumerate() {
        let path = format!("src/m{index}.rs");
        let qualified = format!("crate::m{index}::f{index}");
        assert_eq!(
            history.id(&mut cache, &octopus, &path, &qualified),
            history.id(&mut cache, root, &path, &qualified),
            "{path}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn the_order_of_indexing_never_changes_the_result(seed in any::<u64>(), split in 1usize..12) {
        let (history, commits) = generated(seed);
        let reference = {
            let mut cache = history.index(&[&commits]);
            history.render(&mut cache, &commits)
        };
        let reversed: Vec<GitOid> = commits.iter().rev().copied().collect();
        let mut by_tip = history.index(&[&reversed]);
        prop_assert_eq!(history.render(&mut by_tip, &commits), reference.clone());

        // Interrupted after `split` commits, branch by branch, then completed.
        let at = split.min(commits.len());
        let mut piecewise = history.index(&[&commits[..at], &reversed[..1], &commits]);
        prop_assert_eq!(history.render(&mut piecewise, &commits), reference.clone());

        // One commit at a time, newest first: each call indexes the missing ancestors.
        let singles: Vec<&[GitOid]> = reversed.iter().map(std::slice::from_ref).collect();
        let mut one_by_one = history.index(&singles);
        prop_assert_eq!(history.render(&mut one_by_one, &commits), reference);
    }
}
