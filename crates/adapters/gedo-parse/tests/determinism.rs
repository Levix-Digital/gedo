//! Determinism (User Story 3): results do not depend on threads or parse order. The snapshots of
//! the conformance suite check the same on every platform.

mod common;

use std::sync::Arc;
use std::thread;

const THREADS: usize = 8;

/// A fixed permutation of `0..len` (linear congruential generator, seeded), so the order differs
/// from the sequential one and is the same on every run.
fn shuffled(len: usize, seed: u64) -> Vec<usize> {
    let mut state = seed;
    let mut order: Vec<usize> = (0..len).collect();
    for index in (1..len).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        order.swap(index, (state >> 33) as usize % (index + 1));
    }
    order
}

#[test]
fn threads_and_order_never_change_a_result() {
    let fixtures = Arc::new(common::fixtures());
    let sequential: Vec<String> = fixtures
        .iter()
        .map(|(path, source)| common::render(&common::syntax_map(path, source)))
        .collect();
    for seed in [1, 2] {
        let order = Arc::new(shuffled(fixtures.len(), seed));
        let workers: Vec<_> = (0..THREADS)
            .map(|worker| {
                let fixtures = Arc::clone(&fixtures);
                let order = Arc::clone(&order);
                thread::spawn(move || {
                    order
                        .iter()
                        .skip(worker)
                        .step_by(THREADS)
                        .map(|&index| {
                            let (path, source) = &fixtures[index];
                            (index, common::render(&common::syntax_map(path, source)))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for worker in workers {
            for (index, rendered) in worker.join().expect("worker finished") {
                assert_eq!(rendered, sequential[index], "{}", fixtures[index].0);
            }
        }
    }
}

#[test]
fn every_result_names_its_hash_format() {
    for (path, source) in common::fixtures() {
        assert!(
            common::render(&common::syntax_map(&path, &source)).contains("hash_format: 1\n"),
            "{path}"
        );
    }
}
