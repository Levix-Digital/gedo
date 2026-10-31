//! Commit generations kept in the cache and the generation-bounded search for detached tapes
//! (User Stories 1 and 2 of `specs/008-bounded-detached-walk`).

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use gedo_app::EventLog;
use gedo_app::events::generations::encode_generation;
use gedo_app::events::records::{
    self, COMMIT_TAPES_COLLECTION, GENERATION_FORMAT, GENERATIONS_CACHE_NAMESPACE,
};
use gedo_core::event::{Labels, SemanticOp, TapeId};
use gedo_core::identity::{AstNodeId, SemanticDelta};
use gedo_core::ids::{GedoRefName, GitOid, ObjectHash};
use gedo_core::memory::{MemoryCache, MemoryObjectStore, MemoryRefStore, MemoryWal};
use gedo_core::ports::{
    CacheEntry, CacheRebuildSource, CacheStatus, CommitInfo, NewCommit, ObjectStore, SymbolCache, TreeEntry,
};
use gedo_core::syntax::StructuralHash;
use gedo_core::{Collection, StoreError};
use gedo_crdt::RecordingContext;
use proptest::prelude::*;

struct Empty;

impl CacheRebuildSource for Empty {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
}

fn cache() -> MemoryCache {
    MemoryCache::open(&Empty).expect("cache opens")
}

/// An object store that counts the commits read.
#[derive(Default)]
struct Counting {
    inner: MemoryObjectStore,
    commits_read: Cell<usize>,
}

impl Counting {
    fn take(&self) -> usize {
        self.commits_read.replace(0)
    }
}

impl ObjectStore for Counting {
    fn object_hash(&self) -> ObjectHash {
        self.inner.object_hash()
    }
    fn write_blob(&self, data: &[u8]) -> Result<GitOid, StoreError> {
        self.inner.write_blob(data)
    }
    fn read_blob(&self, oid: &GitOid) -> Result<Vec<u8>, StoreError> {
        self.inner.read_blob(oid)
    }
    fn write_tree(&self, entries: &[TreeEntry]) -> Result<GitOid, StoreError> {
        self.inner.write_tree(entries)
    }
    fn read_tree(&self, oid: &GitOid) -> Result<Vec<TreeEntry>, StoreError> {
        self.inner.read_tree(oid)
    }
    fn write_commit(&self, commit: &NewCommit) -> Result<GitOid, StoreError> {
        self.inner.write_commit(commit)
    }
    fn read_commit(&self, oid: &GitOid) -> Result<CommitInfo, StoreError> {
        self.commits_read.set(self.commits_read.get() + 1);
        self.inner.read_commit(oid)
    }
}

/// A cache that fails every call.
struct Failing(CacheStatus);

impl SymbolCache for Failing {
    fn open_status(&self) -> &CacheStatus {
        &self.0
    }
    fn get(&self, _: &str, _: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        Err(StoreError::invalid_input("cache", "unavailable"))
    }
    fn put(&self, _: &str, _: &[(Vec<u8>, Vec<u8>)]) -> Result<(), StoreError> {
        Err(StoreError::invalid_input("cache", "unavailable"))
    }
    fn clear(&self) -> Result<(), StoreError> {
        Err(StoreError::invalid_input("cache", "unavailable"))
    }
}

/// A repository of empty-tree commits with Gedo references and a write-ahead log.
struct Repo {
    objects: Counting,
    refs: MemoryRefStore,
    wal: MemoryWal,
    cache: MemoryCache,
    tree: GitOid,
    next: u32,
}

impl Repo {
    fn new() -> Self {
        let objects = Counting::default();
        let tree = objects.write_tree(&[]).expect("tree");
        Self {
            objects,
            refs: MemoryRefStore::new(),
            wal: MemoryWal::new(),
            cache: cache(),
            tree,
            next: 0,
        }
    }

    fn commit(&mut self, parents: &[GitOid]) -> GitOid {
        self.next += 1;
        self.objects
            .write_commit(&NewCommit {
                tree: self.tree,
                parents: parents.to_vec(),
                message: format!("c{}", self.next),
            })
            .expect("commit")
    }

