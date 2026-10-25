//! Builds Git histories in memory and indexes them.
#![allow(dead_code)]

use std::collections::BTreeMap;

use gedo_app::IdentityIndex;
use gedo_core::StoreError;
use gedo_core::identity::AstNodeId;
use gedo_core::ids::GitOid;
use gedo_core::memory::{MemoryCache, MemoryObjectStore};
use gedo_core::ports::{CacheEntry, CacheRebuildSource, EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_parse::TreeSitterParser;

/// A cache source with nothing to rebuild from.
pub struct Empty;

impl CacheRebuildSource for Empty {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
}

pub fn cache() -> MemoryCache {
    MemoryCache::open(&Empty).expect("cache opens")
}

/// A repository's history built file by file.
pub struct History {
    pub objects: MemoryObjectStore,
    pub parser: TreeSitterParser,
}

impl Default for History {
    fn default() -> Self {
        Self {
            objects: MemoryObjectStore::new(),
            parser: TreeSitterParser::new(),
        }
    }
}

impl History {
    /// A commit whose tree holds exactly `files`.
    pub fn commit(&self, parents: &[GitOid], files: &BTreeMap<String, String>) -> GitOid {
        let tree = self.tree(
            files
                .iter()
                .map(|(path, text)| (path.as_str(), text.as_str()))
                .collect(),
        );
        self.objects
            .write_commit(&NewCommit {
                tree,
                parents: parents.to_vec(),
                message: "commit".into(),
            })
            .expect("commit is written")
    }

    fn tree(&self, files: Vec<(&str, &str)>) -> GitOid {
        let mut directories: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
        let mut entries = Vec::new();
        for (path, text) in files {
            match path.split_once('/') {
                Some((directory, rest)) => directories.entry(directory).or_default().push((rest, text)),
                None => entries.push(TreeEntry {
                    name: path.into(),
                    mode: EntryMode::Blob,
                    oid: self.objects.write_blob(text.as_bytes()).expect("blob is written"),
                }),
            }
        }
        for (directory, children) in directories {
            entries.push(TreeEntry {
                name: directory.into(),
                mode: EntryMode::Tree,
                oid: self.tree(children),
            });
        }
        self.objects.write_tree(&entries).expect("tree is written")
    }

    /// Indexes `tips` in a new cache and returns it.
    pub fn index(&self, tips: &[&[GitOid]]) -> MemoryCache {
        let mut cache = cache();
        for group in tips {
            let mut index =
                IdentityIndex::open(&self.objects, &self.parser, &mut cache).expect("index opens");
            index.index_history(group).expect("history is indexed");
        }
        cache
    }

    /// The id of `qualified_path` in `path` at `commit`.
    pub fn id(
        &self,
        cache: &mut MemoryCache,
        commit: &GitOid,
        path: &str,
        qualified_path: &str,
    ) -> Option<AstNodeId> {
        let index = IdentityIndex::open(&self.objects, &self.parser, cache).expect("index opens");
        index
            .map_at(commit, path)
            .expect("commit is indexed")
            .and_then(|map| {
                map.nodes
                    .into_iter()
                    .find(|node| node.qualified_path == qualified_path && !node.is_detached())
            })
            .map(|node| node.id)
    }

    /// Everything the index knows about `commits`, as text: the maps of every file and the changes.
    pub fn render(&self, cache: &mut MemoryCache, commits: &[GitOid]) -> String {
        let index = IdentityIndex::open(&self.objects, &self.parser, cache).expect("index opens");
        let mut text = String::new();
        for commit in commits {
            text.push_str(&format!("commit {commit}\n"));
            for path in index.files_at(commit).expect("indexed") {
                let map = index
                    .map_at(commit, &path)
                    .expect("indexed")
                    .expect("file exists");
                for node in &map.nodes {
                    text.push_str(&format!("  {path} {} {}\n", node.qualified_path, node.id));
                }
            }
            let changes = index.changes_of(commit).expect("indexed");
            for delta in &changes.deltas {
                text.push_str(&format!("  {delta:?}\n"));
            }
            for merge in &changes.merges {
                text.push_str(&format!("  {merge:?}\n"));
            }
        }
        text
    }
}

/// Files from `(path, text)` pairs.
pub fn files(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|&(path, text)| (path.to_owned(), text.to_owned()))
        .collect()
}
