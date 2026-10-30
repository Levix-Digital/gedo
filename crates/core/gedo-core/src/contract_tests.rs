//! The contract every adapter of a port must pass, run against the in-memory adapters here and
//! against the real adapters in `gedo-store`.
//!
//! Each check is a public function taking a factory of fresh, empty stores; the macros
//! [`object_store_contract!`](crate::object_store_contract), [`ref_store_contract!`](crate::ref_store_contract),
//! [`wal_contract!`](crate::wal_contract), [`cache_contract!`](crate::cache_contract) and
//! [`collection_contract!`](crate::collection_contract) turn every
//! check into its own `#[test]`.

use crate::collection::{Collection, MAX_UPDATE_ATTEMPTS, MergeOutcome};
use crate::error::StoreError;
use crate::ids::{EntryId, GedoRefName, GitOid, ObjectHash};
use crate::ports::{
    CacheEntry, CacheRebuildSource, CacheStatus, EntryMode, Expected, LogSequence, NewCommit, ObjectStore,
    RebuildReason, RefOutcome, RefStore, RefUpdate, SymbolCache, TreeEntry, WriteAheadLog,
};

/// An id no store holds.
fn absent_oid(hash: ObjectHash) -> GitOid {
    GitOid::from_bytes(&vec![0xee; hash.len_bytes()]).expect("valid length")
}

fn name(text: &str) -> GedoRefName {
    GedoRefName::new(text).expect("valid name")
}

fn commit<O: ObjectStore>(objects: &O, message: &str) -> GitOid {
    let blob = objects.write_blob(message.as_bytes()).expect("blob written");
    let tree = objects
        .write_tree(&[TreeEntry {
            name: "entry".into(),
            mode: EntryMode::Blob,
            oid: blob,
        }])
        .expect("tree written");
    objects
        .write_commit(&NewCommit {
            tree,
            parents: vec![],
            message: message.into(),
        })
        .expect("commit written")
}

fn update(name: &GedoRefName, expected: Expected, new: GitOid) -> RefUpdate {
    RefUpdate {
        name: name.clone(),
        expected,
        new,
        log_message: "gedo: contract test".into(),
    }
}

// --- ObjectStore -------------------------------------------------------------------------------

/// Blobs round trip, and equal content gets equal ids.
pub fn blobs_round_trip<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    assert_eq!(objects.object_hash(), ObjectHash::Sha1);
    let first = objects.write_blob(b"hello").expect("written");
    assert_eq!(first.object_hash(), ObjectHash::Sha1);
    assert_eq!(objects.read_blob(&first).expect("read"), b"hello");
    assert_eq!(
        objects.write_blob(b"hello").expect("written"),
        first,
        "same content, same id"
    );
    assert_ne!(objects.write_blob(b"other").expect("written"), first);
    let empty = objects.write_blob(b"").expect("written");
    assert_eq!(objects.read_blob(&empty).expect("read"), b"");
}

/// A batch of blobs gets the ids single writes give, in order, with repeated content allowed;
/// sizes around a store's packing threshold are covered by `sizes`.
pub fn blob_batches_round_trip<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    assert_eq!(objects.write_blobs(&[]).expect("written"), Vec::<GitOid>::new());
    for size in [1, 99, 100, 101, 250] {
        let contents: Vec<Vec<u8>> = (0..size)
            .map(|n| format!("batch of {size}, blob {}", n % 97).into_bytes())
            .collect();
        let batch: Vec<&[u8]> = contents.iter().map(Vec::as_slice).collect();
        let ids = objects.write_blobs(&batch).expect("written");
        assert_eq!(ids.len(), size);
        for (id, content) in ids.iter().zip(&contents) {
            assert_eq!(objects.read_blob(id).expect("read"), *content);
            assert_eq!(
                objects.write_blob(content).expect("written"),
                *id,
                "same id as a single write"
            );
        }
        assert_eq!(objects.write_blobs(&batch).expect("written again"), ids);
    }
}