    /// A chain of `length` commits on `parent` (none: a new root); the last one.
    fn chain(&mut self, parent: Option<GitOid>, length: usize) -> Vec<GitOid> {
        let mut commits = Vec::with_capacity(length);
        let mut parent = parent;
        for _ in 0..length {
            let commit = self.commit(parent.as_slice());
            commits.push(commit);
            parent = Some(commit);
        }
        commits
    }

    /// Publishes one live event recorded on the first parent of `commit` as its tape.
    fn publish(&mut self, commit: GitOid) -> TapeId {
        self.next += 1;
        let parent = self
            .objects
            .inner
            .read_commit(&commit)
            .expect("commit")
            .parents
            .first()
            .copied();
        let mut after = [0u8; 32];
        after[..4].copy_from_slice(&self.next.to_le_bytes());
        let op = (
            SemanticOp::Delta(SemanticDelta::BodyModified {
                id: AstNodeId([5; 16]),
                before: StructuralHash([0; 32]),
                after: StructuralHash(after),
            }),
            Some(StructuralHash(after)),
        );
        let person = RecordingContext::person("levi@example.com", Labels::new()).expect("valid");
        let mut log = EventLog::open(&self.objects, &self.refs, &mut self.wal).expect("opens");
        log.record(&person, parent, &[op]).expect("recorded");
        log.publish(commit).expect("published").expect("a tape")
    }

    fn generation(&mut self, commit: &GitOid, cache: Option<&dyn SymbolCache>) -> u64 {
        let mut log = EventLog::open(&self.objects, &self.refs, &mut self.wal).expect("opens");
        if let Some(cache) = cache {
            log = log.with_cache(cache);
        }
        log.generation(commit).expect("generation")
    }

    /// The detached tapes and their commits, through a new log on the repository's cache.
    fn detached(&mut self, tips: &[GitOid]) -> BTreeMap<TapeId, Vec<GitOid>> {
        let mut log = EventLog::open(&self.objects, &self.refs, &mut self.wal)
            .expect("opens")
            .with_cache(&self.cache);
        log.detached(tips, 1_000, 90)
            .expect("detached")
            .into_iter()
            .map(|tape| (tape.tape, tape.commits))
            .collect()
    }

    /// Adds an index entry of `tape` for `commit`, which may be missing from the store.
    fn index(&self, commit: GitOid, tape: TapeId) {
        let name = GedoRefName::collection(COMMIT_TAPES_COLLECTION).expect("valid");
        let collection = Collection::new(name, &self.objects, &self.refs);
        collection
            .put(
                &[(
                    records::index_entry(&commit, &tape).expect("entry"),
                    records::encode_index_entry().expect("encoded"),
                )],
                "test: index entry",
            )
            .expect("stored");
    }

    /// The commit index, by tape.
    fn index_map(&self) -> BTreeMap<TapeId, Vec<GitOid>> {
        let name = GedoRefName::collection(COMMIT_TAPES_COLLECTION).expect("valid");
        let collection = Collection::new(name, &self.objects, &self.refs);
        let mut index: BTreeMap<TapeId, Vec<GitOid>> = BTreeMap::new();
        for entry in collection.entries().unwrap_or_default().keys() {
            let (commit, tape) = records::parse_index_entry(entry).expect("valid");
            index.entry(tape).or_default().push(commit);
        }
        index
    }

    /// The detached tapes found by walking every ancestor of `tips` (the search before IMP-006).
    fn reference_detached(&self, tips: &[GitOid]) -> BTreeMap<TapeId, Vec<GitOid>> {
        let mut reachable = HashSet::new();
        let mut stack = tips.to_vec();
        while let Some(commit) = stack.pop() {
            if reachable.insert(commit) {
                stack.extend(self.objects.inner.read_commit(&commit).expect("commit").parents);
            }
        }
        self.index_map()
            .into_iter()
            .filter(|(_, commits)| !commits.iter().any(|commit| reachable.contains(commit)))
            .collect()
    }
}

fn missing_commit() -> GitOid {
    GitOid::from_hex("0123456789abcdef0123456789abcdef01234567").expect("valid")
}

