//! The commits to index: every ancestor of the tips without a record, parents first (research R10).

use std::collections::{BTreeMap, BTreeSet};

use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CommitInfo, ObjectStore};

/// Every commit reachable from `tips` for which `is_indexed` is false, with its commit, in
/// topological order (parents first), ties broken by commit id. The walk stops at indexed commits.
pub fn commits_to_index<O: ObjectStore>(
    objects: &O,
    tips: &[GitOid],
    mut is_indexed: impl FnMut(&GitOid) -> Result<bool, StoreError>,
) -> Result<Vec<(GitOid, CommitInfo)>, StoreError> {
    let mut found: BTreeMap<GitOid, CommitInfo> = BTreeMap::new();
    let mut pending: Vec<GitOid> = tips.to_vec();
    let mut seen: BTreeSet<GitOid> = BTreeSet::new();
    while let Some(oid) = pending.pop() {
        if !seen.insert(oid) || is_indexed(&oid)? {
            continue;
        }
        let commit = objects.read_commit(&oid)?;
        pending.extend(commit.parents.iter().copied());
        found.insert(oid, commit);
    }

    // Kahn's algorithm; the ready set is ordered, so the result does not depend on the tips' order.
    let mut waiting: BTreeMap<GitOid, usize> = BTreeMap::new();
    let mut children: BTreeMap<GitOid, Vec<GitOid>> = BTreeMap::new();
    for (oid, commit) in &found {
        let parents: BTreeSet<&GitOid> = commit
            .parents
            .iter()
            .filter(|parent| found.contains_key(parent))
            .collect();
        waiting.insert(*oid, parents.len());
        for parent in parents {
            children.entry(*parent).or_default().push(*oid);
        }
    }
    let mut ready: BTreeSet<GitOid> = waiting
        .iter()
        .filter(|&(_, &count)| count == 0)
        .map(|(oid, _)| *oid)
        .collect();
    let mut ordered = Vec::with_capacity(found.len());
    while let Some(oid) = ready.pop_first() {
        for child in children.get(&oid).into_iter().flatten() {
            let count = waiting.get_mut(child).expect("every child is waiting");
            *count -= 1;
            if *count == 0 {
                ready.insert(*child);
            }
        }
        let commit = found.remove(&oid).expect("every ordered commit was found");
        ordered.push((oid, commit));
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gedo_core::memory::MemoryObjectStore;
    use gedo_core::ports::NewCommit;

    fn commit(objects: &MemoryObjectStore, parents: &[GitOid], message: &str) -> GitOid {
        let tree = objects.write_tree(&[]).expect("tree");
        objects
            .write_commit(&NewCommit {
                tree,
                parents: parents.to_vec(),
                message: message.into(),
            })
            .expect("commit")
    }

    fn position(order: &[(GitOid, CommitInfo)], oid: GitOid) -> usize {
        order
            .iter()
            .position(|(found, _)| *found == oid)
            .expect("ordered")
    }

    #[test]
    fn parents_come_first_whatever_the_tips_order() {
        let objects = MemoryObjectStore::new();
        let root_a = commit(&objects, &[], "a");
        let root_b = commit(&objects, &[], "b");
        let left = commit(&objects, &[root_a], "left");
        let right = commit(&objects, &[root_a], "right");
        let merge = commit(&objects, &[left, right, root_b], "merge");
        let one = commits_to_index(&objects, &[merge, right], |_| Ok(false)).expect("walk");
        let two = commits_to_index(&objects, &[right, left, merge], |_| Ok(false)).expect("walk");
        let ids = |order: &[(GitOid, CommitInfo)]| order.iter().map(|(oid, _)| *oid).collect::<Vec<_>>();
        assert_eq!(ids(&one), ids(&two));
        assert_eq!(one.len(), 5);
        for (child, parent) in [
            (left, root_a),
            (right, root_a),
            (merge, left),
            (merge, right),
            (merge, root_b),
        ] {
            assert!(position(&one, parent) < position(&one, child));
        }
    }

    #[test]
    fn indexed_commits_stop_the_walk() {
        let objects = MemoryObjectStore::new();
        let root = commit(&objects, &[], "root");
        let middle = commit(&objects, &[root], "middle");
        let tip = commit(&objects, &[middle], "tip");
        let order = commits_to_index(&objects, &[tip], |oid| Ok(*oid == middle)).expect("walk");
        assert_eq!(order.len(), 1);
        assert_eq!(order[0].0, tip);
    }
}