/// Reading an absent object, or an object of another kind, is `ObjectNotFound`.
pub fn missing_objects_are_not_found<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    let absent = absent_oid(objects.object_hash());
    assert!(matches!(
        objects.read_blob(&absent),
        Err(StoreError::ObjectNotFound { .. })
    ));
    assert!(matches!(
        objects.read_tree(&absent),
        Err(StoreError::ObjectNotFound { .. })
    ));
    assert!(matches!(
        objects.read_commit(&absent),
        Err(StoreError::ObjectNotFound { .. })
    ));
    let blob = objects.write_blob(b"a blob").expect("written");
    assert!(matches!(
        objects.read_tree(&blob),
        Err(StoreError::ObjectNotFound { .. })
    ));
    assert!(matches!(
        objects.read_commit(&blob),
        Err(StoreError::ObjectNotFound { .. })
    ));
}

/// Trees come back in Git's order, whatever order they were written in.
pub fn trees_round_trip_in_git_order<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    let blob = objects.write_blob(b"x").expect("written");
    let subtree = objects
        .write_tree(&[TreeEntry {
            name: "leaf".into(),
            mode: EntryMode::Blob,
            oid: blob,
        }])
        .expect("written");
    let entries = vec![
        TreeEntry {
            name: "b".into(),
            mode: EntryMode::Blob,
            oid: blob,
        },
        TreeEntry {
            name: "a".into(),
            mode: EntryMode::Tree,
            oid: subtree,
        },
        TreeEntry {
            name: "a.txt".into(),
            mode: EntryMode::Blob,
            oid: blob,
        },
    ];
    let tree = objects.write_tree(&entries).expect("written");
    let names: Vec<String> = objects
        .read_tree(&tree)
        .expect("read")
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(names, ["a.txt", "a", "b"]);

    let mut reversed = entries.clone();
    reversed.reverse();
    assert_eq!(
        objects.write_tree(&reversed).expect("written"),
        tree,
        "order does not change the id"
    );

    let read_back = objects.read_tree(&tree).expect("read");
    let subtree_entry = read_back.iter().find(|entry| entry.name == "a").expect("present");
    assert_eq!(
        (subtree_entry.mode, subtree_entry.oid),
        (EntryMode::Tree, subtree)
    );
    let empty = objects.write_tree(&[]).expect("written");
    assert!(objects.read_tree(&empty).expect("read").is_empty());
}

/// Executable files, symbolic links and submodules of source histories keep their mode.
pub fn source_entry_modes_round_trip<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    let blob = objects.write_blob(b"#!/bin/sh").expect("written");
    let submodule = commit(&objects, "submodule");
    let entries = vec![
        TreeEntry {
            name: "build.sh".into(),
            mode: EntryMode::ExecutableBlob,
            oid: blob,
        },
        TreeEntry {
            name: "link".into(),
            mode: EntryMode::Link,
            oid: blob,
        },
        TreeEntry {
            name: "vendor".into(),
            mode: EntryMode::Submodule,
            oid: submodule,
        },
    ];
    let tree = objects.write_tree(&entries).expect("written");
    let modes: Vec<(String, EntryMode)> = objects
        .read_tree(&tree)
        .expect("read")
        .into_iter()
        .map(|entry| (entry.name, entry.mode))
        .collect();
    assert_eq!(
        modes,
        [
            ("build.sh".to_owned(), EntryMode::ExecutableBlob),
            ("link".to_owned(), EntryMode::Link),
            ("vendor".to_owned(), EntryMode::Submodule),
        ]
    );
}

/// Invalid and duplicate tree entry names are refused.
pub fn invalid_tree_entries_are_refused<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    let blob = objects.write_blob(b"x").expect("written");
    for bad in ["", ".", "..", "a/b"] {
        let entries = [TreeEntry {
            name: bad.into(),
            mode: EntryMode::Blob,
            oid: blob,
        }];
        assert!(objects.write_tree(&entries).is_err(), "{bad:?} must be refused");
    }
    let duplicate = [
        TreeEntry {
            name: "same".into(),
            mode: EntryMode::Blob,
            oid: blob,
        },
        TreeEntry {
            name: "same".into(),
            mode: EntryMode::Blob,
            oid: blob,
        },
    ];
    assert!(objects.write_tree(&duplicate).is_err());
}