#[test]
fn generations_follow_the_commit_graph() {
    let mut repo = Repo::new();
    let root = repo.commit(&[]);
    let left = repo.chain(Some(root), 3);
    let right = repo.chain(Some(root), 1);
    let merge = repo.commit(&[right[0], left[2]]);
    assert_eq!(repo.generation(&root, None), 1);
    assert_eq!(repo.generation(&left[2], None), 4);
    assert_eq!(repo.generation(&merge, None), 5);
}

#[test]
fn a_new_log_reads_generations_from_the_cache() {
    let mut repo = Repo::new();
    let cache = cache();
    let chain = repo.chain(None, 200);
    assert_eq!(repo.generation(&chain[199], Some(&cache)), 200);
    assert_eq!(repo.objects.take(), 200);

    // Another process: the generation is read back, checked against its parent's.
    assert_eq!(repo.generation(&chain[199], Some(&cache)), 200);
    assert_eq!(repo.objects.take(), 1);

    // New commits on top: only they are read before a cached generation bounds the walk.
    let more = repo.chain(Some(chain[199]), 3);
    assert_eq!(repo.generation(&more[2], Some(&cache)), 203);
    assert!(repo.objects.take() <= 4);

    // Without the cache, everything is read again, with the same result.
    assert_eq!(repo.generation(&more[2], None), 203);
    assert_eq!(repo.objects.take(), 203);
}

#[test]
fn a_cleared_cache_gives_the_same_generations() {
    let mut repo = Repo::new();
    let cache = cache();
    let chain = repo.chain(None, 20);
    assert_eq!(repo.generation(&chain[19], Some(&cache)), 20);
    cache.clear().expect("cleared");
    assert_eq!(repo.generation(&chain[19], Some(&cache)), 20);
    assert_eq!(repo.objects.take(), 40);
}

#[test]
fn bad_generation_records_are_refused_and_rewritten() {
    for bad in [
        encode_generation(7),
        vec![GENERATION_FORMAT, 1, 0, 0],
        {
            let mut other = encode_generation(10);
            other[0] = GENERATION_FORMAT + 1;
            other
        },
        encode_generation(0),
    ] {
        let mut repo = Repo::new();
        let cache = cache();
        let chain = repo.chain(None, 10);
        assert_eq!(repo.generation(&chain[9], Some(&cache)), 10);
        cache
            .put(
                GENERATIONS_CACHE_NAMESPACE,
                &[(chain[9].as_bytes().to_vec(), bad.clone())],
            )
            .expect("stored");
        assert_eq!(repo.generation(&chain[9], Some(&cache)), 10, "{bad:?}");
        assert_eq!(
            cache
                .get(GENERATIONS_CACHE_NAMESPACE, chain[9].as_bytes())
                .expect("read"),
            Some(encode_generation(10)),
            "{bad:?}"
        );
    }
}

#[test]
fn a_failing_cache_only_costs_a_walk() {
    let mut repo = Repo::new();
    let failing = Failing(CacheStatus::Ready);
    let chain = repo.chain(None, 30);
    assert_eq!(repo.generation(&chain[29], Some(&failing)), 30);
    assert_eq!(repo.detached(&[chain[29]]).len(), 0);
}

/// A deep history without Gedo data, 20 commits with tapes on top and one tape on a side branch.
fn deep(repo: &mut Repo) -> (GitOid, TapeId, GitOid) {
    let old = repo.chain(None, 500);
    let recent = repo.chain(Some(old[499]), 20);
    for commit in &recent {
        repo.publish(*commit);
    }
    let side = repo.commit(&[recent[9]]);
    let tape = repo.publish(side);
    (recent[19], tape, side)
}

#[test]
fn the_search_stops_below_the_lowest_unseen_indexed_commit() {
    let mut repo = Repo::new();
    let (tip, tape, side) = deep(&mut repo);
    assert_eq!(repo.detached(&[tip]), BTreeMap::from([(tape, vec![side])]));
    repo.objects.take();
    assert_eq!(repo.detached(&[tip]), BTreeMap::from([(tape, vec![side])]));
    let read = repo.objects.take();
    assert!(read < 100, "{read} commits read on a history of 541");
}

#[test]
fn a_reachable_side_branch_leaves_nothing_detached() {
    let mut repo = Repo::new();
    let (tip, _, side) = deep(&mut repo);
    let merge = repo.commit(&[tip, side]);
    assert!(repo.detached(&[merge]).is_empty());
    assert!(repo.detached(&[tip, side]).is_empty());
}

