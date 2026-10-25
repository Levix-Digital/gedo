//! The files that differ between two trees (research R10). Subtrees with equal ids are skipped.

use std::collections::BTreeMap;

use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::ports::{EntryMode, ObjectStore};

/// One side of a tree entry: its mode and object, when present.
type Side = Option<(EntryMode, GitOid)>;

/// One file that differs: its blob before and after (`None` when absent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    /// The file's path, `/`-separated.
    pub path: String,
    /// The blob before.
    pub old: Option<GitOid>,
    /// The blob after.
    pub new: Option<GitOid>,
}

/// The files kept by `keep` that differ between `old` and `new`, sorted by path.
pub fn diff_trees<O: ObjectStore>(
    objects: &O,
    old: Option<&GitOid>,
    new: Option<&GitOid>,
    keep: &impl Fn(&str) -> bool,
) -> Result<Vec<FileChange>, StoreError> {
    let mut changes = Vec::new();
    diff_into(objects, old, new, "", keep, &mut changes)?;
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(changes)
}

fn diff_into<O: ObjectStore>(
    objects: &O,
    old: Option<&GitOid>,
    new: Option<&GitOid>,
    prefix: &str,
    keep: &impl Fn(&str) -> bool,
    changes: &mut Vec<FileChange>,
) -> Result<(), StoreError> {
    if old == new {
        return Ok(());
    }
    let mut entries: BTreeMap<String, (Side, Side)> = BTreeMap::new();
    if let Some(tree) = old {
        for entry in objects.read_tree(tree)? {
            entries.entry(entry.name).or_default().0 = Some((entry.mode, entry.oid));
        }
    }
    if let Some(tree) = new {
        for entry in objects.read_tree(tree)? {
            entries.entry(entry.name).or_default().1 = Some((entry.mode, entry.oid));
        }
    }
    for (name, (before, after)) in entries {
        let path = format!("{prefix}{name}");
        let subtree = |side: Side| {
            side.filter(|(mode, _)| *mode == EntryMode::Tree)
                .map(|(_, oid)| oid)
        };
        let file = |side: Side| {
            side.filter(|(mode, _)| matches!(mode, EntryMode::Blob | EntryMode::ExecutableBlob))
                .map(|(_, oid)| oid)
        };
        let (old_tree, new_tree) = (subtree(before), subtree(after));
        if old_tree.is_some() || new_tree.is_some() {
            diff_into(
                objects,
                old_tree.as_ref(),
                new_tree.as_ref(),
                &format!("{path}/"),
                keep,
                changes,
            )?;
        }
        let (old_file, new_file) = (file(before), file(after));
        if old_file != new_file && keep(&path) {
            changes.push(FileChange {
                path,
                old: old_file,
                new: new_file,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gedo_core::memory::MemoryObjectStore;
    use gedo_core::ports::TreeEntry;

    fn tree(objects: &MemoryObjectStore, files: &[(&str, &str)]) -> GitOid {
        // Builds nested trees from `dir/file` paths of at most two levels.
        let mut root: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
        let mut entries = Vec::new();
        for &(path, text) in files {
            match path.split_once('/') {
                Some((directory, name)) => root.entry(directory).or_default().push((name, text)),
                None => entries.push(TreeEntry {
                    name: path.into(),
                    mode: EntryMode::Blob,
                    oid: objects.write_blob(text.as_bytes()).expect("blob"),
                }),
            }
        }
        for (directory, children) in root {
            entries.push(TreeEntry {
                name: directory.into(),
                mode: EntryMode::Tree,
                oid: tree(objects, &children),
            });
        }
        objects.write_tree(&entries).expect("tree")
    }

    #[test]
    fn only_changed_kept_files_are_reported() {
        let objects = MemoryObjectStore::new();
        let before = tree(
            &objects,
            &[
                ("src/a.rs", "a"),
                ("src/b.rs", "b"),
                ("README.md", "r"),
                ("x/c.rs", "c"),
            ],
        );
        let after = tree(
            &objects,
            &[
                ("src/a.rs", "a2"),
                ("src/b.rs", "b"),
                ("README.md", "r2"),
                ("y/c.rs", "c"),
            ],
        );
        let keep = |path: &str| path.ends_with(".rs");
        let changes = diff_trees(&objects, Some(&before), Some(&after), &keep).expect("diff");
        let paths: Vec<_> = changes
            .iter()
            .map(|change| (change.path.as_str(), change.old.is_some(), change.new.is_some()))
            .collect();
        assert_eq!(
            paths,
            [
                ("src/a.rs", true, true),
                ("x/c.rs", true, false),
                ("y/c.rs", false, true)
            ]
        );
        assert_eq!(
            diff_trees(&objects, Some(&before), Some(&before), &keep).expect("diff"),
            []
        );
    }

    #[test]
    fn a_file_replaced_by_a_directory_is_both_a_deletion_and_additions() {
        let objects = MemoryObjectStore::new();
        let before = tree(&objects, &[("m.rs", "file")]);
        let after = tree(&objects, &[("m.rs/inner.rs", "nested")]);
        let changes = diff_trees(&objects, Some(&before), Some(&after), &|_: &str| true).expect("diff");
        let paths: Vec<_> = changes.iter().map(|change| change.path.as_str()).collect();
        assert_eq!(paths, ["m.rs", "m.rs/inner.rs"]);
    }
}