/// Commits round trip with their tree, parents and message, and carry the author's e-mail.
pub fn commits_round_trip<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    let root = commit(&objects, "gedo: first");
    let info = objects.read_commit(&root).expect("read");
    assert!(info.parents.is_empty());
    assert_eq!(info.message, "gedo: first");
    assert!(info.author_email.contains('@'), "{:?}", info.author_email);

    let tree = objects.write_tree(&[]).expect("written");
    let other = commit(&objects, "gedo: other");
    let merge = objects
        .write_commit(&NewCommit {
            tree,
            parents: vec![root, other],
            message: "gedo: merge\n\nbody\n".into(),
        })
        .expect("written");
    let info = objects.read_commit(&merge).expect("read");
    assert_eq!(info.tree, tree);
    assert_eq!(info.parents, vec![root, other]);
    assert_eq!(info.message, "gedo: merge\n\nbody\n");
}

/// Runs every object store check as its own test.
#[macro_export]
macro_rules! object_store_contract {
    ($make:expr) => {
        #[test]
        fn blobs_round_trip() {
            $crate::contract_tests::blobs_round_trip($make);
        }
        #[test]
        fn blob_batches_round_trip() {
            $crate::contract_tests::blob_batches_round_trip($make);
        }
        #[test]
        fn missing_objects_are_not_found() {
            $crate::contract_tests::missing_objects_are_not_found($make);
        }
        #[test]
        fn trees_round_trip_in_git_order() {
            $crate::contract_tests::trees_round_trip_in_git_order($make);
        }
        #[test]
        fn source_entry_modes_round_trip() {
            $crate::contract_tests::source_entry_modes_round_trip($make);
        }
        #[test]
        fn invalid_tree_entries_are_refused() {
            $crate::contract_tests::invalid_tree_entries_are_refused($make);
        }
        #[test]
        fn commits_round_trip() {
            $crate::contract_tests::commits_round_trip($make);
        }
    };
}

// --- RefStore ----------------------------------------------------------------------------------

/// Creating, updating and reading a reference with compare-and-swap.
pub fn compare_and_swap_updates<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let tapes = name("refs/gedo/tapes");
    let (first, second) = (commit(&objects, "gedo: one"), commit(&objects, "gedo: two"));
    assert_eq!(refs.read(&tapes).expect("read"), None);

    assert_eq!(
        refs.compare_and_swap(&update(&tapes, Expected::Absent, first))
            .expect("cas"),
        RefOutcome::Updated
    );
    assert_eq!(refs.read(&tapes).expect("read"), Some(first));

    let outcome = refs
        .compare_and_swap(&update(&tapes, Expected::Value(first), second))
        .expect("cas");
    assert_eq!(outcome, RefOutcome::Updated);
    assert_eq!(refs.read(&tapes).expect("read"), Some(second));
}

/// A stale expected value changes nothing and reports the current value.
pub fn stale_expectations_report_the_current_value<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let tapes = name("refs/gedo/tapes");
    let (first, second, third) = (
        commit(&objects, "gedo: one"),
        commit(&objects, "gedo: two"),
        commit(&objects, "gedo: three"),
    );

    let outcome = refs
        .compare_and_swap(&update(&tapes, Expected::Value(first), second))
        .expect("cas");
    assert_eq!(
        outcome,
        RefOutcome::Moved { current: None },
        "expected a value on an absent reference"
    );

    refs.compare_and_swap(&update(&tapes, Expected::Absent, first))
        .expect("cas");
    let outcome = refs
        .compare_and_swap(&update(&tapes, Expected::Absent, second))
        .expect("cas");
    assert_eq!(
        outcome,
        RefOutcome::Moved { current: Some(first) },
        "create over an existing value"
    );

    refs.compare_and_swap(&update(&tapes, Expected::Value(first), second))
        .expect("cas");
    let outcome = refs
        .compare_and_swap(&update(&tapes, Expected::Value(first), third))
        .expect("cas");
    assert_eq!(
        outcome,
        RefOutcome::Moved {
            current: Some(second)
        }
    );
    assert_eq!(refs.read(&tapes).expect("read"), Some(second), "nothing changed");
}

