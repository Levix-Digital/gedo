//! Reconciliation of identities (research R11, ADR-0007).
//!
//! Identity merges form an undirected relation between ids; every id resolves to the lowest id of
//! its connected component. The result depends neither on the order in which merges were found nor
//! on how they are grouped, so every machine resolves the same way.

use std::collections::BTreeMap;

use gedo_core::identity::{AstNodeId, IdentityMerged};

/// Resolves ids through identity merges.
#[derive(Clone, Debug, Default)]
pub struct AliasTable {
    /// Each id's parent in its component; a root is absent. Parents are always lower than their
    /// children, so a root is the lowest id of its component.
    parent: BTreeMap<AstNodeId, AstNodeId>,
}

impl AliasTable {
    /// An empty table: every id resolves to itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// A table holding every merge of `merges`.
    pub fn from_merges<'a>(merges: impl IntoIterator<Item = &'a IdentityMerged>) -> Self {
        let mut table = Self::new();
        merges.into_iter().for_each(|merge| table.add(merge));
        table
    }

    /// Records that `merge.kept` and `merge.merged` name one entity.
    pub fn add(&mut self, merge: &IdentityMerged) {
        self.join(merge.kept, merge.merged);
    }

    /// Records every merge of another table.
    pub fn extend(&mut self, other: &AliasTable) {
        for (&child, &parent) in &other.parent {
            self.join(child, parent);
        }
    }

    /// The id `id` resolves to: the lowest id of its component.
    pub fn resolve(&self, id: AstNodeId) -> AstNodeId {
        let mut current = id;
        while let Some(&parent) = self.parent.get(&current) {
            current = parent;
        }
        current
    }

    /// Whether no merge was recorded.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    fn join(&mut self, left: AstNodeId, right: AstNodeId) {
        let left_root = self.resolve(left);
        let right_root = self.resolve(right);
        if left_root == right_root {
            return;
        }
        let (low, high) = if left_root < right_root {
            (left_root, right_root)
        } else {
            (right_root, left_root)
        };
        self.parent.insert(high, low);
        // Path compression keeps lookups short; it never changes a resolution.
        for id in [left, right] {
            self.compress(id, low);
        }
    }

    fn compress(&mut self, id: AstNodeId, root: AstNodeId) {
        let mut current = id;
        while current != root {
            let Some(&parent) = self.parent.get(&current) else {
                break;
            };
            self.parent.insert(current, root);
            current = parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gedo_core::ids::GitOid;
    use proptest::prelude::*;

    fn id(byte: u8) -> AstNodeId {
        AstNodeId([byte; 16])
    }

    fn merge(left: u8, right: u8) -> IdentityMerged {
        let (kept, merged) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        IdentityMerged {
            kept: id(kept),
            merged: id(merged),
            commit: GitOid::from_bytes(&[0; 20]).expect("valid id"),
        }
    }

    #[test]
    fn unknown_ids_resolve_to_themselves() {
        assert_eq!(AliasTable::new().resolve(id(5)), id(5));
    }

    #[test]
    fn a_merge_resolves_to_the_lower_id() {
        let table = AliasTable::from_merges(&[merge(3, 9)]);
        assert_eq!(table.resolve(id(9)), id(3));
        assert_eq!(table.resolve(id(3)), id(3));
    }

    #[test]
    fn chains_resolve_to_the_lowest_id() {
        let table = AliasTable::from_merges(&[merge(5, 9), merge(9, 2), merge(7, 5)]);
        for byte in [2, 5, 7, 9] {
            assert_eq!(table.resolve(id(byte)), id(2), "{byte}");
        }
    }

    #[test]
    fn combined_tables_agree() {
        let mut left = AliasTable::from_merges(&[merge(4, 8)]);
        let right = AliasTable::from_merges(&[merge(8, 1), merge(6, 7)]);
        left.extend(&right);
        assert_eq!(left.resolve(id(4)), id(1));
        assert_eq!(left.resolve(id(7)), id(6));
    }

    proptest! {
        #[test]
        fn resolution_ignores_order_and_grouping(
            pairs in proptest::collection::vec((0u8..24, 0u8..24), 0..40),
            split in 0usize..40,
            seed in any::<u64>(),
        ) {
            let merges: Vec<_> = pairs.iter().map(|&(a, b)| merge(a, b)).collect();
            let reference = AliasTable::from_merges(&merges);

            let mut shuffled = merges.clone();
            let mut state = seed;
            for index in (1..shuffled.len()).rev() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                shuffled.swap(index, (state >> 33) as usize % (index + 1));
            }
            let at = split.min(shuffled.len());
            let mut grouped = AliasTable::from_merges(&shuffled[..at]);
            grouped.extend(&AliasTable::from_merges(&shuffled[at..]));

            for byte in 0u8..24 {
                let resolved = reference.resolve(id(byte));
                prop_assert_eq!(grouped.resolve(id(byte)), resolved);
                prop_assert!(resolved <= id(byte));
            }
            // The lowest id of each component: no id resolves above another of its component.
            for &(a, b) in &pairs {
                prop_assert_eq!(reference.resolve(id(a)), reference.resolve(id(b)));
                prop_assert!(reference.resolve(id(a)) <= id(a.min(b)));
            }
        }
    }
}
