//! Events reconstructed for commits made without Gedo (User Stories 4 and 5, SC-001).

mod support;

use std::path::Path;

use gedo_app::{CommitEvents, EventLog, IdentityIndex};
use gedo_core::event::{AuthorKind, Event, Labels, Origin, SemanticOp};
use gedo_core::identity::SemanticDelta;
use gedo_core::ids::GitOid;
use gedo_core::memory::{MemoryCache, MemoryRefStore, MemoryWal};
use gedo_core::ports::ObjectStore;
use gedo_crdt::{RecordingContext, event_id, fingerprint};
use gedo_parse::TreeSitterParser;
use gedo_store::{GitObjectStore, GitRepository};
use proptest::prelude::*;
use support::{History, files, generated};

/// The reconstructed events of `commits`, rendered one per line.
fn render(history: &History, cache: &mut MemoryCache, commits: &[GitOid]) -> String {
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    let index = IdentityIndex::open(&history.objects, &history.parser, cache).expect("index opens");
    let mut text = String::new();
    for commit in commits {
        let CommitEvents::Reconstructed(events) = log.events_of_commit(commit, &index).expect("events")
        else {
            panic!("nothing was published");
        };
        for event in &events {
            text.push_str(&format!("{commit} {} {:?}\n", event_id(event), event.clock));
        }
    }
    text
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn reconstruction_does_not_depend_on_the_indexing_order(seed in any::<u64>(), split in 1usize..12) {
        let (history, commits) = generated(seed);
        let reference = {
            let mut cache = history.index(&[&commits]);
            render(&history, &mut cache, &commits)
        };
        let reversed: Vec<GitOid> = commits.iter().rev().copied().collect();
        let mut by_tip = history.index(&[&reversed]);
        prop_assert_eq!(render(&history, &mut by_tip, &commits), reference.clone());
        let at = split.min(commits.len());
        let mut piecewise = history.index(&[&commits[..at], &reversed[..1], &commits]);
        prop_assert_eq!(render(&history, &mut piecewise, &commits), reference);
    }

    #[test]
    fn events_of_a_commit_are_later_than_those_of_its_parents(seed in any::<u64>()) {
        let (history, commits) = generated(seed);
        let cache = history.index(&[&commits]);
        let refs = MemoryRefStore::new();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
        for commit in &commits {
            let own = log.generation(commit).expect("generation");
            for parent in history.objects.read_commit(commit).expect("commit").parents {
                prop_assert!(own > log.generation(&parent).expect("generation"));
            }
        }
        let index = IdentityIndex::open(&history.objects, &history.parser, &cache).expect("opens");
        for commit in &commits {
            let generation = log.generation(commit).expect("generation");
            for event in log.events_of_commit(commit, &index).expect("events").events() {
                prop_assert_eq!(event.clock.generation, generation);
            }
        }
    }
}

#[test]
fn reconstructed_events_describe_the_commit_as_its_author() {
    let history = History::default();
    history.objects.set_author_email("Dana@Example.com");
    let root = history.commit(&[], &files(&[("src/lib.rs", "pub fn f() -> u32 {\n    1\n}\n")]));
    let edit = history.commit(
        &[root],
        &files(&[
            ("src/lib.rs", "pub fn g() -> u32 {\n    2\n}\n"),
            ("README.md", "x"),
        ]),
    );
    let docs = history.commit(
        &[edit],
        &files(&[
            ("src/lib.rs", "pub fn g() -> u32 {\n    2\n}\n"),
            ("README.md", "y"),
        ]),
    );
    let cache = history.index(&[&[docs]]);
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    let index = IdentityIndex::open(&history.objects, &history.parser, &cache).expect("opens");

    let events: Vec<Event> = log
        .events_of_commit(&edit, &index)
        .expect("events")
        .events()
        .into_iter()
        .cloned()
        .collect();
    let deltas = index.changes_of(&edit).expect("indexed").deltas;
    assert_eq!(events.len(), deltas.len());
    for (sequence, (event, delta)) in events.iter().zip(&deltas).enumerate() {
        assert_eq!(event.operation, SemanticOp::Delta(delta.clone()));
        assert_eq!(event.clock.generation, 2);
        assert_eq!(event.clock.sequence as usize, sequence);
        assert_eq!(event.origin, Origin::Reconstructed);
        assert_eq!(event.author.kind, AuthorKind::Person);
        assert_eq!(event.author.fingerprint, fingerprint("dana@example.com"));
        assert!(event.labels.is_empty());
        event.check().expect("valid");
    }
    assert!(
        events
            .iter()
            .any(|event| matches!(event.operation, SemanticOp::Delta(SemanticDelta::Renamed { .. })))
    );
    assert_eq!(
        log.events_of_commit(&docs, &index).expect("events"),
        CommitEvents::Reconstructed(Vec::new()),
        "a commit without Rust changes has no events"
    );
}