/// Creating a reference that already holds exactly the new value succeeds.
pub fn creating_the_same_value_is_idempotent<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let tapes = name("refs/gedo/tapes");
    let first = commit(&objects, "gedo: one");
    refs.compare_and_swap(&update(&tapes, Expected::Absent, first))
        .expect("cas");
    let outcome = refs
        .compare_and_swap(&update(&tapes, Expected::Absent, first))
        .expect("cas");
    assert_eq!(outcome, RefOutcome::Updated);
    assert_eq!(refs.read(&tapes).expect("read"), Some(first));
}

/// Deleting needs the current value.
pub fn delete_is_a_compare_and_swap<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let tapes = name("refs/gedo/tapes");
    let (first, second) = (commit(&objects, "gedo: one"), commit(&objects, "gedo: two"));
    assert_eq!(
        refs.delete(&tapes, first).expect("delete"),
        RefOutcome::Moved { current: None }
    );

    refs.compare_and_swap(&update(&tapes, Expected::Absent, first))
        .expect("cas");
    assert_eq!(
        refs.delete(&tapes, second).expect("delete"),
        RefOutcome::Moved { current: Some(first) }
    );
    assert_eq!(refs.read(&tapes).expect("read"), Some(first));

    assert_eq!(refs.delete(&tapes, first).expect("delete"), RefOutcome::Updated);
    assert_eq!(refs.read(&tapes).expect("read"), None);
    assert_eq!(
        refs.compare_and_swap(&update(&tapes, Expected::Absent, second))
            .expect("cas"),
        RefOutcome::Updated
    );
}

/// Listing returns the prefix and the names below it, by whole components, sorted.
pub fn list_matches_whole_components<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let value = commit(&objects, "gedo: one");
    for text in [
        "refs/gedo/x/two",
        "refs/gedo/x/one",
        "refs/gedo/xy",
        "refs/gedo/z",
    ] {
        refs.compare_and_swap(&update(&name(text), Expected::Absent, value))
            .expect("cas");
    }
    let listed: Vec<String> = refs
        .list(&name("refs/gedo/x"))
        .expect("list")
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect();
    assert_eq!(listed, ["refs/gedo/x/one", "refs/gedo/x/two"]);
    let listed = refs.list(&name("refs/gedo/xy")).expect("list");
    assert_eq!(listed, vec![(name("refs/gedo/xy"), value)]);
    assert!(refs.list(&name("refs/gedo/none")).expect("list").is_empty());
}

/// A name that is a file where another is a directory (`a` and `a/b`) is refused, as in Git.
pub fn directory_file_conflicts_are_refused<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let value = commit(&objects, "gedo: one");
    refs.compare_and_swap(&update(&name("refs/gedo/a/b"), Expected::Absent, value))
        .expect("cas");
    assert!(
        refs.compare_and_swap(&update(&name("refs/gedo/a"), Expected::Absent, value))
            .is_err()
    );
    assert_eq!(refs.read(&name("refs/gedo/a")).expect("read"), None);
}

/// Remote mirrors of a remote without any are empty; invalid remote names are refused.
pub fn remote_mirrors_start_empty<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (_objects, refs) = make();
    assert!(refs.list_remote("origin").expect("list").is_empty());
    assert!(refs.list_remote("../heads").is_err());
    assert!(refs.list_remote("").is_err());
}

/// Runs every reference store check as its own test.
#[macro_export]
macro_rules! ref_store_contract {
    ($make:expr) => {
        #[test]
        fn compare_and_swap_updates() {
            $crate::contract_tests::compare_and_swap_updates($make);
        }
        #[test]
        fn stale_expectations_report_the_current_value() {
            $crate::contract_tests::stale_expectations_report_the_current_value($make);
        }
        #[test]
        fn creating_the_same_value_is_idempotent() {
            $crate::contract_tests::creating_the_same_value_is_idempotent($make);
        }
        #[test]
        fn delete_is_a_compare_and_swap() {
            $crate::contract_tests::delete_is_a_compare_and_swap($make);
        }
        #[test]
        fn list_matches_whole_components() {
            $crate::contract_tests::list_matches_whole_components($make);
        }
        #[test]
        fn directory_file_conflicts_are_refused() {
            $crate::contract_tests::directory_file_conflicts_are_refused($make);
        }
        #[test]
        fn remote_mirrors_start_empty() {
            $crate::contract_tests::remote_mirrors_start_empty($make);
        }
    };
}

