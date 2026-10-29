use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;

use super::lock;
use crate::error::StoreError;
use crate::ids::{GitOid, ObjectHash};
use crate::ports::object::{MAX_BLOB_BYTES, sorted_tree_entries};
use crate::ports::{CommitInfo, NewCommit, ObjectStore, TreeEntry};

#[derive(Clone, Debug, Hash)]
enum StoredObject {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
    Commit {
        tree: GitOid,
        parents: Vec<GitOid>,
        message: String,
        author_email: String,
    },
}

impl StoredObject {
    fn kind(&self) -> &'static str {
        match self {
            StoredObject::Blob(_) => "blob",
            StoredObject::Tree(_) => "tree",
            StoredObject::Commit { .. } => "commit",
        }
    }
}

/// An object database in memory.
///
/// Ids are deterministic hashes of the content, not Git's SHA-1 ids, but have the same
/// properties: equal content gives equal ids, and commits are never deduplicated by accident
/// because each carries a sequence number like a real commit carries a timestamp.
#[derive(Debug)]
pub struct MemoryObjectStore {
    objects: Mutex<HashMap<GitOid, StoredObject>>,
    has_identity: bool,
    commits_written: Mutex<u64>,
    author_email: Mutex<String>,
}

/// The author e-mail of commits written by a [`MemoryObjectStore`] until another is set.
pub const MEMORY_AUTHOR_EMAIL: &str = "gedo@memory.invalid";

impl Default for MemoryObjectStore {
    fn default() -> Self {
        MemoryObjectStore::new()
    }
}

impl MemoryObjectStore {
    /// An empty store whose repository has a user identity.
    pub fn new() -> Self {
        MemoryObjectStore {
            objects: Mutex::new(HashMap::new()),
            has_identity: true,
            commits_written: Mutex::new(0),
            author_email: Mutex::new(MEMORY_AUTHOR_EMAIL.to_owned()),
        }
    }

    /// An empty store whose repository has no user identity, so commits are refused.
    pub fn without_identity() -> Self {
        MemoryObjectStore {
            has_identity: false,
            ..MemoryObjectStore::new()
        }
    }

    /// Sets the author e-mail of the commits written from now on.
    pub fn set_author_email(&self, email: &str) {
        *lock(&self.author_email) = email.to_owned();
    }

    /// Number of objects stored.
    pub fn len(&self) -> usize {
        lock(&self.objects).len()
    }

    /// Whether the store holds no object.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn insert(&self, object: StoredObject, salt: u64) -> GitOid {
        let oid = content_id(&object, salt);
        lock(&self.objects).entry(oid).or_insert(object);
        oid
    }

    fn get(&self, oid: &GitOid, expected: &'static str) -> Result<StoredObject, StoreError> {
        match lock(&self.objects).get(oid) {
            Some(object) if object.kind() == expected => Ok(object.clone()),
            _ => Err(StoreError::ObjectNotFound { oid: *oid, expected }),
        }
    }
}

fn content_id(object: &StoredObject, salt: u64) -> GitOid {
    let mut bytes = Vec::with_capacity(24);
    for seed in 0u64..3 {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        salt.hash(&mut hasher);
        object.hash(&mut hasher);
        bytes.extend_from_slice(&hasher.finish().to_be_bytes());
    }
    bytes.truncate(ObjectHash::Sha1.len_bytes());
    GitOid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a valid SHA-1 id"))
}

impl ObjectStore for MemoryObjectStore {
    fn object_hash(&self) -> ObjectHash {
        ObjectHash::Sha1
    }

    fn write_blob(&self, data: &[u8]) -> Result<GitOid, StoreError> {
        Ok(self.insert(StoredObject::Blob(data.to_vec()), 0))
    }

    fn read_blob(&self, oid: &GitOid) -> Result<Vec<u8>, StoreError> {
        let StoredObject::Blob(data) = self.get(oid, "blob")? else {
            unreachable!("kind checked by get")
        };
        if data.len() > MAX_BLOB_BYTES {
            return Err(StoreError::TooLarge {
                what: format!("blob {oid}"),
                size: data.len() as u64,
                limit: MAX_BLOB_BYTES as u64,
            });
        }
        Ok(data)
    }

    fn write_tree(&self, entries: &[TreeEntry]) -> Result<GitOid, StoreError> {
        Ok(self.insert(StoredObject::Tree(sorted_tree_entries(entries)?), 0))
    }

    fn read_tree(&self, oid: &GitOid) -> Result<Vec<TreeEntry>, StoreError> {
        let StoredObject::Tree(entries) = self.get(oid, "tree")? else {
            unreachable!("kind checked by get")
        };
        Ok(entries)
    }

    fn write_commit(&self, commit: &NewCommit) -> Result<GitOid, StoreError> {
        if !self.has_identity {
            return Err(StoreError::MissingIdentity {
                repository: "memory".into(),
            });
        }
        let sequence = {
            let mut written = lock(&self.commits_written);
            *written += 1;
            *written
        };
        let object = StoredObject::Commit {
            tree: commit.tree,
            parents: commit.parents.clone(),
            message: commit.message.clone(),
            author_email: lock(&self.author_email).clone(),
        };
        Ok(self.insert(object, sequence))
    }

    fn read_commit(&self, oid: &GitOid) -> Result<CommitInfo, StoreError> {
        let StoredObject::Commit {
            tree,
            parents,
            message,
            author_email,
        } = self.get(oid, "commit")?
        else {
            unreachable!("kind checked by get")
        };
        Ok(CommitInfo {
            tree,
            parents,
            message,
            author_email,
        })
    }
}
