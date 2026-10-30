//! Re-attachment of tapes to rewritten commits (`specs/005-history-reattachment/research.md`
//! R3 to R7).
//!
//! A commit made by a rewrite (a rebase, an amend, a squash on the server) has no tape of its own,
//! but its changes are those of live events stored in tapes of the commits it replaced. This
//! module finds those tapes. The candidate events are folded, entity by entity, into the entity's
//! last state; a change of the commit matches an entity whose last state equals the state the
//! change leaves, whatever the operations in between, and the earlier events of that entity are
//! superseded by the match. A tape is attached by the rule of the clarification of 2026-10-30.
//!
//! Everything here is pure and deterministic: the result does not depend on the order of the
//! candidates.

use std::collections::{BTreeMap, BTreeSet};

use gedo_core::event::{Clock, Event, EventId, SemanticOp, TapeId};
use gedo_core::identity::{AstNodeId, IdentityMerged, SemanticDelta};
use gedo_core::syntax::{EntityKind, StructuralHash};

use crate::id::event_id;

/// Key-derivation context of semantic signatures. Signatures are never stored, so it may change
/// with the matching.
const SIGNATURE_CONTEXT_V1: &str = "gedo semantic signature v1";

/// The semantic changes of a commit without its base: equal for an old and a new commit that
/// make the same changes (research R3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemanticSignature(pub [u8; 32]);

/// The signature of a commit's deltas and identity merges, each delta with the entity's
/// structural hash after it (`None` for a tombstone or an unknown hash).
pub fn semantic_signature(
    deltas: &[SemanticDelta],
    merges: &[IdentityMerged],
    result_hash: impl Fn(AstNodeId) -> Option<StructuralHash>,
) -> SemanticSignature {
    let mut changes: Vec<(&SemanticDelta, Option<StructuralHash>)> = deltas
        .iter()
        .map(|delta| {
            let hash = match delta {
                SemanticDelta::Tombstoned { .. } => None,
                _ => result_hash(delta.id()),
            };
            (delta, hash)
        })
        .collect();
    changes.sort();
    let mut pairs: Vec<(AstNodeId, AstNodeId)> =
        merges.iter().map(|merge| (merge.kept, merge.merged)).collect();
    pairs.sort();
    let encoded = postcard::to_allocvec(&(changes, pairs))
        .unwrap_or_else(|_| unreachable!("deltas and ids hold only encodable fields"));
    SemanticSignature(blake3::derive_key(SIGNATURE_CONTEXT_V1, &encoded))
}

/// A tape that may belong to the commit being re-attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateTape {
    /// The tape.
    pub tape: TapeId,
    /// Its events, in tape order.
    pub events: Vec<Event>,
    /// Whether it belongs to the old commit of a rewritten pair named by Git.
    pub paired: bool,
}

/// The result of matching one commit against its candidate tapes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reattachment {
    /// The tapes attached to the commit, sorted by id.
    pub attached: Vec<TapeId>,
    /// Positions of the commit's changes (deltas, then merges) that attached events account for.
    pub accounted: BTreeSet<usize>,
    /// Identities of entities created on the rewritten branch linked to their identity in the
    /// commit, as `(kept, merged)`: the lower id is kept (ADR-0007). Sorted.
    pub links: Vec<(AstNodeId, AstNodeId)>,
    /// Positions of the changes that no attached event accounts for.
    pub unmatched: Vec<usize>,
}

/// How one candidate event is accounted for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// The last event of an entity whose state equals the commit's.
    Matched,
    /// An earlier event of a matched entity.
    Superseded,
    /// An event of an entity with no net effect, which the commit does not change.
    Neutral,
    /// An identity merge the commit agrees with.
    Merge,
    /// Not accounted for.
    Unaccounted,
}

/// The last state of one entity among the candidate events, in their total order.
struct EntityState {
    /// Events of the entity: (candidate tape, event id, structural hash after it), in total order.
    events: Vec<(usize, EventId, Option<StructuralHash>)>,
    /// The entity's structural hash after its last event; `None` after a tombstone.
    last: Option<StructuralHash>,
    /// Kind and qualified path when the entity's creation is among the candidates.
    created: Option<(EntityKind, String)>,
    /// No net effect: created then removed, or changed then changed back.
    neutral: bool,
}