// --- WriteAheadLog -----------------------------------------------------------------------------

fn payloads<W: WriteAheadLog>(wal: &W) -> Vec<(u64, Vec<u8>)> {
    wal.read_all()
        .expect("read")
        .into_iter()
        .map(|(sequence, payload)| (sequence.0, payload))
        .collect()
}

/// Records come back in append order, numbered from zero.
pub fn wal_records_keep_append_order<W: WriteAheadLog>(make: impl Fn() -> W) {
    let mut wal = make();
    assert_eq!(wal.open_report().records, 0);
    assert_eq!(wal.open_report().torn_tail, None);
    assert!(wal.read_all().expect("read").is_empty());
    assert_eq!(wal.append(b"one").expect("append"), LogSequence(0));
    assert_eq!(wal.append(b"").expect("append"), LogSequence(1));
    assert_eq!(wal.append(b"three").expect("append"), LogSequence(2));
    wal.sync().expect("sync");
    assert_eq!(
        payloads(&wal),
        vec![(0, b"one".to_vec()), (1, vec![]), (2, b"three".to_vec())]
    );
}

/// Truncation removes a prefix and renumbers what remains.
pub fn wal_truncation_keeps_later_records<W: WriteAheadLog>(make: impl Fn() -> W) {
    let mut wal = make();
    for payload in [b"a", b"b", b"c"] {
        wal.append(payload).expect("append");
    }
    wal.truncate_through(LogSequence(1)).expect("truncate");
    assert_eq!(payloads(&wal), vec![(0, b"c".to_vec())]);
    assert_eq!(wal.append(b"d").expect("append"), LogSequence(1));
    assert!(wal.truncate_through(LogSequence(5)).is_err(), "beyond the end");
    assert_eq!(payloads(&wal).len(), 2, "a refused truncation changes nothing");
    wal.truncate_through(LogSequence(1)).expect("truncate");
    assert!(wal.read_all().expect("read").is_empty());
    assert_eq!(wal.append(b"e").expect("append"), LogSequence(0));
}

/// Runs every write-ahead log check as its own test.
#[macro_export]
macro_rules! wal_contract {
    ($make:expr) => {
        #[test]
        fn wal_records_keep_append_order() {
            $crate::contract_tests::wal_records_keep_append_order($make);
        }
        #[test]
        fn wal_truncation_keeps_later_records() {
            $crate::contract_tests::wal_truncation_keeps_later_records($make);
        }
    };
}

// --- SymbolCache -------------------------------------------------------------------------------

/// A fixed rebuild source for the cache checks.
pub struct FixedSource(pub Vec<CacheEntry>);

impl CacheRebuildSource for FixedSource {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(self.0.clone())
    }
}

fn sample_source() -> FixedSource {
    FixedSource(vec![
        (
            "symbols".into(),
            b"crate::billing::Invoice".to_vec(),
            b"node-1".to_vec(),
        ),
        (
            "symbols".into(),
            b"crate::billing::total".to_vec(),
            b"node-2".to_vec(),
        ),
        ("files".into(), b"src/billing.rs".to_vec(), b"blob-1".to_vec()),
    ])
}

/// A cache opened where none exists is built from the source.
pub fn new_caches_are_built_from_the_source<C: SymbolCache>(open: impl Fn(&dyn CacheRebuildSource) -> C) {
    let cache = open(&sample_source());
    assert_eq!(
        *cache.open_status(),
        CacheStatus::Rebuilt {
            reason: RebuildReason::Missing
        }
    );
    for (namespace, key, value) in sample_source().0 {
        assert_eq!(cache.get(&namespace, &key).expect("get"), Some(value));
    }
    assert_eq!(cache.get("symbols", b"absent").expect("get"), None);
    assert_eq!(
        cache.get("files", b"crate::billing::total").expect("get"),
        None,
        "namespaces are separate"
    );
}