#[test]
fn identity_merges_become_events_with_the_ids_recorded_at_the_commit() {
    let history = History::default();
    let base = history.commit(&[], &files(&[("src/lib.rs", "pub fn base() {}\n")]));
    let with = |body: u32| format!("pub fn base() {{}}\n\npub fn f() -> u32 {{\n    {body}\n}}\n");
    let left = history.commit(&[base], &files(&[("src/lib.rs", &with(1))]));
    let right = history.commit(&[base], &files(&[("src/lib.rs", &with(2))]));
    let merge = history.commit(&[left, right], &files(&[("src/lib.rs", &with(1))]));

    // The events of `right` are the same whether or not the later merge is indexed.
    let mut before = history.index(&[&[right]]);
    let mut after = history.index(&[&[merge]]);
    assert_eq!(
        render(&history, &mut before, &[right]),
        render(&history, &mut after, &[right])
    );

    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    let index = IdentityIndex::open(&history.objects, &history.parser, &after).expect("opens");
    let events = log.events_of_commit(&merge, &index).expect("events");
    let merges: Vec<&Event> = events
        .events()
        .into_iter()
        .filter(|event| matches!(event.operation, SemanticOp::IdentityMerged { .. }))
        .collect();
    assert_eq!(merges.len(), 1, "{events:?}");
    assert_eq!(merges[0].result_hash, None);
    assert_eq!(merges[0].clock.generation, 3);
}

#[test]
fn a_published_commit_returns_its_tapes_instead_of_reconstructing() {
    let history = History::default();
    let root = history.commit(&[], &files(&[("src/lib.rs", "pub fn f() {}\n")]));
    let next = history.commit(&[root], &files(&[("src/lib.rs", "pub fn g() {}\n")]));
    let cache = history.index(&[&[next]]);
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    let index = IdentityIndex::open(&history.objects, &history.parser, &cache).expect("opens");
    let CommitEvents::Reconstructed(reconstructed) = log.events_of_commit(&next, &index).expect("events")
    else {
        panic!("nothing published yet");
    };
    let ops: Vec<_> = reconstructed
        .iter()
        .map(|event| (event.operation.clone(), event.result_hash))
        .collect();
    let context = RecordingContext::person("levi@example.com", Labels::new()).expect("valid");
    let ids = log.record(&context, Some(root), &ops).expect("recorded");
    let tape = log.publish(next).expect("published").expect("tape");
    match log.events_of_commit(&next, &index).expect("events") {
        CommitEvents::Published(tapes) => {
            assert_eq!(tapes.len(), 1);
            assert_eq!(tapes[0].0, tape);
            assert_eq!(tapes[0].1.iter().map(event_id).collect::<Vec<_>>(), ids);
            assert!(tapes[0].1.iter().all(|event| event.origin == Origin::Live));
        }
        other => panic!("{other:?}"),
    }
}

/// Commits of this repository whose reconstructed events the snapshot records: the first commit
/// of feature 003 (a large change) and the merge of pull request #7.
const COMMITS: &[&str] = &[
    "77602aa70a84426d40a5b92652ad302447908d95",
    "6b32f8bbaf601a0d49ad45909a312818261aba98",
];

#[test]
fn the_reconstructed_events_of_fixed_commits_match_the_snapshot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let Ok(repository) = GitRepository::open(&root) else {
        eprintln!("skipped: {} is not a Git repository", root.display());
        return;
    };
    let objects = GitObjectStore::new(repository);
    let commits: Vec<GitOid> = COMMITS
        .iter()
        .map(|hex| GitOid::from_hex(hex).expect("valid"))
        .collect();
    let parser = TreeSitterParser::new();
    let cache = support::cache();
    let mut index = IdentityIndex::open(&objects, &parser, &cache).expect("index opens");
    if let Err(error) = index.index_history(&commits) {
        // A shallow clone (the default checkout in CI) does not have the history.
        eprintln!("skipped: the history is not available ({error})");
        return;
    }
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&objects, &refs, &mut wal).expect("opens");
    let mut text = String::new();
    for commit in &commits {
        let events = log.events_of_commit(commit, &index).expect("events");
        let events = events.events();
        let mut digest = blake3::Hasher::new();
        for event in &events {
            digest.update(&event_id(event).0);
        }
        text.push_str(&format!(
            "{commit}: {} events, generation {}, digest {}\n",
            events.len(),
            events.first().map_or(0, |event| event.clock.generation),
            digest.finalize().to_hex()
        ));
        for event in events.iter().take(3) {
            text.push_str(&format!("  {} {:?}\n", event_id(event), event.clock));
        }
    }
    insta::assert_snapshot!(text);
}