/// Matches the changes of one commit against `candidates`.
///
/// `changes` are the commit's deltas against its first parent then its identity merges, as
/// reconstruction lists them; `result_hash` gives an entity's structural hash in the commit, and
/// `resolve` an id through the identity merges known to the index.
pub fn match_commit(
    changes: &[SemanticOp],
    result_hash: impl Fn(AstNodeId) -> Option<StructuralHash>,
    candidates: &[CandidateTape],
    resolve: impl Fn(AstNodeId) -> AstNodeId,
) -> Reattachment {
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by_key(|&index| candidates[index].tape);
    let states = fold(candidates, &order, &resolve);

    // The entity each change leaves, and the state it leaves it in.
    let mut targets: BTreeMap<AstNodeId, Vec<usize>> = BTreeMap::new();
    for (position, change) in changes.iter().enumerate() {
        if let SemanticOp::Delta(delta) = change {
            targets.entry(resolve(delta.id())).or_default().push(position);
        }
    }
    let commit_merges: BTreeSet<(AstNodeId, AstNodeId)> = changes
        .iter()
        .filter_map(|change| match change {
            SemanticOp::IdentityMerged { kept, merged } => Some((*kept, *merged)),
            SemanticOp::Delta(_) => None,
        })
        .collect();

    // Entities matched by the commit: the commit's entity, its changes, and the position of the
    // matched event among the entity's events (the latest one leaving the commit's state; a
    // replayed commit of a rebase merge matches an event before the branch's later ones).
    let mut matched: BTreeMap<AstNodeId, (AstNodeId, Vec<usize>, usize)> = BTreeMap::new();
    let mut claimed: BTreeSet<AstNodeId> = BTreeSet::new();
    for (&target, positions) in &targets {
        let Some(state) = states.get(&target) else {
            continue;
        };
        let Some(after) = leaves(&changes[positions[0]], &result_hash) else {
            continue;
        };
        if positions
            .iter()
            .any(|&position| leaves(&changes[position], &result_hash) != Some(after))
        {
            continue;
        }
        if let Some(at) = state.events.iter().rposition(|(_, _, hash)| *hash == after) {
            matched.insert(target, (target, positions.clone(), at));
            claimed.insert(target);
        }
    }
    // Entities created on the rewritten branch have another id in the commit (ADR-0007).
    for (&target, positions) in &targets {
        if states.contains_key(&target) {
            continue;
        }
        let Some(SemanticOp::Delta(SemanticDelta::Created { kind, at, .. })) = positions
            .iter()
            .map(|&position| &changes[position])
            .find(|change| matches!(change, SemanticOp::Delta(SemanticDelta::Created { .. })))
        else {
            continue;
        };
        let Some(Some(after)) = leaves(&changes[positions[0]], &result_hash) else {
            continue;
        };
        if positions
            .iter()
            .any(|&position| leaves(&changes[position], &result_hash) != Some(Some(after)))
        {
            continue;
        }
        let found = states.iter().find(|(id, state)| {
            !claimed.contains(*id)
                && !targets.contains_key(*id)
                && state.last == Some(after)
                && state.created.as_ref() == Some(&(*kind, at.qualified_path.clone()))
        });
        if let Some((&entity, state)) = found {
            claimed.insert(entity);
            matched.insert(entity, (target, positions.clone(), state.events.len() - 1));
        }
    }

    // Statuses of every candidate event, then the attachment rule to a fixed point: a match or a
    // supersession counts only while the matched event's tape is attached.
    let mut status: BTreeMap<(usize, EventId), Status> = BTreeMap::new();
    let mut matched_event: BTreeMap<AstNodeId, (usize, EventId)> = BTreeMap::new();
    for (&entity, state) in &states {
        if let Some((_, _, at)) = matched.get(&entity) {
            for (position, (tape, id, _)) in state.events.iter().enumerate() {
                let found = match position.cmp(at) {
                    std::cmp::Ordering::Less => Status::Superseded,
                    std::cmp::Ordering::Equal => Status::Matched,
                    std::cmp::Ordering::Greater => Status::Unaccounted,
                };
                status.insert((*tape, *id), found);
            }
            let (tape, id, _) = state.events[*at];
            matched_event.insert(entity, (tape, id));
        } else {
            let neutral = state.neutral && !targets.contains_key(&entity);
            for (tape, id, _) in &state.events {
                status.insert(
                    (*tape, *id),
                    if neutral {
                        Status::Neutral
                    } else {
                        Status::Unaccounted
                    },
                );
            }
        }
    }
    for &index in &order {
        for event in &candidates[index].events {
            if let SemanticOp::IdentityMerged { kept, merged } = event.operation {
                let agreed = commit_merges.contains(&(kept, merged)) || resolve(kept) == resolve(merged);
                status.insert(
                    (index, event_id(event)),
                    if agreed {
                        Status::Merge
                    } else {
                        Status::Unaccounted
                    },
                );
            }
        }
    }
    let event_entity: BTreeMap<(usize, EventId), AstNodeId> = states
        .iter()
        .flat_map(|(&entity, state)| {
            state
                .events
                .iter()
                .map(move |(tape, id, _)| ((*tape, *id), entity))
        })
        .collect();

    let mut attached: BTreeSet<usize> = (0..candidates.len()).collect();
    loop {
        let next: BTreeSet<usize> = attached
            .iter()
            .copied()
            .filter(|&index| {
                let mut counts = false;
                let mut unaccounted = false;
                for event in &candidates[index].events {
                    let key = (index, event_id(event));
                    match status.get(&key).copied().unwrap_or(Status::Unaccounted) {
                        Status::Matched | Status::Superseded => {
                            let by = event_entity
                                .get(&key)
                                .and_then(|entity| matched_event.get(entity));
                            if by.is_some_and(|(tape, _)| attached.contains(tape)) {
                                counts = true;
                            } else {
                                unaccounted = true;
                            }
                        }
                        Status::Neutral | Status::Merge => {}
                        Status::Unaccounted => unaccounted = true,
                    }
                }
                counts && (candidates[index].paired || !unaccounted)
            })
            .collect();
        if next == attached {
            break;
        }
        attached = next;
    }

    let mut result = Reattachment::default();
    for (&entity, (target, positions, _)) in &matched {
        let Some((tape, _)) = matched_event.get(&entity) else {
            continue;
        };
        if !attached.contains(tape) {
            continue;
        }
        result.accounted.extend(positions.iter().copied());
        if entity != *target {
            result.links.push((entity.min(*target), entity.max(*target)));
        }
    }
    for (position, change) in changes.iter().enumerate() {
        if let SemanticOp::IdentityMerged { kept, merged } = change {
            let recorded = attached.iter().any(|&index| {
                candidates[index].events.iter().any(|event| {
                    matches!(event.operation, SemanticOp::IdentityMerged { kept: k, merged: m } if k == *kept && m == *merged)
                })
            });
            if recorded {
                result.accounted.insert(position);
            }
        }
    }
    result.links.sort();
    result.unmatched = (0..changes.len())
        .filter(|position| !result.accounted.contains(position))
        .collect();
    result.attached = attached.iter().map(|&index| candidates[index].tape).collect();
    result.attached.sort();
    result.attached.dedup();
    result
}