/// Entries can be added, replaced and cleared.
pub fn cache_entries_can_change<C: SymbolCache>(open: impl Fn(&dyn CacheRebuildSource) -> C) {
    let mut cache = open(&FixedSource(vec![]));
    cache
        .put(
            "symbols",
            &[(b"k".to_vec(), b"v1".to_vec()), (b"j".to_vec(), b"w".to_vec())],
        )
        .expect("put");
    cache
        .put("symbols", &[(b"k".to_vec(), b"v2".to_vec())])
        .expect("put");
    assert_eq!(cache.get("symbols", b"k").expect("get"), Some(b"v2".to_vec()));
    assert_eq!(cache.get("symbols", b"j").expect("get"), Some(b"w".to_vec()));
    assert_eq!(cache.get("other", b"k").expect("get"), None);
    cache.clear().expect("clear");
    assert_eq!(cache.get("symbols", b"k").expect("get"), None);
}

/// Invalid namespace names are refused.
pub fn invalid_cache_namespaces_are_refused<C: SymbolCache>(open: impl Fn(&dyn CacheRebuildSource) -> C) {
    let mut cache = open(&FixedSource(vec![]));
    for bad in ["", "meta", "Upper", "a/b", &"x".repeat(65)] {
        assert!(
            cache.put(bad, &[(b"k".to_vec(), b"v".to_vec())]).is_err(),
            "{bad:?} must be refused"
        );
        assert!(cache.get(bad, b"k").is_err(), "{bad:?} must be refused");
    }
}

/// Runs every cache check as its own test.
#[macro_export]
macro_rules! cache_contract {
    ($open:expr) => {
        #[test]
        fn new_caches_are_built_from_the_source() {
            $crate::contract_tests::new_caches_are_built_from_the_source($open);
        }
        #[test]
        fn cache_entries_can_change() {
            $crate::contract_tests::cache_entries_can_change($open);
        }
        #[test]
        fn invalid_cache_namespaces_are_refused() {
            $crate::contract_tests::invalid_cache_namespaces_are_refused($open);
        }
    };
}

// --- Collection over both ports ----------------------------------------------------------------

fn entry(id: &str, content: &str) -> (EntryId, Vec<u8>) {
    (EntryId::new(id).expect("valid id"), content.as_bytes().to_vec())
}

fn tapes() -> GedoRefName {
    name("refs/gedo/tapes")
}

/// Entries put into a collection are read back; putting them again writes nothing.
pub fn collections_store_entries<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let collection = Collection::new(tapes(), &objects, &refs);
    assert_eq!(collection.head().expect("head"), None);
    assert_eq!(
        collection
            .get(&EntryId::new("abcd").expect("valid id"))
            .expect("get"),
        None
    );

    let first = collection
        .put(
            &[
                entry("abcd01", "one"),
                entry("ab9999", "two"),
                entry("ff00", "three"),
            ],
            "gedo: put",
        )
        .expect("put");
    assert_eq!(refs.read(&tapes()).expect("read"), Some(first));
    assert_eq!(
        collection
            .get(&EntryId::new("abcd01").expect("valid id"))
            .expect("get"),
        Some(b"one".to_vec())
    );
    assert_eq!(
        collection
            .get(&EntryId::new("ff00").expect("valid id"))
            .expect("get"),
        Some(b"three".to_vec())
    );
    assert_eq!(
        collection
            .get(&EntryId::new("abcd02").expect("valid id"))
            .expect("get"),
        None
    );

    assert_eq!(
        collection
            .put(&[entry("abcd01", "one")], "gedo: again")
            .expect("put"),
        first,
        "nothing new, no commit"
    );

    let second = collection
        .put(&[entry("1234", "four")], "gedo: more")
        .expect("put");
    let info = objects.read_commit(&second).expect("commit");
    assert_eq!(info.parents, vec![first], "each change extends the chain");
    assert_eq!(info.message, "gedo: more");
    assert_eq!(collection.entries().expect("entries").len(), 4);

    let root = objects.read_tree(&info.tree).expect("tree");
    let directories: Vec<&str> = root.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(directories, ["12", "ab", "ff"], "two-character fan-out");
    assert!(root.iter().all(|entry| entry.mode == EntryMode::Tree));
}

