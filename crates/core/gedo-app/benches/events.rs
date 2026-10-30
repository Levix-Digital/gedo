//! The event log on the Git store adapter (SC-004 to SC-006): recording one event (p95 under
//! 1 ms), publishing a tape of 1,000 events (under 1 s), filtering 10,000 events by a label (under
//! 500 ms) and, when `GEDO_BENCH_REPO` names a Git repository, reconstructing the events of its last
//! commits (p95 under 100 ms once indexed). `GEDO_BENCH_LARGE=1` also publishes into a chain of
//! 100,000 events, and `GEDO_BENCH_SAMPLES=1` prints every run in order.
//!
//! Criterion measures the mean of one record; the percentiles are measured separately and printed.
//! The scratch repository is created with `git init`, which only this developer tool runs.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_app::{EventLog, IdentityIndex};
use gedo_core::StoreError;
use gedo_core::event::{LabelKey, LabelValue, Labels, SemanticOp};
use gedo_core::identity::{AstNodeId, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::memory::MemoryCache;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, NewCommit, ObjectStore};
use gedo_core::syntax::StructuralHash;
use gedo_crdt::{EventFilter, RecordingContext};
use gedo_parse::TreeSitterParser;
use gedo_store::{FileWal, GitObjectStore, GitRefStore, GitRepository, RedbCache};

struct Empty;

impl CacheRebuildSource for Empty {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
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

/// A scratch repository with one commit.
struct Scratch {
    _directory: tempfile::TempDir,
    objects: GitObjectStore,
    refs: GitRefStore,
    gedo_dir: std::path::PathBuf,
    cache: RedbCache,
    commit: GitOid,
}

fn scratch() -> Scratch {
    let directory = tempfile::tempdir().expect("temporary directory");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(directory.path())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "Gedo Bench"]);
    git(&["config", "user.email", "bench@gedo.invalid"]);
    let repository = GitRepository::open_isolated(directory.path()).expect("opens");
    let gedo_dir = repository.gedo_dir().expect("gedo directory");
    std::fs::create_dir_all(&gedo_dir).expect("created");
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository);
    let tree = objects.write_tree(&[]).expect("tree");
    let commit = objects
        .write_commit(&NewCommit {
            tree,
            parents: vec![],
            message: "bench".into(),
        })
        .expect("commit");
    let cache = RedbCache::open(&gedo_dir, &Empty).expect("cache opens");
    Scratch {
        _directory: directory,
        objects,
        refs,
        gedo_dir,
        cache,
        commit,
    }
}

fn op(n: u32) -> (SemanticOp, Option<StructuralHash>) {
    let mut after = [0u8; 32];
    after[..4].copy_from_slice(&n.to_le_bytes());
    (
        SemanticOp::Delta(SemanticDelta::BodyModified {
            id: AstNodeId([7; 16]),
            before: StructuralHash([0; 32]),
            after: StructuralHash(after),
        }),
        Some(StructuralHash(after)),
    )
}

fn context(milestone: usize) -> RecordingContext {
    RecordingContext::from_texts(
        "bench@gedo.invalid",
        Some("worker-1"),
        Some(&format!("dc.plan=bench,dc.milestone=m{milestone}")),
    )
    .expect("valid")
}

/// Publishes `tapes` tapes of `events` events each; returns the time of each publish.
fn publish_tapes(scratch: &Scratch, tapes: usize, events: u32, offset: u32) -> Vec<Duration> {
    let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
    let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal)
        .expect("opens")
        .with_cache(&scratch.cache);
    let mut times = Vec::with_capacity(tapes);
    for tape in 0..tapes {
        let start = offset + tape as u32 * events;
        let ops: Vec<_> = (start..start + events).map(op).collect();
        log.record(&context(tape % 10), Some(scratch.commit), &ops)
            .expect("recorded");
        let started = Instant::now();
        log.publish(scratch.commit).expect("published");
        times.push(started.elapsed());
    }
    times
}

