//! [`ObjectStore`] over the repository's object database, through gitoxide.
//!
//! Only `.git/objects` is written: never the working tree, the index, `HEAD` or a reference.

use gedo_core::ports::object::{MAX_BLOB_BYTES, sorted_tree_entries};
use gedo_core::ports::{CommitInfo, EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_core::{GitOid, ObjectHash, StoreError};
use gix::bstr::ByteSlice;
use gix::objs::tree::EntryKind;

use crate::convert::{from_gix_oid, to_gix_oid};
use crate::repository::GitRepository;

/// Objects in a Git repository.
#[derive(Clone, Debug)]
pub struct GitObjectStore {
    repository: GitRepository,
}

impl GitObjectStore {
    /// The object store of `repository`.
    pub fn new(repository: GitRepository) -> Self {
        GitObjectStore { repository }
    }

    /// Reads an object of the `expected` kind, checking its size against `limit` before loading it.
    fn read(
        &self,
        oid: &GitOid,
        expected: gix::object::Kind,
        limit: Option<usize>,
    ) -> Result<Vec<u8>, StoreError> {
        let repo = self.repository.local();
        let id = to_gix_oid(oid)?;
        let not_found = || StoreError::ObjectNotFound {
            oid: *oid,
            expected: kind_name(expected),
        };
        let header = repo
            .try_find_header(id)
            .map_err(|error| StoreError::backend("read object header", error))?;
        let header = header.ok_or_else(not_found)?;
        if header.kind() != expected {
            return Err(not_found());
        }
        if let Some(limit) = limit
            && header.size() > limit as u64
        {
            return Err(StoreError::TooLarge {
                what: format!("blob {oid}"),
                size: header.size(),
                limit: limit as u64,
            });
        }
        let object = repo
            .try_find_object(id)
            .map_err(|error| StoreError::backend("read object", error))?;
        Ok(object.ok_or_else(not_found)?.detach().data)
    }
}

fn kind_name(kind: gix::object::Kind) -> &'static str {
    match kind {
        gix::object::Kind::Blob => "blob",
        gix::object::Kind::Tree => "tree",
        gix::object::Kind::Commit => "commit",
        gix::object::Kind::Tag => "tag",
    }
}

impl ObjectStore for GitObjectStore {
    fn object_hash(&self) -> ObjectHash {
        ObjectHash::Sha1
    }

    fn write_blob(&self, data: &[u8]) -> Result<GitOid, StoreError> {
        let id = self.repository.local().write_blob(data).map(|id| id.detach());
        from_gix_oid(id.map_err(|error| StoreError::backend("write blob", error))?)
    }

    fn read_blob(&self, oid: &GitOid) -> Result<Vec<u8>, StoreError> {
        self.read(oid, gix::object::Kind::Blob, Some(MAX_BLOB_BYTES))
    }

    fn write_tree(&self, entries: &[TreeEntry]) -> Result<GitOid, StoreError> {
        let entries = sorted_tree_entries(entries)?
            .into_iter()
            .map(|entry| {
                let mode = match entry.mode {
                    EntryMode::Blob => EntryKind::Blob,
                    EntryMode::Tree => EntryKind::Tree,
                };
                Ok(gix::objs::tree::Entry {
                    mode: mode.into(),
                    filename: entry.name.into(),
                    oid: to_gix_oid(&entry.oid)?,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        let tree = gix::objs::Tree { entries };
        let id = self.repository.local().write_object(&tree).map(|id| id.detach());
        from_gix_oid(id.map_err(|error| StoreError::backend("write tree", error))?)
    }

    fn read_tree(&self, oid: &GitOid) -> Result<Vec<TreeEntry>, StoreError> {
        let data = self.read(oid, gix::object::Kind::Tree, None)?;
        let tree = gix::objs::TreeRef::from_bytes(&data, gix::hash::Kind::Sha1)
            .map_err(|error| corrupted(oid, format!("malformed tree: {error}")))?;
        tree.entries
            .iter()
            .map(|entry| {
                let mode = match entry.mode.kind() {
                    EntryKind::Blob => EntryMode::Blob,
                    EntryKind::Tree => EntryMode::Tree,
                    other => return Err(corrupted(oid, format!("unsupported entry mode {other:?}"))),
                };
                let name = entry
                    .filename
                    .to_str()
                    .map_err(|_| corrupted(oid, "entry name is not UTF-8"))?;
                Ok(TreeEntry {
                    name: name.to_owned(),
                    mode,
                    oid: from_gix_oid(entry.oid.to_owned())?,
                })
            })
            .collect()
    }

    fn write_commit(&self, commit: &NewCommit) -> Result<GitOid, StoreError> {
        let signature = self.repository.signature()?;
        let parents = commit.parents.iter().map(to_gix_oid).collect::<Result<_, _>>()?;
        let object = gix::objs::Commit {
            tree: to_gix_oid(&commit.tree)?,
            parents,
            author: signature.clone(),
            committer: signature,
            encoding: None,
            message: commit.message.as_str().into(),
            extra_headers: Vec::new(),
        };
        let id = self
            .repository
            .local()
            .write_object(&object)
            .map(|id| id.detach());
        from_gix_oid(id.map_err(|error| StoreError::backend("write commit", error))?)
    }

    fn read_commit(&self, oid: &GitOid) -> Result<CommitInfo, StoreError> {
        let data = self.read(oid, gix::object::Kind::Commit, None)?;
        let commit = gix::objs::CommitRef::from_bytes(&data, gix::hash::Kind::Sha1)
            .map_err(|error| corrupted(oid, format!("malformed commit: {error}")))?;
        let message = commit
            .message
            .to_str()
            .map_err(|_| corrupted(oid, "message is not UTF-8"))?;
        Ok(CommitInfo {
            tree: from_gix_oid(commit.tree())?,
            parents: commit.parents().map(from_gix_oid).collect::<Result<_, _>>()?,
            message: message.to_owned(),
        })
    }
}

fn corrupted(oid: &GitOid, reason: impl Into<String>) -> StoreError {
    StoreError::Corrupted {
        path: format!("object {oid}"),
        offset: None,
        reason: reason.into(),
    }
}
