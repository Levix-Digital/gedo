//! The tree CRDT (User Stories 1 to 4): scenarios of the acceptance criteria and properties of
//! the convergence (arrival order, duplicates, shared events, permuted sides, idempotence).

use gedo_core::event::{Author, AuthorKind, Clock, Event, EventId, Labels, Origin, SemanticOp};
use gedo_core::identity::{AstNodeId, Location, SemanticDelta};
use gedo_core::syntax::{EntityKind, StructuralHash};
use gedo_crdt::tree::{join_qualified, split_qualified};
use gedo_crdt::{
    ConflictKind, Convergence, EntityState, Part, PartValue, Place, TreeError, TreeState, converge, event_id,
    fingerprint, fold,
};
use proptest::prelude::*;

const FILE: &str = "src/lib.rs";
const IMPL: &str = "crate::<impl Invoice>";

fn id(n: u8) -> AstNodeId {
    AstNodeId([n; 16])
}

fn hash(n: u8) -> StructuralHash {
    StructuralHash([n; 32])
}

fn location(path: &str, qualified_path: &str) -> Location {
    Location {
        unit: "src".into(),
        path: path.into(),
        qualified_path: qualified_path.into(),
    }
}

fn entity(kind: EntityKind, path: &str, qualified_path: &str, n: u8) -> EntityState {
    let (container, name) = split_qualified(qualified_path);
    EntityState {
        kind,
        present: true,
        place: Place {
            unit: "src".into(),
            path: path.into(),
            container: container.into(),
        },
        name: name.into(),
        signature: Some(hash(n)),
        body: Some(hash(n.wrapping_add(100))),
        structural: Some(hash(n.wrapping_add(200))),
    }
}

/// A base with an `impl Invoice` (1) holding `total` (2), and free functions `f` (3) and `g` (4).
fn base() -> TreeState {
    let mut state = TreeState::new();
    state.insert(id(1), entity(EntityKind::Impl, FILE, IMPL, 1));
    state.insert(
        id(2),
        entity(EntityKind::Method, FILE, &format!("{IMPL}::total"), 2),
    );
    state.insert(id(3), entity(EntityKind::Function, FILE, "crate::f", 3));
    state.insert(id(4), entity(EntityKind::Function, FILE, "crate::g", 4));
    state
}

/// An event of side `side`, the `sequence`-th of its generation.
fn event(side: u8, sequence: u32, delta: SemanticDelta, result: Option<u8>) -> Event {
    let operation = SemanticOp::Delta(delta);
    Event {
        clock: Clock {
            generation: 10,
            sequence,
        },
        target: operation.target(),
        result_hash: if operation.has_no_result() {
            None
        } else {
            Some(hash(result.unwrap_or(sequence as u8)))
        },
        operation,
        author: Author {
            fingerprint: fingerprint(&format!("side{side}@example.com")),
            kind: AuthorKind::Person,
        },
        labels: Labels::new(),
        origin: Origin::Live,
    }
}

fn created(side: u8, seq: u32, n: u8, kind: EntityKind, qualified_path: &str, result: u8) -> Event {
    event(
        side,
        seq,
        SemanticDelta::Created {
            id: id(n),
            kind,
            at: location(FILE, qualified_path),
        },
        Some(result),
    )
}

fn body(side: u8, seq: u32, n: u8, after: u8) -> Event {
    event(
        side,
        seq,
        SemanticDelta::BodyModified {
            id: id(n),
            before: hash(0),
            after: hash(after),
        },
        Some(after),
    )
}

fn signature(side: u8, seq: u32, n: u8, after: u8) -> Event {
    event(
        side,
        seq,
        SemanticDelta::SignatureMutated {
            id: id(n),
            before: hash(0),
            after: hash(after),
        },
        Some(after),
    )
}

fn removed(side: u8, seq: u32, n: u8, qualified_path: &str) -> Event {
    event(
        side,
        seq,
        SemanticDelta::Tombstoned {
            id: id(n),
            kind: EntityKind::Function,
            at: location(FILE, qualified_path),
        },
        None,
    )
}

fn moved(side: u8, seq: u32, n: u8, from: &str, to_path: &str, to: &str) -> Event {
    event(
        side,
        seq,
        SemanticDelta::Moved {
            id: id(n),
            from: location(FILE, from),
            to: location(to_path, to),
        },
        Some(seq as u8 + 50),
    )
}

