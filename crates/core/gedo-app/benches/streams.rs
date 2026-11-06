//! Virtual streams on a real repository (SC-003 of `specs/009-virtual-streams`): creating,
//! switching and reading a stream, listing 1,000 streams, and publishing a stream of 1,000 events
//! against `EventLog::publish` of the same events. Each operation runs through new use cases, as
//! separate commands would.
//!
//! The scratch repository is created with `git init`, which only this developer tool runs.

use std::process::Command;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_app::{EventLog, StreamPublish, Streams};
use gedo_core::event::{Labels, SemanticOp};
use gedo_core::identity::{AstNodeId, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::ports::{EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_core::stream::StreamName;
use gedo_core::syntax::StructuralHash;
use gedo_crdt::{RecordingContext, StreamFilter};
use gedo_store::{FileStreamStore, FileWal, GitObjectStore, GitRefStore, GitRepository};

const LISTED: usize = 1_000;
const EVENTS: u32 = 1_000;
const RUNS: usize = 20;

fn op(n: u32) -> (SemanticOp, Option<StructuralHash>) {
    let mut after = [0u8; 32];
    after[..4].copy_from_slice(&n.to_le_bytes());
    (
        SemanticOp::Delta(SemanticDelta::BodyModified {
            id: AstNodeId([3; 16]),
            before: StructuralHash([0; 32]),
            after: StructuralHash(after),
        }),
        Some(StructuralHash(after)),
    )
}

fn report(name: &str, mut samples: Vec<Duration>) {
    samples.sort();
    let at = |fraction: f64| samples[((samples.len() as f64 - 1.0) * fraction).round() as usize];
    println!(
        "{name} over {} runs: p50 {:?}, p95 {:?}",
        samples.len(),
        at(0.50),
        at(0.95)
    );
}

fn time(mut operation: impl FnMut(usize)) -> Vec<Duration> {
    (0..RUNS)
        .map(|run| {
            let started = Instant::now();
            operation(run);
            started.elapsed()
        })
        .collect()
}

fn tree(objects: &GitObjectStore, text: &str) -> GitOid {
    let blob = objects.write_blob(text.as_bytes()).expect("blob");
    objects
        .write_tree(&[TreeEntry {
            name: "lib.rs".into(),
            mode: EntryMode::Blob,
            oid: blob,
        }])
        .expect("tree")
}

fn streams_bench() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(directory.path())
            .status()
            .expect("git runs")
    };
    assert!(git(&["init", "--quiet"]).success());
    assert!(git(&["config", "user.name", "Gedo Bench"]).success());
    assert!(git(&["config", "user.email", "bench@gedo.invalid"]).success());
    let repository = GitRepository::open_isolated(directory.path()).expect("opens");
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository.clone());
    let store = FileStreamStore::open(&repository).expect("opens");
    let base_tree = tree(&objects, "pub fn base() {}\n");
    let base = objects
        .write_commit(&NewCommit {
            tree: base_tree,
            parents: vec![],
            message: "base".into(),
        })
        .expect("commit");
    let name = |n: usize| StreamName::new(&format!("bench/s{n}")).expect("name");

    let create = time(|run| {
        Streams::new(&objects, &refs, &store)
            .create(
                &name(run),
                None,
                Labels::from_pairs(&[("dc.plan", "bench")]).expect("labels"),
                Some(base),
            )
            .expect("created");
    });
    report("stream create", create);

    let works = [
        tree(&objects, "pub fn a() {}\n"),
        tree(&objects, "pub fn b() {}\n"),
    ];
    let switch = time(|run| {
        Streams::new(&objects, &refs, &store)
            .switch(Some(&name(run % 2)), works[run % 2])
            .expect("switched");
    });
    report("stream switch", switch);

    let get = time(|run| {
        Streams::new(&objects, &refs, &store)
            .get(&name(run))
            .expect("read");
    });
    report("stream get", get);

    for n in RUNS..LISTED {
        Streams::new(&objects, &refs, &store)
            .create(&name(n), None, Labels::new(), Some(base))
            .expect("created");
    }
    let list = time(|_| {
        let listed = Streams::new(&objects, &refs, &store)
            .list(&StreamFilter::from_texts(Some("dc.plan=bench"), None).expect("filter"))
            .expect("listed");
        assert_eq!(listed.len(), RUNS);
    });
    report(&format!("stream list of {LISTED}"), list);

    let ops: Vec<_> = (0..EVENTS).map(op).collect();
    let mut stream_publish = Vec::new();
    let mut plain_publish = Vec::new();
    let gedo_dir = repository.gedo_dir().expect("gedo directory");
    let person = RecordingContext::person("bench@gedo.invalid", Labels::new()).expect("valid");
    for run in 0..RUNS {
        let streams = Streams::new(&objects, &refs, &store);
        let published = StreamName::new(&format!("publish/s{run}")).expect("name");
        streams
            .create(&published, None, Labels::new(), Some(base))
            .expect("created");
        streams
            .record(&published, "bench@gedo.invalid", &ops)
            .expect("recorded");
        let work = tree(&objects, &format!("pub fn w{run}() {{}}\n"));
        let started = Instant::now();
        let outcome = streams
            .publish(&published, Some(work), "bench\n")
            .expect("published");
        stream_publish.push(started.elapsed());
        assert!(matches!(outcome, StreamPublish::Published { events, .. } if events == EVENTS as usize));

        let mut wal = FileWal::open(&gedo_dir).expect("log opens");
        let mut log = EventLog::open(&objects, &refs, &mut wal).expect("opens");
        log.record(&person, Some(base), &ops).expect("recorded");
        let commit = objects
            .write_commit(&NewCommit {
                tree: work,
                parents: vec![base],
                message: format!("plain {run}"),
            })
            .expect("commit");
        let started = Instant::now();
        log.publish(commit).expect("published");
        plain_publish.push(started.elapsed());
    }
    report(&format!("stream publish of {EVENTS} events"), stream_publish);
    report(&format!("EventLog::publish of {EVENTS} events"), plain_publish);
}

fn bench(criterion: &mut Criterion) {
    streams_bench();
    criterion.bench_function("noop", |bencher| bencher.iter(|| 1));
}

criterion_group!(benches, bench);
criterion_main!(benches);