#[test]
fn an_indexed_commit_missing_from_the_store_is_unreachable() {
    let mut repo = Repo::new();
    let (tip, tape, side) = deep(&mut repo);
    repo.index(missing_commit(), tape);
    let detached = repo.detached(&[tip]);
    assert_eq!(detached.len(), 1);
    assert_eq!(
        detached[&tape].iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([&side, &missing_commit()])
    );
}

#[test]
fn no_index_means_no_walk() {
    let mut repo = Repo::new();
    let chain = repo.chain(None, 50);
    repo.objects.take();
    assert!(repo.detached(&[chain[49]]).is_empty());
}

#[test]
fn a_missing_tip_is_an_error() {
    let mut repo = Repo::new();
    let (_, _, _) = deep(&mut repo);
    let mut log = EventLog::open(&repo.objects, &repo.refs, &mut repo.wal)
        .expect("opens")
        .with_cache(&repo.cache);
    assert!(log.detached(&[missing_commit()], 1_000, 90).is_err());
}

/// A generated history: each commit has up to two parents among the earlier ones (none: a root).
fn generated(repo: &mut Repo, shape: &[(u8, u8, u8)]) -> Vec<GitOid> {
    let mut commits: Vec<GitOid> = Vec::new();
    for (index, (parents, first, second)) in shape.iter().enumerate() {
        let parents: Vec<GitOid> = if index == 0 {
            Vec::new()
        } else {
            let mut chosen = vec![commits[usize::from(*first) % index]];
            if *parents >= 2 {
                let other = commits[usize::from(*second) % index];
                if other != chosen[0] {
                    chosen.push(other);
                }
            }
            if *parents == 0 && index % 7 == 3 {
                Vec::new()
            } else {
                chosen
            }
        };
        let commit = repo.commit(&parents);
        commits.push(commit);
    }
    commits
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn the_bounded_search_finds_what_the_full_walk_finds(
        shape in prop::collection::vec((0u8..3, any::<u8>(), any::<u8>()), 1..30),
        published in prop::collection::vec(any::<u8>(), 0..8),
        tips in prop::collection::vec(any::<u8>(), 1..3),
        missing in any::<bool>(),
    ) {
        let mut repo = Repo::new();
        let commits = generated(&mut repo, &shape);
        let mut tapes = Vec::new();
        for pick in published.iter().collect::<BTreeSet<_>>() {
            let commit = commits[usize::from(*pick) % commits.len()];
            if repo.index_map().values().flatten().any(|indexed| *indexed == commit) {
                continue;
            }
            tapes.push(repo.publish(commit));
        }
        if missing && let Some(tape) = tapes.first() {
            repo.index(missing_commit(), *tape);
        }
        let tips: Vec<GitOid> = tips.iter().map(|pick| commits[usize::from(*pick) % commits.len()]).collect();
        let expected = repo.reference_detached(&tips);
        let normalize = |map: BTreeMap<TapeId, Vec<GitOid>>| -> BTreeMap<TapeId, BTreeSet<GitOid>> {
            map.into_iter().map(|(tape, commits)| (tape, commits.into_iter().collect())).collect()
        };
        prop_assert_eq!(normalize(repo.detached(&tips)), normalize(expected.clone()));
        // Again with every generation cached.
        prop_assert_eq!(normalize(repo.detached(&tips)), normalize(expected));
    }

    #[test]
    fn cached_generations_equal_computed_ones(
        shape in prop::collection::vec((0u8..3, any::<u8>(), any::<u8>()), 1..40),
        order in prop::collection::vec(any::<u8>(), 1..10),
    ) {
        let mut repo = Repo::new();
        let commits = generated(&mut repo, &shape);
        let cache = cache();
        for pick in order {
            let commit = commits[usize::from(pick) % commits.len()];
            let cached = repo.generation(&commit, Some(&cache));
            prop_assert_eq!(cached, repo.generation(&commit, None));
        }
        for commit in &commits {
            let cached = repo.generation(commit, Some(&cache));
            prop_assert_eq!(cached, repo.generation(commit, None));
        }
    }
}