fn renamed(side: u8, seq: u32, n: u8, from: &str, to: &str, qualified_path: &str) -> Event {
    event(
        side,
        seq,
        SemanticDelta::Renamed {
            id: id(n),
            from_name: from.into(),
            to_name: to.into(),
            qualified_path: qualified_path.into(),
        },
        Some(seq as u8 + 60),
    )
}

fn merged(side: u8, seq: u32, kept: u8, alias: u8) -> Event {
    let operation = SemanticOp::IdentityMerged {
        kept: id(kept),
        merged: id(alias),
    };
    Event {
        clock: Clock {
            generation: 10,
            sequence: seq,
        },
        target: operation.target(),
        result_hash: None,
        operation,
        author: Author {
            fingerprint: fingerprint(&format!("side{side}@example.com")),
            kind: AuthorKind::Person,
        },
        labels: Labels::new(),
        origin: Origin::Live,
    }
}

fn run(sides: &[Vec<Event>]) -> Convergence {
    converge(&base(), sides).expect("sides fold")
}

fn get(convergence: &Convergence, n: u8) -> &EntityState {
    convergence.state.entity(id(n)).expect("entity in the state")
}

// User Story 1: members of a structure behave as a set.

#[test]
fn additions_to_the_same_container_both_apply() {
    let result = run(&[
        vec![created(1, 0, 10, EntityKind::Method, &format!("{IMPL}::tax"), 10)],
        vec![created(2, 0, 11, EntityKind::Method, &format!("{IMPL}::due"), 11)],
    ]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    assert_eq!(result.state.members(FILE, IMPL), vec![id(2), id(10), id(11)]);
}

#[test]
fn a_removal_and_an_addition_in_the_same_container_both_apply() {
    let result = run(&[
        vec![removed(1, 0, 2, &format!("{IMPL}::total"))],
        vec![created(2, 0, 11, EntityKind::Method, &format!("{IMPL}::due"), 11)],
    ]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    assert_eq!(result.state.members(FILE, IMPL), vec![id(11)]);
}

#[test]
fn a_removal_on_both_sides_applies_once() {
    let result = run(&[
        vec![removed(1, 0, 3, "crate::f")],
        vec![removed(2, 3, 3, "crate::f")],
    ]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    assert!(!get(&result, 3).present);
}

#[test]
fn the_same_entity_added_on_both_sides_is_one_entity() {
    let same = run(&[
        vec![created(1, 0, 10, EntityKind::Function, "crate::h", 7)],
        vec![created(2, 4, 10, EntityKind::Function, "crate::h", 7)],
    ]);
    assert!(same.is_clean(), "{:?}", same.conflicts);
    assert!(get(&same, 10).present);

    let linked = run(&[
        vec![created(1, 0, 12, EntityKind::Function, "crate::h", 7)],
        vec![created(2, 0, 10, EntityKind::Function, "crate::h", 7)],
    ]);
    assert!(linked.is_clean(), "{:?}", linked.conflicts);
    assert_eq!(linked.links, vec![(id(10), id(12))]);
    assert!(linked.state.entity(id(10)).is_some());
    assert!(linked.state.entity(id(12)).is_none());
}

// User Story 2: a move and an edit of the same entity commute.

#[test]
fn a_move_and_a_body_change_commute() {
    let result = run(&[
        vec![moved(1, 0, 3, "crate::f", "src/other.rs", "crate::other::f")],
        vec![body(2, 0, 3, 77)],
    ]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    let f = get(&result, 3);
    assert_eq!(f.place.path, "src/other.rs");
    assert_eq!(f.qualified_path(), "crate::other::f");
    assert_eq!(f.body, Some(hash(77)));
    assert_eq!(f.structural, None, "combined from two sides");
}

#[test]
fn a_rename_and_a_move_commute() {
    let result = run(&[
        vec![renamed(1, 0, 3, "f", "h", "crate::h")],
        vec![moved(2, 0, 3, "crate::f", "src/other.rs", "crate::other::f")],
    ]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    assert_eq!(get(&result, 3).qualified_path(), "crate::other::h");
}

#[test]
fn signature_and_body_from_different_sides_converge() {
    let result = run(&[vec![signature(1, 0, 3, 31)], vec![body(2, 0, 3, 32)]]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    let f = get(&result, 3);
    assert_eq!((f.signature, f.body), (Some(hash(31)), Some(hash(32))));
}

#[test]
fn a_change_on_one_side_applies_in_either_order() {
    let change = vec![body(1, 0, 4, 40)];
    let one = run(&[change.clone(), vec![]]);
    let other = run(&[vec![], change]);
    assert_eq!(one.state, other.state);
    assert_eq!(get(&one, 4).body, Some(hash(40)));
    assert_eq!(
        get(&one, 4).structural,
        Some(hash(40)),
        "one side holds every part"
    );
}

#[test]
fn the_same_change_on_both_sides_applies_once() {
    let result = run(&[vec![body(1, 0, 4, 40)], vec![body(2, 5, 4, 40)]]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    assert_eq!(get(&result, 4).body, Some(hash(40)));
}

// User Story 3: incompatible changes are reported, never hidden.

#[test]
fn two_bodies_conflict_and_keep_the_base() {
    let ours = body(1, 0, 3, 41);
    let theirs = body(2, 0, 3, 42);
    let result = run(&[vec![ours.clone()], vec![theirs.clone()]]);
    assert_eq!(result.conflicts.len(), 1);
    let conflict = &result.conflicts[0];
    assert_eq!(
        (conflict.entity, conflict.kind),
        (id(3), ConflictKind::Part(Part::Body))
    );
    assert_eq!(conflict.base, Some(PartValue::Body(Some(hash(103)))));
    assert_eq!(conflict.values.len(), 2);
    assert_eq!(conflict.values[0].sides, vec![0]);
    assert_eq!(conflict.values[0].events, vec![event_id(&ours)]);
    assert_eq!(conflict.values[1].events, vec![event_id(&theirs)]);
    assert_eq!(get(&result, 3).body, Some(hash(103)));
}

#[test]
fn a_removal_against_a_change_conflicts_and_keeps_the_entity() {
    let result = run(&[vec![removed(1, 0, 3, "crate::f")], vec![body(2, 0, 3, 41)]]);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].kind, ConflictKind::Part(Part::Presence));
    assert_eq!(get(&result, 3), base().entity(id(3)).expect("in base"));
}

#[test]
fn a_removed_container_against_a_new_member_conflicts() {
    let result = run(&[
        vec![
            removed(1, 0, 1, IMPL),
            removed(1, 1, 2, &format!("{IMPL}::total")),
        ],
        vec![created(2, 0, 11, EntityKind::Method, &format!("{IMPL}::due"), 11)],
    ]);
    assert_eq!(result.conflicts.len(), 1, "{:?}", result.conflicts);
    let conflict = &result.conflicts[0];
    assert_eq!(
        (conflict.entity, conflict.kind),
        (id(1), ConflictKind::RemovedContainer)
    );
    assert_eq!(conflict.others, vec![id(11)]);
    assert!(get(&result, 1).present, "the container keeps its base state");
}

#[test]
fn different_entities_added_on_both_sides_conflict() {
    let result = run(&[
        vec![created(1, 0, 12, EntityKind::Function, "crate::h", 7)],
        vec![created(2, 0, 10, EntityKind::Function, "crate::h", 8)],
    ]);
    assert_eq!(result.conflicts.len(), 1);
    let conflict = &result.conflicts[0];
    assert_eq!(
        (conflict.entity, conflict.kind),
        (id(10), ConflictKind::AddedOnBothSides)
    );
    assert_eq!(conflict.others, vec![id(12)]);
    assert!(result.state.entity(id(10)).is_none() && result.state.entity(id(12)).is_none());
}

#[test]
fn implementation_blocks_added_on_both_sides_do_not_conflict() {
    let result = run(&[
        vec![created(1, 0, 12, EntityKind::Impl, "crate::<impl Order>", 7)],
        vec![created(2, 0, 10, EntityKind::Impl, "crate::<impl Order>", 8)],
    ]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
}

#[test]
fn a_conflict_leaves_unrelated_changes_applied() {
    let result = run(&[
        vec![
            body(1, 0, 3, 41),
            created(1, 1, 10, EntityKind::Method, &format!("{IMPL}::tax"), 10),
        ],
        vec![body(2, 0, 3, 42), removed(2, 1, 4, "crate::g")],
    ]);
    assert_eq!(result.conflicts.len(), 1);
    assert!(get(&result, 10).present);
    assert!(!get(&result, 4).present);
}

// User Story 4 and edge cases.

#[test]
fn three_sides_converge_part_by_part() {
    let result = run(&[
        vec![body(1, 0, 3, 41)],
        vec![body(2, 0, 3, 41)],
        vec![body(3, 0, 3, 43)],
    ]);
    assert_eq!(result.conflicts.len(), 1);
    let values = &result.conflicts[0].values;
    assert_eq!(values[0].sides, vec![0, 1]);
    assert_eq!(values[1].sides, vec![2]);
}

#[test]
fn entities_created_and_removed_on_a_side_leave_no_trace() {
    let side = vec![
        created(1, 0, 10, EntityKind::Function, "crate::tmp", 9),
        removed(1, 1, 10, "crate::tmp"),
    ];
    let folded = fold(&base(), &side).expect("folds");
    assert!(folded.neutral.contains(&id(10)));
    let result = run(&[side, vec![]]);
    assert!(result.state.entity(id(10)).is_none());
}

#[test]
fn identity_merges_make_two_ids_one_entity() {
    // Side 2 knows f as 9 and merges it into 3; side 1 edits 3.
    let result = run(&[
        vec![body(1, 0, 3, 41)],
        vec![merged(2, 0, 3, 9), signature(2, 1, 9, 51)],
    ]);
    assert!(result.is_clean(), "{:?}", result.conflicts);
    let f = get(&result, 3);
    assert_eq!((f.signature, f.body), (Some(hash(51)), Some(hash(41))));
}

#[test]
fn an_event_about_an_unknown_entity_is_an_error() {
    let stray = body(2, 0, 99, 1);
    let error = converge(&base(), &[vec![], vec![stray.clone()]]).expect_err("unknown entity");
    assert_eq!(error.side, 1);
    assert_eq!(
        error.error,
        TreeError::UnknownEntity {
            event: event_id(&stray),
            entity: id(99)
        }
    );
}

#[test]
fn qualified_paths_round_trip() {
    for path in ["crate::m::f", "f", "crate::<impl a::B for C>::g"] {
        let (container, name) = split_qualified(path);
        assert_eq!(join_qualified(container, name), path);
    }
}

// Properties (research R11).

/// What a generated side does, on base entities 1 to 4 or on the side's own new entities.
#[derive(Clone, Debug)]
enum Op {
    Body(u8, u8),
    Signature(u8, u8),
    Move(u8, u8),
    Rename(u8, u8),
    Remove(u8),
    Create(u8, u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (1u8..=4, 0u8..3).prop_map(|(n, v)| Op::Body(n, v)),
        (1u8..=4, 0u8..3).prop_map(|(n, v)| Op::Signature(n, v)),
        (3u8..=4, 0u8..2).prop_map(|(n, v)| Op::Move(n, v)),
        (3u8..=4, 0u8..2).prop_map(|(n, v)| Op::Rename(n, v)),
        (2u8..=4).prop_map(Op::Remove),
        (0u8..3, 0u8..2).prop_map(|(k, v)| Op::Create(k, v)),
    ]
}

/// The events of a side, in recording order.
fn events_of(side: u8, ops: &[Op]) -> Vec<Event> {
    let mut events = Vec::new();
    for (sequence, op) in ops.iter().enumerate() {
        let seq = sequence as u32;
        let qualified = |n: u8| match n {
            1 => IMPL.to_owned(),
            2 => format!("{IMPL}::total"),
            3 => "crate::f".to_owned(),
            _ => "crate::g".to_owned(),
        };
        events.push(match *op {
            Op::Body(n, v) => body(side, seq, n, 20 + v),
            Op::Signature(n, v) => signature(side, seq, n, 30 + v),
            Op::Move(n, v) => moved(
                side,
                seq,
                n,
                &qualified(n),
                FILE,
                &format!("crate::m{v}::{}", split_qualified(&qualified(n)).1),
            ),
            Op::Rename(n, v) => renamed(side, seq, n, "x", &format!("r{v}"), &format!("crate::r{v}")),
            Op::Remove(n) => removed(side, seq, n, &qualified(n)),
            Op::Create(k, v) => created(
                side,
                seq,
                40 + k,
                EntityKind::Function,
                &format!("crate::new{k}"),
                60 + v,
            ),
        });
    }
    events
}

fn sides() -> impl Strategy<Value = Vec<Vec<Op>>> {
    prop::collection::vec(prop::collection::vec(op(), 0..8), 1..4)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arrival_order_and_duplicates_do_not_matter(ops in sides(), seed in any::<u64>()) {
        let sides: Vec<Vec<Event>> = ops.iter().enumerate().map(|(i, o)| events_of(i as u8, o)).collect();
        let expected = run(&sides);
        let shuffled: Vec<Vec<Event>> = sides
            .iter()
            .map(|events| {
                let mut events: Vec<Event> = events.iter().chain(events.iter().take(2)).cloned().collect();
                let len = events.len().max(1) as u64;
                events.rotate_left((seed % len) as usize);
                events.reverse();
                events
            })
            .collect();
        prop_assert_eq!(run(&shuffled), expected);
    }

    #[test]
    fn permuting_sides_permutes_only_their_positions(ops in sides()) {
        let sides: Vec<Vec<Event>> = ops.iter().enumerate().map(|(i, o)| events_of(i as u8, o)).collect();
        let forward = run(&sides);
        let reversed: Vec<Vec<Event>> = sides.iter().rev().cloned().collect();
        let mut backward = run(&reversed);
        let last = sides.len() - 1;
        for conflict in &mut backward.conflicts {
            for value in &mut conflict.values {
                let mut pairs: Vec<(usize, EventId)> =
                    value.sides.iter().map(|side| last - side).zip(value.events.iter().copied()).collect();
                pairs.sort();
                value.sides = pairs.iter().map(|(side, _)| *side).collect();
                value.events = pairs.iter().map(|(_, event)| *event).collect();
            }
        }
        prop_assert_eq!(backward, forward);
    }

    #[test]
    fn a_side_converges_with_itself_and_with_nothing(ops in prop::collection::vec(op(), 0..10)) {
        let side = events_of(0, &ops);
        let alone = run(std::slice::from_ref(&side));
        prop_assert!(alone.is_clean());
        let twice = run(&[side.clone(), side.clone()]);
        prop_assert!(twice.is_clean());
        prop_assert_eq!(&twice.state, &alone.state);
        let empty = run(&[side, vec![]]);
        prop_assert!(empty.is_clean());
        prop_assert_eq!(&empty.state, &alone.state);
    }

    #[test]
    fn events_shared_by_every_side_apply_once(shared in prop::collection::vec(op(), 0..6), count in 2usize..4) {
        let events = events_of(0, &shared);
        let alone = run(std::slice::from_ref(&events));
        let copies = vec![events; count];
        let together = run(&copies);
        prop_assert!(together.is_clean());
        prop_assert_eq!(together.state, alone.state);
    }

    #[test]
    fn no_change_of_a_single_side_is_lost(ops in sides()) {
        let sides: Vec<Vec<Event>> = ops.iter().enumerate().map(|(i, o)| events_of(i as u8, o)).collect();
        let result = run(&sides);
        let base = base();
        let folded: Vec<_> = sides.iter().map(|events| fold(&base, events).expect("folds")).collect();
        for (index, side) in folded.iter().enumerate() {
            for (entity, version) in &side.touched {
                let Some(original) = base.entity(*entity) else { continue };
                let others_touch = folded.iter().enumerate().any(|(other, s)| other != index && s.touched.contains_key(entity));
                let in_conflict = result.conflicts.iter().any(|c| c.entity == *entity || c.others.contains(entity));
                if others_touch || in_conflict {
                    continue;
                }
                let converged = result.state.entity(*entity).expect("base entity kept");
                for part in gedo_crdt::tree::PARTS {
                    if version.value(part) != original.value(part) {
                        prop_assert_eq!(converged.value(part), version.value(part));
                    }
                }
            }
        }
    }

    #[test]
    fn fewer_sides_never_add_conflicts(ops in sides()) {
        let sides: Vec<Vec<Event>> = ops.iter().enumerate().map(|(i, o)| events_of(i as u8, o)).collect();
        if run(&sides).is_clean() {
            for skip in 0..sides.len() {
                let fewer: Vec<Vec<Event>> = sides.iter().enumerate().filter(|(i, _)| *i != skip).map(|(_, s)| s.clone()).collect();
                if !fewer.is_empty() {
                    prop_assert!(run(&fewer).is_clean());
                }
            }
        }
    }
}

#[test]
fn a_fixed_convergence_is_pinned() {
    let result = run(&[
        vec![
            body(1, 0, 3, 41),
            created(1, 1, 10, EntityKind::Method, &format!("{IMPL}::tax"), 10),
            moved(1, 2, 4, "crate::g", "src/other.rs", "crate::other::g"),
        ],
        vec![
            body(2, 0, 3, 42),
            renamed(2, 1, 4, "g", "h", "crate::h"),
            removed(2, 2, 2, &format!("{IMPL}::total")),
        ],
        vec![
            created(3, 0, 12, EntityKind::Function, "crate::k", 7),
            signature(3, 1, 4, 33),
        ],
    ]);
    insta::assert_debug_snapshot!(result);
}
