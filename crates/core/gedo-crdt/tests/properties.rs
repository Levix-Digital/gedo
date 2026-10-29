//! Properties of event ids, labels, clocks, filters and reconstruction (User Stories 1, 2, 4, 5).

use gedo_core::event::{
    AgentId, Author, AuthorFingerprint, AuthorKind, Clock, Event, EventId, LabelKey, LabelValue, Labels,
    Origin, SemanticOp, Tape,
};
use gedo_core::identity::{AstNodeId, IdentityMerged, Location, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::syntax::{EntityKind, StructuralHash};
use gedo_crdt::{
    AuthorKindFilter, EventFilter, event_id, fingerprint, format_labels, generation, live_clock,
    parse_labels, reconstructed_events, sort_events, tape_id,
};
use proptest::prelude::*;

fn location(path: &str) -> Location {
    Location {
        unit: "src".into(),
        path: "src/lib.rs".into(),
        qualified_path: path.into(),
    }
}

fn node(n: u8) -> AstNodeId {
    AstNodeId([n; 16])
}

fn hash(n: u8) -> StructuralHash {
    StructuralHash([n; 32])
}

fn delta(kind: u8, n: u8) -> SemanticDelta {
    match kind % 4 {
        0 => SemanticDelta::Created {
            id: node(n),
            kind: EntityKind::Function,
            at: location("crate::f"),
        },
        1 => SemanticDelta::Renamed {
            id: node(n),
            from_name: "f".into(),
            to_name: "g".into(),
            qualified_path: "crate::g".into(),
        },
        2 => SemanticDelta::BodyModified {
            id: node(n),
            before: hash(1),
            after: hash(2),
        },
        _ => SemanticDelta::Tombstoned {
            id: node(n),
            kind: EntityKind::Function,
            at: location("crate::f"),
        },
    }
}

fn event(kind: u8, n: u8, clock: Clock, labels: Labels, agent: Option<&str>) -> Event {
    let operation = SemanticOp::Delta(delta(kind, n));
    Event {
        clock,
        target: operation.target(),
        result_hash: (!operation.has_no_result()).then_some(hash(n)),
        operation,
        author: Author {
            fingerprint: fingerprint("levi@example.com"),
            kind: match agent {
                Some(agent) => AuthorKind::Agent(AgentId::new(agent).expect("valid")),
                None => AuthorKind::Person,
            },
        },
        labels,
        origin: Origin::Live,
    }
}

fn clock(generation: u64, sequence: u32) -> Clock {
    Clock { generation, sequence }
}

fn sample() -> Event {
    event(
        2,
        3,
        clock(4, 1),
        Labels::from_pairs(&[("dc.plan", "gedo_event_model"), ("dc.milestone", "M2")]).expect("valid"),
        Some("worker-1"),
    )
}

#[test]
fn ids_are_uuid_version_8() {
    let id = event_id(&sample());
    assert_eq!(id.0[6] >> 4, 8, "version");
    assert_eq!(id.0[8] >> 6, 0b10, "variant");
}

#[test]
fn encoding_and_ids_are_pinned() {
    let event = sample();
    let encoded = postcard::to_allocvec(&event).expect("encodes");
    let decoded: Event = postcard::from_bytes(&encoded).expect("decodes");
    assert_eq!(decoded, event);
    let tape = Tape {
        events: vec![event_id(&event)],
    };
    let hex: String = encoded.iter().map(|byte| format!("{byte:02x}")).collect();
    insta::assert_snapshot!(format!(
        "event: {hex}\nevent id: {}\ntape id: {}",
        event_id(&event),
        tape_id(&tape)
    ));
}

#[test]
fn every_field_takes_part_in_the_id() {
    let base = sample();
    let id = event_id(&base);
    let mut changes: Vec<Event> = Vec::new();
    let mut changed = base.clone();
    changed.clock.sequence += 1;
    changes.push(changed);
    changes.push(event(2, 4, base.clock, base.labels.clone(), Some("worker-1")));
    let mut changed = base.clone();
    changed.result_hash = Some(hash(9));
    changes.push(changed);
    let mut changed = base.clone();
    changed.author.fingerprint = fingerprint("other@example.com");
    changes.push(changed);
    let mut changed = base.clone();
    changed.author.kind = AuthorKind::Person;
    changes.push(changed);
    let mut changed = base.clone();
    changed.labels = Labels::from_pairs(&[("dc.plan", "gedo_event_model")]).expect("valid");
    changes.push(changed);
    let mut changed = base.clone();
    changed.origin = Origin::Reconstructed;
    changes.push(changed);
    for changed in changes {
        assert_ne!(event_id(&changed), id, "{changed:?}");
    }
    assert_eq!(event_id(&base.clone()), id);
}

#[test]
fn filters_match_labels_kinds_agents_and_fingerprints() {
    let labelled = sample();
    let person = event(0, 1, clock(1, 0), Labels::new(), None);
    let pair = |key: &str, value: &str| {
        let key = LabelKey::new(key).expect("valid");
        let value = LabelValue::new(&key, value).expect("valid");
        (key, value)
    };
    let by_plan = EventFilter {
        labels: vec![pair("dc.plan", "gedo_event_model")],
        ..EventFilter::default()
    };
    assert!(by_plan.matches(&labelled));
    assert!(!by_plan.matches(&person));
    let wrong_value = EventFilter {
        labels: vec![pair("dc.plan", "other")],
        ..EventFilter::default()
    };
    assert!(!wrong_value.matches(&labelled));
    let both = EventFilter {
        labels: vec![pair("dc.plan", "gedo_event_model"), pair("dc.milestone", "M2")],
        ..EventFilter::default()
    };
    assert!(both.matches(&labelled));

    let agents = EventFilter {
        author_kind: Some(AuthorKindFilter::Agent),
        ..EventFilter::default()
    };
    assert!(agents.matches(&labelled) && !agents.matches(&person));
    let people = EventFilter {
        author_kind: Some(AuthorKindFilter::Person),
        ..EventFilter::default()
    };
    assert!(!people.matches(&labelled) && people.matches(&person));
    let worker = EventFilter {
        agent: Some(AgentId::new("worker-1").expect("valid")),
        ..EventFilter::default()
    };
    assert!(worker.matches(&labelled) && !worker.matches(&person));
    let other_agent = EventFilter {
        agent: Some(AgentId::new("worker-2").expect("valid")),
        ..EventFilter::default()
    };
    assert!(!other_agent.matches(&labelled));
    let by_fingerprint = EventFilter {
        fingerprint: Some(AuthorFingerprint([0; 32])),
        ..EventFilter::default()
    };
    assert!(!by_fingerprint.matches(&person));
    assert!(EventFilter::default().matches(&person));
}

#[test]
fn reconstruction_lists_deltas_then_merges_as_recorded() {
    let deltas = [delta(0, 1), delta(1, 2), delta(3, 3)];
    let merges = [IdentityMerged {
        kept: node(1),
        merged: node(5),
        commit: GitOid::from_bytes(&[1; 20]).expect("valid"),
    }];
    let events = reconstructed_events(7, "Levi@Example.com", &deltas, &merges, |id| Some(hash(id.0[0])))
        .expect("built");
    assert_eq!(events.len(), 4);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.clock, clock(7, index as u32));
        assert_eq!(event.origin, Origin::Reconstructed);
        assert_eq!(event.author.kind, AuthorKind::Person);
        assert_eq!(event.author.fingerprint, fingerprint("levi@example.com"));
        assert!(event.labels.is_empty());
        event.check().expect("valid");
    }
    assert_eq!(events[2].result_hash, None, "tombstone");
    assert_eq!(events[3].target, node(5), "merge targets the merged id");
    assert_eq!(events[3].result_hash, None);

    let again = reconstructed_events(7, "levi@example.com", &deltas, &merges, |id| Some(hash(id.0[0])))
        .expect("built");
    let ids = |events: &[Event]| events.iter().map(event_id).collect::<Vec<EventId>>();
    assert_eq!(ids(&again), ids(&events));

    let missing = reconstructed_events(7, "e@x", &deltas, &[], |_| None).expect_err("refused");
    assert_eq!(missing.id, node(1));
}