/// Entries are listed by id prefix from one fan-out directory; bad prefixes are refused.
pub fn entries_are_listed_by_prefix<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let collection = Collection::new(tapes(), &objects, &refs);
    assert!(collection.entries_with_prefix("ab").expect("list").is_empty());
    collection
        .put(
            &[
                entry("abcd01", "one"),
                entry("abcd02", "two"),
                entry("ab9999", "three"),
                entry("ff00", "four"),
            ],
            "gedo: put",
        )
        .expect("put");
    let ids = |prefix: &str| -> Vec<String> {
        collection
            .entries_with_prefix(prefix)
            .expect("list")
            .keys()
            .map(ToString::to_string)
            .collect()
    };
    assert_eq!(ids("abcd"), ["abcd01", "abcd02"]);
    assert_eq!(ids("ab"), ["ab9999", "abcd01", "abcd02"]);
    assert_eq!(ids("abcd02"), ["abcd02"]);
    assert!(ids("12").is_empty());
    for refused in ["a", "AB", "zz", ""] {
        assert!(collection.entries_with_prefix(refused).is_err(), "{refused:?}");
    }
}

/// The same id with different content is corruption, and nothing is written.
pub fn conflicting_entries_are_refused<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let collection = Collection::new(tapes(), &objects, &refs);
    let head = collection.put(&[entry("abcd", "one")], "gedo: put").expect("put");
    let error = collection
        .put(&[entry("abcd", "other")], "gedo: put")
        .expect_err("refused");
    assert!(matches!(error, StoreError::Corrupted { .. }), "{error}");
    assert_eq!(collection.head().expect("head"), Some(head));
    assert!(collection.put(&[], "gedo: nothing").is_err());
    assert!(
        collection
            .put(&[entry("abcd", "x"), entry("abcd", "y")], "gedo: twice")
            .is_err()
    );
}

/// A reference store that lets a rival writer win the first races: before each of the first
/// `races` compare-and-swaps, the rival puts its own entry through the inner store.
pub struct RacingRefs<'a, O: ObjectStore, R: RefStore> {
    objects: &'a O,
    inner: &'a R,
    races_left: std::sync::Mutex<u32>,
    rival_writes: std::sync::Mutex<u32>,
}

impl<'a, O: ObjectStore, R: RefStore> RacingRefs<'a, O, R> {
    /// Wraps `inner`; the rival wins the next `races` updates.
    pub fn new(objects: &'a O, inner: &'a R, races: u32) -> Self {
        RacingRefs {
            objects,
            inner,
            races_left: races.into(),
            rival_writes: 0.into(),
        }
    }

    /// Entries the rival wrote so far.
    pub fn rival_writes(&self) -> u32 {
        *self.rival_writes.lock().expect("not poisoned")
    }
}

impl<O: ObjectStore, R: RefStore> RefStore for RacingRefs<'_, O, R> {
    fn read(&self, name: &GedoRefName) -> Result<Option<GitOid>, StoreError> {
        self.inner.read(name)
    }
    fn list(&self, prefix: &GedoRefName) -> Result<Vec<(GedoRefName, GitOid)>, StoreError> {
        self.inner.list(prefix)
    }
    fn list_remote(&self, remote: &str) -> Result<Vec<(String, GitOid)>, StoreError> {
        self.inner.list_remote(remote)
    }
    fn compare_and_swap(&self, update: &RefUpdate) -> Result<RefOutcome, StoreError> {
        let race = {
            let mut left = self.races_left.lock().expect("not poisoned");
            let race = *left > 0;
            *left = left.saturating_sub(1);
            race
        };
        if race {
            let mut writes = self.rival_writes.lock().expect("not poisoned");
            *writes += 1;
            let rival = Collection::new(update.name.clone(), self.objects, self.inner);
            rival.put(&[entry(&format!("ee{:04x}", *writes), "rival")], "gedo: rival")?;
        }
        self.inner.compare_and_swap(update)
    }
    fn delete(&self, name: &GedoRefName, expected: GitOid) -> Result<RefOutcome, StoreError> {
        self.inner.delete(name, expected)
    }
}

