//! Collections: sets of content-addressed entries stored as a commit chain under one reference
//! (ADR-0011).
//!
//! The reference `refs/gedo/<collection>` points to the newest commit. Its tree holds the entries
//! in a two-level fan-out (`ab/cdef...` for the entry `abcdef...`), like Git notes. Every change is
//! a new commit whose parent is the previous head, written with compare-and-swap, so the chain
//! fast-forwards like a branch and a plain push never needs force. Chains that diverged (another
//! machine pushed) are joined with a merge commit whose tree is the union of both sides.

use std::collections::{BTreeMap, HashSet, VecDeque};

use crate::error::StoreError;
use crate::ids::{EntryId, GedoRefName, GitOid};
use crate::ports::{EntryMode, Expected, NewCommit, ObjectStore, RefOutcome, RefStore, RefUpdate, TreeEntry};

/// Compare-and-swap attempts before an update is given up as `Contended`.
pub const MAX_UPDATE_ATTEMPTS: u32 = 8;

/// The entries of a collection: entry id to the blob holding its content.
pub type EntryMap = BTreeMap<EntryId, GitOid>;

/// What joining a remote chain did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The remote has no chain for this collection.
    NoRemoteChain,
    /// The remote chain is already part of the local one.
    UpToDate,
    /// The local chain was behind and now points to the remote head.
    FastForwarded {
        /// The new head.
        head: GitOid,
    },
    /// Both chains had changes; a merge commit joins them.
    Merged {
        /// The merge commit.
        head: GitOid,
    },
}

/// One collection over an object store and a reference store.
#[derive(Debug)]
pub struct Collection<'a, O: ObjectStore, R: RefStore> {
    name: GedoRefName,
    objects: &'a O,
    refs: &'a R,
}

impl<'a, O: ObjectStore, R: RefStore> Collection<'a, O, R> {
    /// The collection stored at `name`.
    pub fn new(name: GedoRefName, objects: &'a O, refs: &'a R) -> Self {
        Collection { name, objects, refs }
    }

    /// The collection's reference.
    pub fn name(&self) -> &GedoRefName {
        &self.name
    }

    /// The newest commit of the chain, if the collection exists.
    pub fn head(&self) -> Result<Option<GitOid>, StoreError> {
        self.refs.read(&self.name)
    }

