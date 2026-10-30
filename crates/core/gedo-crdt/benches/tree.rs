//! The tree CRDT alone (SC-004 of `specs/006-tree-crdt`): two sides of 10,000 events each over a
//! base of 10,000 entities, p95 under 50 ms. `GEDO_BENCH_SAMPLES=1` prints every run in order.

use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_core::event::{Author, AuthorKind, Clock, Event, Labels, Origin, SemanticOp};
use gedo_core::identity::{AstNodeId, Location, SemanticDelta};
use gedo_core::syntax::{EntityKind, StructuralHash};
use gedo_crdt::{EntityState, Place, TreeState, converge, fingerprint};

const ENTITIES: u32 = 10_000;
const EVENTS_PER_SIDE: u32 = 10_000;
const RUNS: usize = 25;

fn id(n: u32) -> AstNodeId {
    let mut bytes = [0u8; 16];
    bytes[..4].copy_from_slice(&n.to_be_bytes());
    AstNodeId(bytes)
}

fn hash(n: u32, salt: u8) -> StructuralHash {
    let mut bytes = [salt; 32];
    bytes[..4].copy_from_slice(&n.to_be_bytes());
    StructuralHash(bytes)
}

fn base() -> TreeState {
    let mut state = TreeState::new();
    for n in 0..ENTITIES {
        state.insert(
            id(n),
            EntityState {
                kind: EntityKind::Function,
                present: true,
                place: Place {
                    unit: "src".into(),
                    path: format!("src/m{}.rs", n % 100),
                    container: format!("crate::m{}", n % 100),
                },
                name: format!("f{n}"),
                signature: Some(hash(n, 1)),
                body: Some(hash(n, 2)),
                structural: Some(hash(n, 3)),
            },
        );
    }
    state
}

/// One side: body changes on its own half of the entities, moves on a shared tenth (which commute
/// with the other side's body changes), and new functions.
fn side(number: u8) -> Vec<Event> {
    let author = Author {
        fingerprint: fingerprint(&format!("side{number}@example.com")),
        kind: AuthorKind::Person,
    };
    (0..EVENTS_PER_SIDE)
        .map(|sequence| {
            let n = sequence % ENTITIES;
            let delta = match sequence % 10 {
                0 if number == 0 => SemanticDelta::Moved {
                    id: id(n),
                    from: Location {
                        unit: "src".into(),
                        path: format!("src/m{}.rs", n % 100),
                        qualified_path: format!("crate::m{}::f{n}", n % 100),
                    },
                    to: Location {
                        unit: "src".into(),
                        path: "src/moved.rs".into(),
                        qualified_path: format!("crate::moved::f{n}"),
                    },
                },
                1 => SemanticDelta::Created {
                    id: id(ENTITIES + u32::from(number) * EVENTS_PER_SIDE + sequence),
                    kind: EntityKind::Function,
                    at: Location {
                        unit: "src".into(),
                        path: "src/new.rs".into(),
                        qualified_path: format!("crate::new::s{number}_{sequence}"),
                    },
                },
                _ => SemanticDelta::BodyModified {
                    id: id((n / 2) * 2 + u32::from(number)),
                    before: hash(n, 2),
                    after: hash(n, 10 + number),
                },
            };
            let operation = SemanticOp::Delta(delta);
            Event {
                clock: Clock {
                    generation: 2,
                    sequence,
                },
                target: operation.target(),
                operation,
                result_hash: Some(hash(sequence, 20 + number)),
                author: author.clone(),
                labels: Labels::new(),
                origin: Origin::Live,
            }
        })
        .collect()
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index]
}

fn report(name: &str, mut samples: Vec<Duration>) {
    if std::env::var_os("GEDO_BENCH_SAMPLES").is_some() {
        println!("{name}, every run in order: {samples:?}");
    }
    samples.sort();
    println!(
        "{name} over {} runs: p50 {:?}, p95 {:?}, p99 {:?}",
        samples.len(),
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        percentile(&samples, 0.99),
    );
}

fn tree(criterion: &mut Criterion) {
    let base = base();
    let sides = vec![side(0), side(1)];
    let samples = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let result = converge(&base, &sides).expect("sides fold");
            let elapsed = started.elapsed();
            assert!(result.is_clean(), "{} conflicts", result.conflicts.len());
            elapsed
        })
        .collect();
    report(
        "converge two sides of 10,000 events over 10,000 entities",
        samples,
    );
    criterion.bench_function("converge two sides of 100 events", |bencher| {
        let small: Vec<Vec<Event>> = sides.iter().map(|events| events[..100].to_vec()).collect();
        bencher.iter(|| converge(&base, &small).expect("sides fold"));
    });
}

criterion_group!(benches, tree);
criterion_main!(benches);