/// A put that loses races reloads, keeps the rival's entries and retries; it gives up as
/// `Contended` after [`MAX_UPDATE_ATTEMPTS`].
pub fn puts_retry_when_the_reference_moves<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let racing = RacingRefs::new(&objects, &refs, 3);
    let collection = Collection::new(tapes(), &objects, &racing);
    collection
        .put(&[entry("abcd", "mine")], "gedo: put")
        .expect("succeeds after 3 lost races");
    assert_eq!(racing.rival_writes(), 3);
    let entries = Collection::new(tapes(), &objects, &refs)
        .entries()
        .expect("entries");
    assert_eq!(entries.len(), 4, "ours and the rival's three entries");

    let racing = RacingRefs {
        objects: &objects,
        inner: &refs,
        races_left: MAX_UPDATE_ATTEMPTS.into(),
        rival_writes: 3.into(),
    };
    let collection = Collection::new(tapes(), &objects, &racing);
    let error = collection
        .put(&[entry("dcba", "mine")], "gedo: put")
        .expect_err("gives up");
    assert!(
        matches!(
            error,
            StoreError::Contended {
                attempts: MAX_UPDATE_ATTEMPTS,
                ..
            }
        ),
        "{error}"
    );
}

/// Diverged chains are joined by a merge commit with the union of both sides; behind and ahead
/// chains fast-forward or stay as they are.
pub fn diverged_chains_merge_by_union<O: ObjectStore, R: RefStore>(make: impl Fn() -> (O, R)) {
    let (objects, refs) = make();
    let local = Collection::new(tapes(), &objects, &refs);
    let base = local.put(&[entry("aaaa", "base")], "gedo: base").expect("put");

    // A teammate's chain: same base, other entries (another reference stands in for the remote).
    let theirs = Collection::new(name("refs/gedo/theirs"), &objects, &refs);
    assert_eq!(
        theirs.merge(base).expect("merge"),
        MergeOutcome::FastForwarded { head: base }
    );
    let their_head = theirs
        .put(&[entry("bbbb", "theirs")], "gedo: theirs")
        .expect("put");
    local.put(&[entry("cccc", "mine")], "gedo: mine").expect("put");

    let MergeOutcome::Merged { head } = local.merge(their_head).expect("merge") else {
        panic!("diverged chains need a merge commit");
    };
    let info = objects.read_commit(&head).expect("commit");
    assert_eq!(info.parents.len(), 2);
    assert_eq!(info.parents[1], their_head);
    let ids: Vec<String> = local
        .entries()
        .expect("entries")
        .keys()
        .map(ToString::to_string)
        .collect();
    assert_eq!(ids, ["aaaa", "bbbb", "cccc"]);

    assert_eq!(
        local.merge(their_head).expect("merge"),
        MergeOutcome::UpToDate,
        "already merged"
    );
    assert_eq!(
        local.merge(base).expect("merge"),
        MergeOutcome::UpToDate,
        "an ancestor"
    );
    assert_eq!(
        theirs.merge(head).expect("merge"),
        MergeOutcome::FastForwarded { head },
        "behind"
    );
}

/// A remote without a chain for the collection leaves it unchanged.
pub fn merging_a_missing_remote_chain_changes_nothing<O: ObjectStore, R: RefStore>(
    make: impl Fn() -> (O, R),
) {
    let (objects, refs) = make();
    let collection = Collection::new(tapes(), &objects, &refs);
    let head = collection.put(&[entry("abcd", "one")], "gedo: put").expect("put");
    assert_eq!(
        collection.merge_remote("origin").expect("merge"),
        MergeOutcome::NoRemoteChain
    );
    assert_eq!(collection.head().expect("head"), Some(head));
}

/// Runs every collection check as its own test.
#[macro_export]
macro_rules! collection_contract {
    ($make:expr) => {
        #[test]
        fn collections_store_entries() {
            $crate::contract_tests::collections_store_entries($make);
        }
        #[test]
        fn entries_are_listed_by_prefix() {
            $crate::contract_tests::entries_are_listed_by_prefix($make);
        }
        #[test]
        fn conflicting_entries_are_refused() {
            $crate::contract_tests::conflicting_entries_are_refused($make);
        }
        #[test]
        fn puts_retry_when_the_reference_moves() {
            $crate::contract_tests::puts_retry_when_the_reference_moves($make);
        }
        #[test]
        fn diverged_chains_merge_by_union() {
            $crate::contract_tests::diverged_chains_merge_by_union($make);
        }
        #[test]
        fn merging_a_missing_remote_chain_changes_nothing() {
            $crate::contract_tests::merging_a_missing_remote_chain_changes_nothing($make);
        }
    };
}
