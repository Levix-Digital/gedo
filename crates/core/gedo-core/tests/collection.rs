//! Collections on the in-memory adapters: the contract checks plus properties of the fan-out
//! layout and of the union merge.

use std::collections::HashSet;

use gedo_core::collection::{EntryMap, union};
use gedo_core::memory::{MemoryObjectStore, MemoryRefStore};
use gedo_core::{Collection, EntryId, GedoRefName, GitOid, MergeOutcome};
use proptest::prelude::*;

gedo_core::collection_contract!(|| (MemoryObjectStore::new(), MemoryRefStore::new()));

fn blob(byte: u8) -> GitOid {
    GitOid::from_bytes(&[byte; 20]).expect("valid id")
}

/// Small maps over a small id space, so that sides overlap, with content derived from the id
/// (content-addressed: the same id always has the same blob).
fn entry_map() -> impl Strategy<Value = EntryMap> {
    proptest::collection::btree_set(0u8..32, 0..12).prop_map(|ids| {
        ids.into_iter()
            .map(|id| (EntryId::new(&format!("{id:02x}00")).expect("valid id"), blob(id)))
            .collect()
    })
}

proptest! {
    #[test]
    fn union_is_commutative(a in entry_map(), b in entry_map()) {
        prop_assert_eq!(union(&a, &b), union(&b, &a));
    }

    #[test]
    fn union_is_associative(a in entry_map(), b in entry_map(), c in entry_map()) {
        let left = union(&union(&a, &b).expect("no conflict"), &c);
        let right = union(&a, &union(&b, &c).expect("no conflict"));
        prop_assert_eq!(left, right);
    }

    #[test]
    fn union_is_idempotent(a in entry_map(), b in entry_map()) {
        prop_assert_eq!(union(&a, &a).expect("no conflict"), a.clone());
        let once = union(&a, &b).expect("no conflict");
        prop_assert_eq!(union(&once, &b).expect("no conflict"), once);
    }

    #[test]
    fn fan_out_paths_are_unique_even_ignoring_case(ids in proptest::collection::btree_set("[0-9a-f]{4,40}", 1..50)) {
        let mut paths = HashSet::new();
        for text in &ids {
            let id = EntryId::new(text).expect("valid id");
            let (directory, file) = id.fan_out();
            prop_assert_eq!(directory.len(), 2);
            prop_assert!(!file.is_empty());
            let path = format!("{directory}/{file}").to_lowercase();
            prop_assert!(paths.insert(path));
        }
    }

    #[test]
    fn merging_in_either_direction_gives_the_same_entries(
        mine in proptest::collection::btree_set(0u8..64, 1..10),
        theirs in proptest::collection::btree_set(0u8..64, 1..10),
    ) {
        let objects = MemoryObjectStore::new();
        let refs = MemoryRefStore::new();
        let name = |text: &str| GedoRefName::new(text).expect("valid name");
        let put = |collection: &Collection<'_, _, _>, ids: &std::collections::BTreeSet<u8>| {
            let entries: Vec<_> = ids
                .iter()
                .map(|id| (EntryId::new(&format!("{id:02x}00")).expect("valid id"), vec![*id]))
                .collect();
            collection.put(&entries, "gedo: put").expect("put")
        };
        let a = Collection::new(name("refs/gedo/a"), &objects, &refs);
        let b = Collection::new(name("refs/gedo/b"), &objects, &refs);
        let a_head = put(&a, &mine);
        let b_head = put(&b, &theirs);
        a.merge(b_head).expect("merge");
        b.merge(a_head).expect("merge");
        prop_assert_eq!(a.entries().expect("entries"), b.entries().expect("entries"));
        let again = a.merge(b_head).expect("merge");
        prop_assert_eq!(again, MergeOutcome::UpToDate);
    }
}
