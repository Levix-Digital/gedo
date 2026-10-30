//! Property-based tests of the re-attachment rules (`specs/005-history-reattachment/research.md`
//! R4 to R7): generated branches of live events and the squash commit a server would make of them.

use std::collections::BTreeMap;

use gedo_core::event::{
    Author, AuthorFingerprint, AuthorKind, Clock, Event, Labels, Origin, SemanticOp, Tape,
};
use gedo_core::identity::{AstNodeId, Location, SemanticDelta};
use gedo_core::syntax::{EntityKind, StructuralHash};
use gedo_crdt::{CandidateTape, event_id, match_commit, tape_id};
use proptest::prelude::*;

fn node(n: u16) -> AstNodeId {
    let mut bytes = [0u8; 16];
    bytes[..2].copy_from_slice(&n.to_be_bytes());
    AstNodeId(bytes)
}

fn hash(n: u16) -> StructuralHash {
    let mut bytes = [0u8; 32];
    bytes[..2].copy_from_slice(&n.to_be_bytes());
    StructuralHash(bytes)
}

fn at(n: u16) -> Location {
    Location {
        unit: "src".into(),
        path: "src/lib.rs".into(),
        qualified_path: format!("crate::f{n}"),
    }
}

/// What a step of the branch does to one entity.
#[derive(Clone, Debug)]
enum Step {
    /// Changes the body of entity `n` to `value` (creating it when it is new and absent).
    Edit(u16, u16),
    /// Removes entity `n` when it exists.
    Remove(u16),
}

fn steps() -> impl Strategy<Value = Vec<Vec<Step>>> {
    let step = prop_oneof![
        4 => (0u16..8, 1u16..5).prop_map(|(n, value)| Step::Edit(n, value)),
        1 => (0u16..8).prop_map(Step::Remove),
    ];
    prop::collection::vec(prop::collection::vec(step, 1..5), 1..5)
}

/// Entities 0 to 3 exist before the branch with value 0; 4 to 7 are new.
fn preexisting(n: u16) -> bool {
    n < 4
}

struct Branch {
    tapes: Vec<CandidateTape>,
    /// The squash commit's changes and the structural hash of each entity in it.
    changes: Vec<SemanticOp>,
    results: BTreeMap<AstNodeId, StructuralHash>,
    /// Tapes with an event of an entity whose net effect is a change.
    effective: Vec<gedo_core::event::TapeId>,
    created_alive: usize,
}

fn event(generation: u64, sequence: u32, delta: SemanticDelta, result: Option<StructuralHash>) -> Event {
    Event {
        clock: Clock { generation, sequence },
        target: delta.id(),
        operation: SemanticOp::Delta(delta),
        result_hash: result,
        author: Author {
            fingerprint: AuthorFingerprint([1; 32]),
            kind: AuthorKind::Person,
        },
        labels: Labels::new(),
        origin: Origin::Live,
    }
}