/// A random commit graph: each commit's parents are earlier commits.
fn graph() -> impl Strategy<Value = Vec<Vec<usize>>> {
    proptest::collection::vec(
        proptest::collection::vec(any::<prop::sample::Index>(), 0..3),
        1..30,
    )
    .prop_map(|commits| {
        commits
            .into_iter()
            .enumerate()
            .map(|(index, parents)| {
                if index == 0 {
                    Vec::new()
                } else {
                    parents.into_iter().map(|parent| parent.index(index)).collect()
                }
            })
            .collect()
    })
}

fn label_text() -> impl Strategy<Value = Vec<(String, String)>> {
    proptest::collection::btree_map("[a-z0-9][a-z0-9._-]{0,10}", "[ -~é,\\\\=]{1,20}", 0..16)
        .prop_map(|map| map.into_iter().collect())
}

proptest! {
    #[test]
    fn generations_increase_along_every_parent_edge(parents in graph()) {
        let mut generations: Vec<u64> = Vec::new();
        for commit_parents in &parents {
            let parent_generations: Vec<u64> = commit_parents.iter().map(|&p| generations[p]).collect();
            let own = generation(&parent_generations);
            for parent in &parent_generations {
                prop_assert!(own > *parent);
            }
            // A live event on top of the commit is later than the commit's own events.
            prop_assert!(live_clock(Some(own), 0) > clock(own, u32::MAX));
            generations.push(own);
        }
    }

    #[test]
    fn sorting_is_total_and_independent_of_input_order(
        specs in proptest::collection::vec((0u8..4, 0u8..6, 1u64..4, 0u32..3), 1..20),
        seed in any::<u64>(),
    ) {
        let events: Vec<Event> = specs
            .iter()
            .map(|&(kind, n, generation, sequence)| event(kind, n, clock(generation, sequence), Labels::new(), None))
            .collect();
        let mut sorted = events.clone();
        sort_events(&mut sorted);
        let mut shuffled = events;
        let len = shuffled.len();
        for index in 0..len {
            let other = (seed.wrapping_mul(index as u64 + 1) % len as u64) as usize;
            shuffled.swap(index, other);
        }
        sort_events(&mut shuffled);
        prop_assert_eq!(&sorted, &shuffled);
        for window in sorted.windows(2) {
            prop_assert!((window[0].clock, event_id(&window[0])) <= (window[1].clock, event_id(&window[1])));
        }
    }

    #[test]
    fn labels_round_trip_through_their_text_form(pairs in label_text()) {
        let labels = Labels::from_pairs(&pairs).expect("valid");
        prop_assert_eq!(parse_labels(&format_labels(&labels)).expect("parses"), labels);
    }

    #[test]
    fn arbitrary_label_texts_never_panic(text in "\\PC{0,64}") {
        let _outcome = parse_labels(&text);
    }

    #[test]
    fn equal_content_gives_equal_ids(kind in 0u8..4, n in any::<u8>(), generation in 1u64..100, sequence in any::<u32>()) {
        let first = event(kind, n, clock(generation, sequence), Labels::new(), None);
        let second = event(kind, n, clock(generation, sequence), Labels::new(), None);
        prop_assert_eq!(event_id(&first), event_id(&second));
    }
}