/// Filters with the cache the publishes filled, and with an empty cache that the first filter
/// fills (a clone that never cached the events, or a cache that was rebuilt).
fn report_filters(name: &str, scratch: &Scratch, filter: &EventFilter) {
    let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
    let run = |cache: &RedbCache, wal: &mut FileWal| {
        let log = EventLog::open(&scratch.objects, &scratch.refs, wal)
            .expect("opens")
            .with_cache(cache);
        let started = Instant::now();
        let found = log.filter(filter).expect("filter");
        assert_eq!(found.len(), 1_000);
        started.elapsed()
    };
    let samples = (0..5).map(|_| run(&scratch.cache, &mut wal)).collect();
    report(name, samples);
    let cold = (0..5)
        .map(|_| {
            let empty = tempfile::tempdir().expect("temporary directory");
            let cache = RedbCache::open(empty.path(), &Empty).expect("cache opens");
            run(&cache, &mut wal)
        })
        .collect();
    report(&format!("{name}, empty cache"), cold);
}

fn report_log() {
    let scratch = scratch();
    let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
    let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal).expect("opens");
    let samples = (0..2_000)
        .map(|n| {
            let started = Instant::now();
            log.record(&context(0), Some(scratch.commit), &[op(n)])
                .expect("recorded");
            started.elapsed()
        })
        .collect();
    report("record one event", samples);
    drop(log);
    drop(wal);

    let scratch = self::scratch();
    report(
        "publish a tape of 1,000 events",
        publish_tapes(&scratch, 10, 1_000, 0),
    );

    let key = LabelKey::new("dc.milestone").expect("valid");
    let filter = EventFilter {
        labels: vec![(key.clone(), LabelValue::new(&key, "m3").expect("valid"))],
        ..EventFilter::default()
    };
    report_filters("filter 10,000 events by one label", &scratch, &filter);

    // Commits usually publish a few events each, and tapes under the pack threshold of the store
    // stay loose objects: the same 10,000 events in 500 tapes of 20.
    let small = self::scratch();
    publish_tapes(&small, 500, 20, 0);
    report_filters(
        "filter 10,000 events published in tapes of 20 by one label",
        &small,
        &filter,
    );

    if std::env::var_os("GEDO_BENCH_LARGE").is_some() {
        let large = self::scratch();
        publish_tapes(&large, 100, 1_000, 0);
        report(
            "publish a tape of 1,000 events into a chain of 100,000",
            publish_tapes(&large, 5, 1_000, 100_000),
        );
    }
}

fn report_reconstruction() {
    let Some(path) = std::env::var_os("GEDO_BENCH_REPO") else {
        println!("reconstruction: set GEDO_BENCH_REPO to a Git repository to measure it");
        return;
    };
    let path = Path::new(&path);
    let rev_list = Command::new("git")
        .args(["rev-list", "--max-count=20", "HEAD"])
        .current_dir(path)
        .output()
        .expect("git runs");
    let commits: Vec<GitOid> = String::from_utf8_lossy(&rev_list.stdout)
        .lines()
        .map(|line| GitOid::from_hex(line).expect("commit id"))
        .collect();
    let repository = GitRepository::open(path).expect("opens");
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository);
    let parser = TreeSitterParser::new();
    let cache = MemoryCache::open(&Empty).expect("cache opens");
    let mut index = IdentityIndex::open(&objects, &parser, &cache).expect("index opens");
    index.index_history(&commits[..1]).expect("indexed");
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut wal = FileWal::open(directory.path()).expect("log opens");
    let mut log = EventLog::open(&objects, &refs, &mut wal).expect("opens");
    log.generation(&commits[0]).expect("generation");
    let samples = commits
        .iter()
        .map(|commit| {
            let started = Instant::now();
            log.events_of_commit(commit, &index).expect("events");
            started.elapsed()
        })
        .collect();
    report("reconstruct the events of one commit", samples);
}

fn record_event(criterion: &mut Criterion) {
    report_log();
    report_reconstruction();
    let scratch = scratch();
    let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
    let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal).expect("opens");
    let labels = Labels::new();
    let person = RecordingContext::person("bench@gedo.invalid", labels).expect("valid");
    let mut n = 0u32;
    criterion.bench_function("record_event", |bencher| {
        bencher.iter(|| {
            n = n.wrapping_add(1);
            log.record(&person, Some(scratch.commit), &[op(n)])
                .expect("recorded")
        })
    });
}

criterion_group!(benches, record_event);
criterion_main!(benches);