    /// The content of one entry.
    pub fn get(&self, id: &EntryId) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(head) = self.head()? else { return Ok(None) };
        let root = self.objects.read_tree(&self.objects.read_commit(&head)?.tree)?;
        let (directory, file) = id.fan_out();
        let Some(fan_out) = find_entry(&root, directory, EntryMode::Tree) else {
            return Ok(None);
        };
        let subtree = self.objects.read_tree(&fan_out)?;
        match find_entry(&subtree, file, EntryMode::Blob) {
            Some(blob) => Ok(Some(self.objects.read_blob(&blob)?)),
            None => Ok(None),
        }
    }

    /// Every entry of the collection.
    pub fn entries(&self) -> Result<EntryMap, StoreError> {
        match self.head()? {
            Some(head) => self.entries_of(&head),
            None => Ok(EntryMap::new()),
        }
    }

    /// Every entry of the chain ending at `commit`.
    pub fn entries_of(&self, commit: &GitOid) -> Result<EntryMap, StoreError> {
        let root = self.objects.read_tree(&self.objects.read_commit(commit)?.tree)?;
        let mut entries = EntryMap::new();
        for directory in root {
            if directory.mode != EntryMode::Tree {
                return Err(self.corrupted(format!(
                    "unexpected file {:?} at the top of the tree",
                    directory.name
                )));
            }
            for file in self.objects.read_tree(&directory.oid)? {
                let id = EntryId::new(&format!("{}{}", directory.name, file.name))
                    .map_err(|error| self.corrupted(error.to_string()))?;
                if file.mode != EntryMode::Blob || id.fan_out().0 != directory.name {
                    return Err(self.corrupted(format!("entry {id} is not stored as a fan-out file")));
                }
                entries.insert(id, file.oid);
            }
        }
        Ok(entries)
    }

    /// Adds entries in one new commit and returns the head.
    ///
    /// Entries already present with the same content are kept as they are; when nothing changes,
    /// no commit is written. The same id with different content is `Corrupted`, because entries
    /// are content-addressed. A concurrent writer moving the reference causes a reload and a
    /// retry, up to [`MAX_UPDATE_ATTEMPTS`].
    pub fn put(&self, entries: &[(EntryId, Vec<u8>)], message: &str) -> Result<GitOid, StoreError> {
        if entries.is_empty() {
            return Err(StoreError::invalid_input(
                "put into collection",
                "no entries given",
            ));
        }
        let mut blobs = EntryMap::new();
        for (id, content) in entries {
            let blob = self.objects.write_blob(content)?;
            if blobs
                .insert(id.clone(), blob)
                .is_some_and(|previous| previous != blob)
            {
                return Err(StoreError::invalid_input(
                    "put into collection",
                    format!("entry {id} given twice"),
                ));
            }
        }
        self.update(|head| {
            let Some(tree) = self.add_to_tree(head, &blobs)? else {
                return Ok(None);
            };
            let parents = head.into_iter().collect();
            Ok(Some(self.objects.write_commit(&NewCommit {
                tree,
                parents,
                message: message.to_owned(),
            })?))
        })
    }

    /// The root tree of `head` with `blobs` added, rewriting only the fan-out directories they
    /// fall into; `None` when every entry is already present.
    fn add_to_tree(&self, head: Option<GitOid>, blobs: &EntryMap) -> Result<Option<GitOid>, StoreError> {
        let mut root: BTreeMap<String, GitOid> = BTreeMap::new();
        if let Some(head) = head {
            for directory in self.objects.read_tree(&self.objects.read_commit(&head)?.tree)? {
                if directory.mode != EntryMode::Tree {
                    return Err(self.corrupted(format!(
                        "unexpected file {:?} at the top of the tree",
                        directory.name
                    )));
                }
                root.insert(directory.name, directory.oid);
            }
        }
        let mut by_directory: BTreeMap<&str, Vec<(&str, &EntryId, GitOid)>> = BTreeMap::new();
        for (id, blob) in blobs {
            let (directory, file) = id.fan_out();
            by_directory.entry(directory).or_default().push((file, id, *blob));
        }
        let mut changed = false;
        for (directory, additions) in by_directory {
            let mut files = match root.get(directory) {
                Some(subtree) => self.objects.read_tree(subtree)?,
                None => Vec::new(),
            };
            let before = files.len();
            for (file, id, blob) in additions {
                match files.iter().find(|entry| entry.name == file) {
                    Some(existing) if existing.oid != blob || existing.mode != EntryMode::Blob => {
                        return Err(self.conflict(id));
                    }
                    Some(_) => {}
                    None => files.push(TreeEntry {
                        name: file.to_owned(),
                        mode: EntryMode::Blob,
                        oid: blob,
                    }),
                }
            }
            if files.len() != before {
                root.insert(directory.to_owned(), self.objects.write_tree(&files)?);
                changed = true;
            }
        }
        if !changed && head.is_some() {
            return Ok(None);
        }
        let entries: Vec<TreeEntry> = root
            .into_iter()
            .map(|(name, oid)| TreeEntry {
                name,
                mode: EntryMode::Tree,
                oid,
            })
            .collect();
        Ok(Some(self.objects.write_tree(&entries)?))
    }

    /// Joins the chain `git fetch` mirrored from `remote` into the local chain.
    pub fn merge_remote(&self, remote: &str) -> Result<MergeOutcome, StoreError> {
        let short_name = self.name.short_name();
        let remote_head = self
            .refs
            .list_remote(remote)?
            .into_iter()
            .find(|(name, _)| name == short_name);
        match remote_head {
            Some((_, head)) => self.merge(head),
            None => Ok(MergeOutcome::NoRemoteChain),
        }
    }

    /// Joins the chain ending at `other` into the local chain.
    pub fn merge(&self, other: GitOid) -> Result<MergeOutcome, StoreError> {
        let mut outcome = MergeOutcome::UpToDate;
        self.update(|head| {
            let Some(local) = head else {
                outcome = MergeOutcome::FastForwarded { head: other };
                return Ok(Some(other));
            };
            if local == other || self.is_ancestor(&other, &local)? {
                outcome = MergeOutcome::UpToDate;
                return Ok(None);
            }
            if self.is_ancestor(&local, &other)? {
                outcome = MergeOutcome::FastForwarded { head: other };
                return Ok(Some(other));
            }
            let merged = union(&self.entries_of(&local)?, &self.entries_of(&other)?)
                .map_err(|id| self.conflict(&id))?;
            let tree = self.write_entries(&merged)?;
            let message = format!("gedo: merge {other} into {}", self.name);
            let commit = self.objects.write_commit(&NewCommit {
                tree,
                parents: vec![local, other],
                message,
            })?;
            outcome = MergeOutcome::Merged { head: commit };
            Ok(Some(commit))
        })?;
        Ok(outcome)
    }

    /// Runs `change` on the current head and compare-and-swaps the reference to the commit it
    /// returns, retrying from the new head when another writer moved the reference first.
    /// `change` returns `None` when there is nothing to write.
    fn update(
        &self,
        mut change: impl FnMut(Option<GitOid>) -> Result<Option<GitOid>, StoreError>,
    ) -> Result<GitOid, StoreError> {
        let mut head = self.head()?;
        for attempt in 1..=MAX_UPDATE_ATTEMPTS {
            let Some(new) = change(head)? else {
                return head
                    .ok_or_else(|| StoreError::invalid_input("update collection", "nothing to write"));
            };
            let update = RefUpdate {
                name: self.name.clone(),
                expected: Expected::from_current(head),
                new,
                log_message: format!("gedo: update {}", self.name.short_name()),
            };
            match self.refs.compare_and_swap(&update)? {
                RefOutcome::Updated => return Ok(new),
                RefOutcome::Moved { current } => {
                    tracing::debug!(collection = %self.name, attempt, "reference moved by another writer; retrying");
                    head = current;
                }
            }
        }
        Err(StoreError::Contended {
            name: self.name.to_string(),
            attempts: MAX_UPDATE_ATTEMPTS,
        })
    }

    /// Writes the fan-out tree of `entries` and returns its root.
    fn write_entries(&self, entries: &EntryMap) -> Result<GitOid, StoreError> {
        let mut directories: BTreeMap<&str, Vec<TreeEntry>> = BTreeMap::new();
        for (id, blob) in entries {
            let (directory, file) = id.fan_out();
            directories.entry(directory).or_default().push(TreeEntry {
                name: file.to_owned(),
                mode: EntryMode::Blob,
                oid: *blob,
            });
        }
        let mut root = Vec::with_capacity(directories.len());
        for (directory, files) in directories {
            let oid = self.objects.write_tree(&files)?;
            root.push(TreeEntry {
                name: directory.to_owned(),
                mode: EntryMode::Tree,
                oid,
            });
        }
        self.objects.write_tree(&root)
    }

    /// Whether `ancestor` is reachable from `descendant` through parents.
    fn is_ancestor(&self, ancestor: &GitOid, descendant: &GitOid) -> Result<bool, StoreError> {
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([*descendant]);
        while let Some(commit) = queue.pop_front() {
            if commit == *ancestor {
                return Ok(true);
            }
            if seen.insert(commit) {
                queue.extend(self.objects.read_commit(&commit)?.parents);
            }
        }
        Ok(false)
    }

    fn corrupted(&self, reason: String) -> StoreError {
        StoreError::Corrupted {
            path: self.name.to_string(),
            offset: None,
            reason,
        }
    }

    fn conflict(&self, id: &EntryId) -> StoreError {
        self.corrupted(format!("entry {id} has two different contents"))
    }
}

/// The union of two entry maps; `Err(id)` when an id has different contents on the two sides.
pub fn union(left: &EntryMap, right: &EntryMap) -> Result<EntryMap, EntryId> {
    let mut merged = left.clone();
    for (id, blob) in right {
        match merged.get(id) {
            Some(existing) if existing != blob => return Err(id.clone()),
            Some(_) => {}
            None => {
                merged.insert(id.clone(), *blob);
            }
        }
    }
    Ok(merged)
}

fn find_entry(entries: &[TreeEntry], name: &str, mode: EntryMode) -> Option<GitOid> {
    entries
        .iter()
        .find(|entry| entry.name == name && entry.mode == mode)
        .map(|entry| entry.oid)
}
