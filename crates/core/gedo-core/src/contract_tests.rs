//! The contract every adapter of a port must pass, run against the in-memory adapters here and
//! against the real adapters in `gedo-store`.
//!
//! Each check is a public function taking a factory of fresh, empty stores; the macros
//! [`object_store_contract!`](crate::object_store_contract), [`ref_store_contract!`](crate::ref_store_contract),
//! [`wal_contract!`](crate::wal_contract) and [`cache_contract!`](crate::cache_contract) turn every
//! check into its own `#[test]`.

use crate::error::StoreError;
use crate::ids::{GedoRefName, GitOid, ObjectHash};
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

/// Commits round trip with their tree, parents and message.
pub fn commits_round_trip<O: ObjectStore>(make: impl Fn() -> O) {
    let objects = make();
    let root = commit(&objects, "gedo: first");
    let info = objects.read_commit(&root).expect("read");
    assert!(info.parents.is_empty());
    assert_eq!(info.message, "gedo: first");

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
        fn missing_objects_are_not_found() {
            $crate::contract_tests::missing_objects_are_not_found($make);
        }
        #[test]
        fn trees_round_trip_in_git_order() {
            $crate::contract_tests::trees_round_trip_in_git_order($make);
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