/// The links of a re-attachment as identity merges at `commit`, for reconstruction.
pub fn link_merges(links: &[(AstNodeId, AstNodeId)], commit: gedo_core::ids::GitOid) -> Vec<IdentityMerged> {
    links
        .iter()
        .map(|&(kept, merged)| IdentityMerged { kept, merged, commit })
        .collect()
}

/// The state a change leaves its entity in: `Some(None)` for a tombstone, `Some(Some(hash))`
/// otherwise, `None` when the hash is unknown (never matches).
fn leaves(
    change: &SemanticOp,
    result_hash: &impl Fn(AstNodeId) -> Option<StructuralHash>,
) -> Option<Option<StructuralHash>> {
    match change {
        SemanticOp::Delta(SemanticDelta::Tombstoned { .. }) => Some(None),
        SemanticOp::Delta(delta) => result_hash(delta.id()).map(Some),
        SemanticOp::IdentityMerged { .. } => None,
    }
}

/// Folds the delta events of the candidates, by resolved target, in the total order of events.
fn fold(
    candidates: &[CandidateTape],
    order: &[usize],
    resolve: &impl Fn(AstNodeId) -> AstNodeId,
) -> BTreeMap<AstNodeId, EntityState> {
    let mut events: Vec<(Clock, EventId, usize, &Event, &SemanticDelta)> = Vec::new();
    for &index in order {
        for event in &candidates[index].events {
            if let SemanticOp::Delta(delta) = &event.operation {
                events.push((event.clock, event_id(event), index, event, delta));
            }
        }
    }
    events.sort_by_key(|event| (event.0, event.1, event.2));
    events.dedup_by(|left, right| left.1 == right.1 && left.2 == right.2);

    let mut by_entity: BTreeMap<AstNodeId, Vec<(usize, &Event, &SemanticDelta)>> = BTreeMap::new();
    for (_, _, index, event, delta) in events {
        by_entity
            .entry(resolve(event.target))
            .or_default()
            .push((index, event, delta));
    }
    by_entity
        .into_iter()
        .map(|(entity, list)| (entity, state_of(&list)))
        .collect()
}