/// Live events of the branch, one tape per commit, and its squash.
fn branch(commits: &[Vec<Step>], offset: u16) -> Branch {
    let mut state: BTreeMap<u16, Option<u16>> = (0..8).map(|n| (n, preexisting(n).then_some(0))).collect();
    let mut tapes = Vec::new();
    let mut touched: Vec<Vec<u16>> = Vec::new();
    for (index, steps) in commits.iter().enumerate() {
        let mut events = Vec::new();
        let mut entities = Vec::new();
        for step in steps {
            let sequence = u32::try_from(events.len()).expect("small");
            let generation = 10 + index as u64;
            match *step {
                Step::Edit(n, value) => {
                    let id = node(offset + n);
                    let value = value + offset;
                    let delta = match state[&n] {
                        Some(before) if before != value => SemanticDelta::BodyModified {
                            id,
                            before: hash(before),
                            after: hash(value),
                        },
                        Some(_) => continue,
                        None => SemanticDelta::Created {
                            id,
                            kind: EntityKind::Function,
                            at: at(offset + n),
                        },
                    };
                    events.push(event(generation, sequence, delta, Some(hash(value))));
                    state.insert(n, Some(value));
                    entities.push(n);
                }
                Step::Remove(n) => {
                    if state[&n].is_none() {
                        continue;
                    }
                    let delta = SemanticDelta::Tombstoned {
                        id: node(offset + n),
                        kind: EntityKind::Function,
                        at: at(offset + n),
                    };
                    events.push(event(generation, sequence, delta, None));
                    state.insert(n, None);
                    entities.push(n);
                }
            }
        }
        if events.is_empty() {
            continue;
        }
        let ids = events.iter().map(event_id).collect();
        tapes.push(CandidateTape {
            tape: tape_id(&Tape { events: ids }),
            events,
            paired: false,
        });
        touched.push(entities);
    }

    // The squash: the net effect of the branch, with new ids for entities created on it.
    let mut changes = Vec::new();
    let mut results = BTreeMap::new();
    let mut effective_entities = Vec::new();
    let mut created_alive = 0;
    let ever: Vec<u16> = touched.iter().flatten().copied().collect();
    for (&n, &last) in &state {
        if !ever.contains(&n) {
            continue;
        }
        let before = preexisting(n).then_some(0u16);
        let id = node(offset + n);
        match (before, last) {
            (Some(old), Some(new)) if old != new => {
                changes.push(SemanticOp::Delta(SemanticDelta::BodyModified {
                    id,
                    before: hash(old),
                    after: hash(new),
                }));
                results.insert(id, hash(new));
                effective_entities.push(n);
            }
            (Some(_), None) => {
                changes.push(SemanticOp::Delta(SemanticDelta::Tombstoned {
                    id,
                    kind: EntityKind::Function,
                    at: at(offset + n),
                }));
                effective_entities.push(n);
            }
            (None, Some(new)) => {
                let squashed = node(offset + n + 1000);
                changes.push(SemanticOp::Delta(SemanticDelta::Created {
                    id: squashed,
                    kind: EntityKind::Function,
                    at: at(offset + n),
                }));
                results.insert(squashed, hash(new));
                effective_entities.push(n);
                created_alive += 1;
            }
            _ => {}
        }
    }
    let effective = tapes
        .iter()
        .zip(&touched)
        .filter(|(_, entities)| entities.iter().any(|n| effective_entities.contains(n)))
        .map(|(tape, _)| tape.tape)
        .collect();
    Branch {
        tapes,
        changes,
        results,
        effective,
        created_alive,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn a_squash_attaches_every_tape_with_an_effect_and_explains_every_change(commits in steps()) {
        let branch = branch(&commits, 0);
        let result = match_commit(&branch.changes, |id| branch.results.get(&id).copied(), &branch.tapes, |id| id);
        let mut effective = branch.effective.clone();
        effective.sort();
        effective.dedup();
        // Tapes whose every event concerns an entity without net effect attach nowhere; the
        // others all attach.
        for tape in &effective {
            prop_assert!(result.attached.contains(tape), "{tape:?} not attached: {result:?}");
        }
        prop_assert!(result.unmatched.is_empty(), "{result:?}");
        prop_assert_eq!(result.links.len(), branch.created_alive);
        for (kept, merged) in &result.links {
            prop_assert!(kept < merged);
        }
    }

    #[test]
    fn the_result_does_not_depend_on_the_order_of_the_candidates(commits in steps(), seed in any::<u64>()) {
        let branch = branch(&commits, 0);
        let reference = match_commit(&branch.changes, |id| branch.results.get(&id).copied(), &branch.tapes, |id| id);
        let mut shuffled = branch.tapes.clone();
        let mut state = seed;
        for index in (1..shuffled.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            shuffled.swap(index, (state >> 33) as usize % (index + 1));
        }
        let again = match_commit(&branch.changes, |id| branch.results.get(&id).copied(), &shuffled, |id| id);
        prop_assert_eq!(again, reference);
    }

    #[test]
    fn an_unrelated_branch_on_other_entities_changes_nothing(ours in steps(), theirs in steps()) {
        let branch = branch(&ours, 0);
        let other = branch_of_others(&theirs);
        let alone = match_commit(&branch.changes, |id| branch.results.get(&id).copied(), &branch.tapes, |id| id);
        let mut candidates = branch.tapes.clone();
        candidates.extend(other.tapes.iter().cloned());
        let together = match_commit(&branch.changes, |id| branch.results.get(&id).copied(), &candidates, |id| id);
        for tape in &other.tapes {
            if !branch.tapes.iter().any(|ours| ours.tape == tape.tape) {
                prop_assert!(!together.attached.contains(&tape.tape));
            }
        }
        prop_assert_eq!(together.accounted, alone.accounted);
        prop_assert_eq!(together.links, alone.links);
    }
}

/// A branch on entities disjoint from [`branch`]'s (ids from 100).
fn branch_of_others(commits: &[Vec<Step>]) -> Branch {
    branch(commits, 100)
}
