//! Detached-tape search on a deep history (IMP-006, SC-001 and SC-002 of
//! `specs/008-bounded-detached-walk`): a long history without Gedo data, then 200 commits with
//! published tapes and one tape detached from a side branch. The first call runs on an empty
//! cache; the others, each through a new event log as separate commands would, read the
//! generations from it. `GEDO_BENCH_DEPTH` sets the depth of the history without Gedo data
//! (default 100,000).
//!
//! The scratch repository is created with `git init` and `git fast-import`, which only this
//! developer tool runs.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_app::EventLog;
use gedo_core::StoreError;
use gedo_core::event::{Labels, SemanticOp};
use gedo_core::identity::{AstNodeId, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, NewCommit, ObjectStore, SymbolCache};
use gedo_core::syntax::StructuralHash;
use gedo_crdt::RecordingContext;
use gedo_store::{FileWal, GitObjectStore, GitRefStore, GitRepository, RedbCache};

/// Commits with a published tape, on top of the deep history.
const PUBLISHED: usize = 200;

struct Empty;

impl CacheRebuildSource for Empty {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
}

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

fn detached_walk() {
    let depth: usize = std::env::var("GEDO_BENCH_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000);
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
    let mut import = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(directory.path())
        .stdin(Stdio::piped())
        .spawn()
        .expect("fast-import runs");
    {
        let stdin = import.stdin.as_mut().expect("stdin");
        let mut script = String::new();
        for n in 0..depth {
            script.push_str(&format!(
                "commit refs/heads/main\nmark :{}\ncommitter Bench <bench@gedo.invalid> {} +0000\ndata 2\nc\n{}M 644 inline f\ndata {}\n{n}\n",
                n + 1,
                1_000_000 + n,
                if n == 0 { String::new() } else { format!("from :{n}\n") },
                n.to_string().len() + 1
            ));
        }
        stdin.write_all(script.as_bytes()).expect("written");
    }
    assert!(import.wait().expect("waits").success());
    let head = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "main"])
            .current_dir(directory.path())
            .output()
            .expect("rev-parse")
            .stdout,
    )
    .expect("utf-8");
    let repository = GitRepository::open_isolated(directory.path()).expect("opens");
    let gedo_dir = repository.gedo_dir().expect("gedo directory");
    std::fs::create_dir_all(&gedo_dir).expect("created");
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository);
    let mut tip = GitOid::from_hex(head.trim()).expect("oid");
    let tree = objects.read_commit(&tip).expect("commit").tree;
    let person = RecordingContext::person("bench@gedo.invalid", Labels::new()).expect("valid");
    let mut wal = FileWal::open(&gedo_dir).expect("log opens");
    let mut side_parent = tip;
    {
        let mut log = EventLog::open(&objects, &refs, &mut wal).expect("opens");
        for n in 0..PUBLISHED {
            let parent = tip;
            tip = objects
                .write_commit(&NewCommit {
                    tree,
                    parents: vec![parent],
                    message: format!("p{n}"),
                })
                .expect("commit");
            log.record(&person, Some(parent), &[op(n as u32)])
                .expect("recorded");
            log.publish(tip).expect("published");
            if n == PUBLISHED / 2 {
                side_parent = tip;
            }
        }
        let side = objects
            .write_commit(&NewCommit {
                tree,
                parents: vec![side_parent],
                message: "side".into(),
            })
            .expect("commit");
        log.record(&person, Some(side_parent), &[op(1_000_000)])
            .expect("recorded");
        log.publish(side).expect("published");
    }
    let cache_dir = tempfile::tempdir().expect("temporary directory");
    let cache = RedbCache::open(cache_dir.path(), &Empty).expect("cache opens");
    let mut first = Vec::new();
    let mut samples = Vec::new();
    for run in 0..13 {
        if run % 13 == 0 {
            cache.clear().expect("cleared");
        }
        let mut log = EventLog::open(&objects, &refs, &mut wal)
            .expect("opens")
            .with_cache(&cache);
        let started = Instant::now();
        let detached = log.detached(&[tip], 2_000_000_000, 90).expect("detached");
        let elapsed = started.elapsed();
        assert_eq!(detached.len(), 1);
        if run % 13 == 0 {
            first.push(elapsed);
        } else {
            samples.push(elapsed);
        }
    }
    println!(
        "detached tapes, {depth} commits without Gedo data, {PUBLISHED} with, empty cache: {:?}",
        first[0]
    );
    report(
        &format!("detached tapes, {depth} commits without Gedo data, {PUBLISHED} with, generations cached"),
        samples,
    );
}

fn bench(criterion: &mut Criterion) {
    detached_walk();
    criterion.bench_function("noop", |bencher| bencher.iter(|| 1));
}

criterion_group!(benches, bench);
criterion_main!(benches);