fn state_of(list: &[(usize, &Event, &SemanticDelta)]) -> EntityState {
    let mut created: Option<(EntityKind, String)> = None;
    for (_, _, delta) in list {
        match delta {
            SemanticDelta::Created { kind, at, .. } => created = Some((*kind, at.qualified_path.clone())),
            SemanticDelta::Moved { to, .. } => {
                if let Some((_, path)) = created.as_mut() {
                    path.clone_from(&to.qualified_path);
                }
            }
            SemanticDelta::Renamed { qualified_path, .. } => {
                if let Some((_, path)) = created.as_mut() {
                    path.clone_from(qualified_path);
                }
            }
            _ => {}
        }
    }
    let last = list.last().and_then(|(_, event, _)| event.result_hash);
    EntityState {
        events: list
            .iter()
            .map(|(index, event, _)| (*index, event_id(event), event.result_hash))
            .collect(),
        last,
        created,
        neutral: neutral(list),
    }
}

/// Whether the events have no net effect (research R5).
fn neutral(list: &[(usize, &Event, &SemanticDelta)]) -> bool {
    let first = list.first().map(|(_, _, delta)| *delta);
    let last = list.last().map(|(_, _, delta)| *delta);
    if matches!(first, Some(SemanticDelta::Created { .. }))
        && matches!(last, Some(SemanticDelta::Tombstoned { .. }))
    {
        return true;
    }
    let mut body: Option<(StructuralHash, StructuralHash)> = None;
    let mut signature: Option<(StructuralHash, StructuralHash)> = None;
    for (_, _, delta) in list {
        match delta {
            SemanticDelta::BodyModified { before, after, .. } => {
                body = Some((body.map_or(*before, |(first, _)| first), *after));
            }
            SemanticDelta::SignatureMutated { before, after, .. } => {
                signature = Some((signature.map_or(*before, |(first, _)| first), *after));
            }
            _ => return false,
        }
    }
    (body.is_some() || signature.is_some())
        && body.is_none_or(|(first, last)| first == last)
        && signature.is_none_or(|(first, last)| first == last)
}

#[cfg(test)]
mod tests {
    use gedo_core::event::{Author, AuthorFingerprint, AuthorKind, Labels, Origin};
    use gedo_core::identity::Location;

    use super::*;
    use crate::id::tape_id;

    fn node(n: u8) -> AstNodeId {
        AstNodeId([n; 16])
    }

    fn hash(n: u8) -> StructuralHash {
        StructuralHash([n; 32])
    }

