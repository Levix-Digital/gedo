//! Port to the repository's object database.

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::error::StoreError;
use crate::ids::{GitOid, ObjectHash};

/// Largest blob read back, in bytes (64 MiB); larger blobs are refused with `TooLarge`.
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;

/// What a tree entry points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryMode {
    /// A blob (file mode `100644`).
    Blob,
    /// A subtree (file mode `40000`).
    Tree,
    /// An executable blob (file mode `100755`), found in source histories.
    ExecutableBlob,
    /// A symbolic link (file mode `120000`), found in source histories.
    Link,
    /// A submodule commit (file mode `160000`), found in source histories.
    Submodule,
}

/// One entry of a tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TreeEntry {
    /// Entry name: non-empty, without `/` or NUL, not `.` or `..`.
    pub name: String,
    /// Blob or subtree.
    pub mode: EntryMode,
    /// The object it points to.
    pub oid: GitOid,
}

/// A commit to write; author and committer are the repository's identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCommit {
    /// The root tree.
    pub tree: GitOid,
    /// Parent commits, first parent first.
    pub parents: Vec<GitOid>,
    /// The commit message.
    pub message: String,
}

/// A commit read back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitInfo {
    /// The root tree.
    pub tree: GitOid,
    /// Parent commits, first parent first.
    pub parents: Vec<GitOid>,
    /// The commit message.
    pub message: String,
    /// The author's e-mail as written in the commit (invalid UTF-8 replaced).
    pub author_email: String,
}

/// Content-addressed storage of blobs, trees and commits.
///
/// Writing the same content twice returns the same id, and nothing outside the object database
/// changes: not the working tree, the index, `HEAD` or any branch.
pub trait ObjectStore {
    /// The hash of every id this store returns.
    fn object_hash(&self) -> ObjectHash;
    /// Stores a blob.
    fn write_blob(&self, data: &[u8]) -> Result<GitOid, StoreError>;
    /// Stores several blobs at once and returns their ids in the same order; a store may write a
    /// large batch as one unit (a pack) instead of one object at a time.
    fn write_blobs(&self, blobs: &[&[u8]]) -> Result<Vec<GitOid>, StoreError> {
        blobs.iter().map(|data| self.write_blob(data)).collect()
    }
    /// Reads a blob; `ObjectNotFound` if absent, `TooLarge` above [`MAX_BLOB_BYTES`].
    fn read_blob(&self, oid: &GitOid) -> Result<Vec<u8>, StoreError>;
    /// Stores a tree; entries may come in any order and are written in Git's order.
    fn write_tree(&self, entries: &[TreeEntry]) -> Result<GitOid, StoreError>;
    /// Reads a tree, entries in Git's order.
    fn read_tree(&self, oid: &GitOid) -> Result<Vec<TreeEntry>, StoreError>;
    /// Stores a commit signed with the repository identity; `MissingIdentity` if none is set.
    fn write_commit(&self, commit: &NewCommit) -> Result<GitOid, StoreError>;
    /// Reads a commit.
    fn read_commit(&self, oid: &GitOid) -> Result<CommitInfo, StoreError>;
}

/// Validates tree entries and returns them in Git's order.
///
/// Git sorts entries by name bytes, comparing a subtree's name as if it ended with `/`.
pub fn sorted_tree_entries(entries: &[TreeEntry]) -> Result<Vec<TreeEntry>, StoreError> {
    let mut names = HashSet::with_capacity(entries.len());
    for entry in entries {
        validate_entry_name(&entry.name)?;
        if !names.insert(entry.name.as_str()) {
            return Err(StoreError::invalid_input(
                "write tree",
                format!("duplicate entry name {:?}", entry.name),
            ));
        }
    }
    let mut sorted = entries.to_vec();
    sorted.sort_by(git_tree_order);
    Ok(sorted)
}

/// Git's tree entry order.
pub fn git_tree_order(left: &TreeEntry, right: &TreeEntry) -> Ordering {
    sort_key(left).cmp(sort_key(right).as_ref())
}

fn sort_key(entry: &TreeEntry) -> Vec<u8> {
    let mut key = entry.name.as_bytes().to_vec();
    if entry.mode == EntryMode::Tree {
        key.push(b'/');
    }
    key
}

fn validate_entry_name(name: &str) -> Result<(), StoreError> {
    let reason = if name.is_empty() {
        "is empty"
    } else if name == "." || name == ".." {
        "is . or .."
    } else if name.contains(['/', '\0']) {
        "contains / or NUL"
    } else {
        return Ok(());
    };
    Err(StoreError::invalid_input(
        "write tree",
        format!("entry name {name:?} {reason}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, mode: EntryMode) -> TreeEntry {
        TreeEntry {
            name: name.into(),
            mode,
            oid: GitOid::from_bytes(&[0; 20]).expect("valid id"),
        }
    }

    #[test]
    fn subtrees_sort_as_if_they_ended_with_a_slash() {
        let sorted = sorted_tree_entries(&[
            entry("a.b", EntryMode::Blob),
            entry("a", EntryMode::Tree),
            entry("a-b", EntryMode::Blob),
        ])
        .expect("valid entries");
        let names: Vec<_> = sorted.iter().map(|entry| entry.name.as_str()).collect();
        // '-' (0x2d) < '.' (0x2e) < '/' (0x2f)
        assert_eq!(names, ["a-b", "a.b", "a"]);
    }

    #[test]
    fn invalid_and_duplicate_names_are_refused() {
        for name in ["", ".", "..", "a/b", "a\0b"] {
            assert!(
                sorted_tree_entries(&[entry(name, EntryMode::Blob)]).is_err(),
                "{name:?}"
            );
        }
        assert!(sorted_tree_entries(&[entry("x", EntryMode::Blob), entry("x", EntryMode::Tree)]).is_err());
    }
}