    fn at(path: &str) -> Location {
        Location {
            unit: "src".into(),
            path: "src/lib.rs".into(),
            qualified_path: path.into(),
        }
    }

    fn created(id: u8, path: &str) -> SemanticDelta {
        SemanticDelta::Created {
            id: node(id),
            kind: EntityKind::Function,
            at: at(path),
        }
    }

    fn body(id: u8, before: u8, after: u8) -> SemanticDelta {
        SemanticDelta::BodyModified {
            id: node(id),
            before: hash(before),
            after: hash(after),
        }
    }

    fn tombstoned(id: u8, path: &str) -> SemanticDelta {
        SemanticDelta::Tombstoned {
            id: node(id),
            kind: EntityKind::Function,
            at: at(path),
        }
    }

    fn event(generation: u64, sequence: u32, delta: SemanticDelta, result: Option<u8>) -> Event {
        Event {
            clock: Clock { generation, sequence },
            target: delta.id(),
            operation: SemanticOp::Delta(delta),
            result_hash: result.map(hash),
            author: Author {
                fingerprint: AuthorFingerprint([1; 32]),
                kind: AuthorKind::Person,
            },
            labels: Labels::new(),
            origin: Origin::Live,
        }
    }

    fn tape(events: Vec<Event>, paired: bool) -> CandidateTape {
        let ids = events.iter().map(event_id).collect();
        CandidateTape {
            tape: tape_id(&gedo_core::event::Tape { events: ids }),
            events,
            paired,
        }
    }

    fn results(pairs: &[(u8, u8)]) -> impl Fn(AstNodeId) -> Option<StructuralHash> {
        let map: BTreeMap<AstNodeId, StructuralHash> =
            pairs.iter().map(|&(id, h)| (node(id), hash(h))).collect();
        move |id| map.get(&id).copied()
    }

    #[test]
    fn signatures_ignore_the_base_and_see_every_change() {
        let deltas = vec![body(1, 2, 3)];
        let one = semantic_signature(&deltas, &[], results(&[(1, 9)]));
        assert_eq!(one, semantic_signature(&deltas, &[], results(&[(1, 9)])));
        assert_ne!(one, semantic_signature(&deltas, &[], results(&[(1, 8)])));
        assert_ne!(one, semantic_signature(&[body(1, 2, 4)], &[], results(&[(1, 9)])));
    }

    #[test]
    fn a_squash_of_a_created_and_edited_entity_attaches_every_tape_and_links_the_ids() {
        let first = tape(vec![event(5, 0, created(1, "crate::f"), Some(10))], false);
        let second = tape(vec![event(6, 0, body(1, 1, 2), Some(11))], false);
        let changes = vec![SemanticOp::Delta(created(7, "crate::f"))];
        let result = match_commit(
            &changes,
            results(&[(7, 11)]),
            &[second.clone(), first.clone()],
            |id| id,
        );
        let mut expected = vec![first.tape, second.tape];
        expected.sort();
        assert_eq!(result.attached, expected);
        assert_eq!(result.links, vec![(node(1), node(7))]);
        assert!(result.unmatched.is_empty());
    }

    #[test]
    fn an_unrelated_tape_stays_detached() {
        let ours = tape(vec![event(5, 0, body(1, 1, 2), Some(11))], false);
        let theirs = tape(vec![event(5, 1, body(3, 1, 2), Some(12))], false);
        let changes = vec![SemanticOp::Delta(body(1, 0, 2))];
        let result = match_commit(&changes, results(&[(1, 11)]), &[theirs, ours.clone()], |id| id);
        assert_eq!(result.attached, vec![ours.tape]);
    }

    #[test]
    fn without_a_pair_a_partly_explained_tape_is_not_attached_with_a_pair_it_is() {
        let events = vec![
            event(5, 0, body(1, 1, 2), Some(11)),
            event(5, 1, body(2, 1, 2), Some(12)),
        ];
        let changes = vec![SemanticOp::Delta(body(1, 0, 2))];
        let unpaired = match_commit(
            &changes,
            results(&[(1, 11)]),
            &[tape(events.clone(), false)],
            |id| id,
        );
        assert!(unpaired.attached.is_empty());
        assert_eq!(unpaired.unmatched, vec![0]);
        let paired = match_commit(&changes, results(&[(1, 11)]), &[tape(events, true)], |id| id);
        assert_eq!(paired.attached.len(), 1);
        assert!(paired.unmatched.is_empty());
    }

    #[test]
    fn neutral_entities_do_not_stop_a_tape() {
        let events = vec![
            event(5, 0, body(1, 1, 2), Some(11)),
            event(5, 1, created(2, "crate::helper"), Some(20)),
            event(5, 2, tombstoned(2, "crate::helper"), None),
            event(5, 3, body(3, 4, 5), Some(30)),
            event(5, 4, body(3, 5, 4), Some(31)),
        ];
        let changes = vec![SemanticOp::Delta(body(1, 0, 2))];
        let result = match_commit(&changes, results(&[(1, 11)]), &[tape(events, false)], |id| id);
        assert_eq!(result.attached.len(), 1);
    }

    #[test]
    fn a_tape_of_neutral_events_alone_attaches_nowhere() {
        let events = vec![
            event(5, 0, created(2, "crate::helper"), Some(20)),
            event(5, 1, tombstoned(2, "crate::helper"), None),
        ];
        let changes = vec![SemanticOp::Delta(body(1, 0, 2))];
        let result = match_commit(&changes, results(&[(1, 11)]), &[tape(events, false)], |id| id);
        assert!(result.attached.is_empty());
    }

    #[test]
    fn a_tombstone_matches_a_tombstone() {
        let events = vec![event(5, 0, tombstoned(1, "crate::f"), None)];
        let changes = vec![SemanticOp::Delta(tombstoned(1, "crate::f"))];
        let result = match_commit(&changes, results(&[]), &[tape(events, false)], |id| id);
        assert_eq!(result.attached.len(), 1);
        assert!(result.unmatched.is_empty());
    }

    #[test]
    fn changes_the_tapes_do_not_explain_are_unmatched() {
        let events = vec![event(5, 0, body(1, 1, 2), Some(11))];
        let changes = vec![SemanticOp::Delta(body(1, 0, 2)), SemanticOp::Delta(body(4, 0, 2))];
        let result = match_commit(
            &changes,
            results(&[(1, 11), (4, 40)]),
            &[tape(events, true)],
            |id| id,
        );
        assert_eq!(result.accounted, BTreeSet::from([0]));
        assert_eq!(result.unmatched, vec![1]);
    }

    #[test]
    fn a_replayed_commit_matches_the_event_it_replays() {
        let first = tape(vec![event(5, 0, body(1, 1, 2), Some(11))], false);
        let second = tape(vec![event(6, 0, body(1, 2, 3), Some(12))], false);
        let changes = vec![SemanticOp::Delta(body(1, 1, 2))];
        let one = match_commit(
            &changes,
            results(&[(1, 11)]),
            &[first.clone(), second.clone()],
            |id| id,
        );
        assert_eq!(one.attached, vec![first.tape]);
        let changes = vec![SemanticOp::Delta(body(1, 2, 3))];
        let two = match_commit(
            &changes,
            results(&[(1, 12)]),
            std::slice::from_ref(&second),
            |id| id,
        );
        assert_eq!(two.attached, vec![second.tape]);
    }

    /// The known limit of research R7: without a pair, an unrelated tape whose only events are
    /// superseded by a later matched event of the same entity is attached too.
    #[test]
    fn an_unrelated_tape_only_superseded_is_attached_the_known_limit() {
        let ours = tape(vec![event(6, 0, body(1, 1, 2), Some(11))], false);
        let theirs = tape(vec![event(5, 0, body(1, 1, 3), Some(13))], false);
        let changes = vec![SemanticOp::Delta(body(1, 0, 2))];
        let result = match_commit(&changes, results(&[(1, 11)]), &[ours, theirs], |id| id);
        assert_eq!(result.attached.len(), 2);
    }
}
