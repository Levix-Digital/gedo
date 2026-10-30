//! Re-attachment on the Git store adapter (SC-004, SC-005 of `specs/005-history-reattachment`):
//! re-attaching one fetched squash commit with 10,000 stored events of which 1,000 are detached
//! (p95 under 100 ms once indexed), re-attaching a rebased commit through the fast path, and filling
//! an empty event cache with 10,000 events (under 2 s) followed by the first filter (under 500 ms).
//! `GEDO_BENCH_SAMPLES=1` prints every run in order.
//!
//! The scratch repository is created with `git init`, which only this developer tool runs.

use std::collections::BTreeMap;
use std::process::Command;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_app::{CommitEvents, EventLog, IdentityIndex};
use gedo_core::StoreError;
use gedo_core::event::{LabelKey, LabelValue, Labels, SemanticOp};
use gedo_core::identity::{AstNodeId, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_core::syntax::StructuralHash;
use gedo_crdt::{EventFilter, RecordingContext};
use gedo_parse::TreeSitterParser;
use gedo_store::{FileWal, GitObjectStore, GitRefStore, GitRepository, RedbCache};

/// Branches published and then rewritten away; each holds one tape of [`EVENTS_PER_TAPE`] events.
const BRANCHES: usize = 50;
const EVENTS_PER_TAPE: u32 = 20;
/// Functions in the scratch crate.
const FUNCTIONS: usize = 200;

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

struct Scratch {
    _directory: tempfile::TempDir,
    objects: GitObjectStore,
    refs: GitRefStore,
    gedo_dir: std::path::PathBuf,
    cache: RedbCache,
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
    let cache = RedbCache::open(&gedo_dir, &Empty).expect("cache opens");
    Scratch {
        _directory: directory,
        objects,
        refs,
        gedo_dir,
        cache,
    }
}

/// The crate's source with function `i` returning `values[i]`, and an `extra` file when given.
fn commit(scratch: &Scratch, parents: &[GitOid], values: &BTreeMap<usize, u32>, extra: bool) -> GitOid {
    let mut lib = String::new();
    for i in 0..FUNCTIONS {
        lib.push_str(&format!(
            "pub fn f{i}() -> u32 {{\n    {}\n}}\n\n",
            values.get(&i).copied().unwrap_or(0)
        ));
    }
    let mut src = vec![TreeEntry {
        name: "lib.rs".into(),
        mode: EntryMode::Blob,
        oid: scratch.objects.write_blob(lib.as_bytes()).expect("blob"),
    }];
    if extra {
        src.push(TreeEntry {
            name: "other.rs".into(),
            mode: EntryMode::Blob,
            oid: scratch
                .objects
                .write_blob(b"pub fn other() -> u32 {\n    2\n}\n")
                .expect("blob"),
        });
    }
    let src = scratch.objects.write_tree(&src).expect("tree");
    let tree = scratch
        .objects
        .write_tree(&[TreeEntry {
            name: "src".into(),
            mode: EntryMode::Tree,
            oid: src,
        }])
        .expect("tree");
    scratch
        .objects
        .write_commit(&NewCommit {
            tree,
            parents: parents.to_vec(),
            message: "bench".into(),
        })
        .expect("commit")
}

fn person() -> RecordingContext {
    RecordingContext::person("bench@gedo.invalid", Labels::new()).expect("valid")
}

/// A live edit of `id` that a later save overwrote.
fn intermediate(id: AstNodeId, n: u32) -> (SemanticOp, Option<StructuralHash>) {
    let mut after = [0u8; 32];
    after[..4].copy_from_slice(&n.to_le_bytes());
    after[31] = 1;
    (
        SemanticOp::Delta(SemanticDelta::BodyModified {
            id,
            before: StructuralHash([0; 32]),
            after: StructuralHash(after),
        }),
        Some(StructuralHash(after)),
    )
}

/// Publishes the changes of `commit` as one live tape of `EVENTS_PER_TAPE` events: earlier saves
/// of the same entity, then the commit's own change.
fn publish_branch(scratch: &Scratch, parser: &TreeSitterParser, commit: GitOid, parent: GitOid, seed: u32) {
    let index = IdentityIndex::open(&scratch.objects, parser, &scratch.cache).expect("opens");
    let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
    let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal)
        .expect("opens")
        .with_cache(&scratch.cache);
    let CommitEvents::Reconstructed(events) = log.events_of_commit(&commit, &index).expect("events") else {
        panic!("already published");
    };
    let target = events[0].target;
    let mut ops: Vec<_> = (0..EVENTS_PER_TAPE - 1)
        .map(|n| intermediate(target, seed * EVENTS_PER_TAPE + n))
        .collect();
    ops.extend(
        events
            .iter()
            .map(|event| (event.operation.clone(), event.result_hash)),
    );
    log.record(&person(), Some(parent), &ops).expect("recorded");
    log.publish(commit).expect("published");
}

fn report_reattachment() {
    let scratch = scratch();
    let parser = TreeSitterParser::new();
    let root = commit(&scratch, &[], &BTreeMap::new(), false);
    let main = commit(&scratch, &[root], &BTreeMap::new(), true);

    // 9,000 events that stay attached: published for the root, which stays in the history.
    {
        let mut index = IdentityIndex::open(&scratch.objects, &parser, &scratch.cache).expect("opens");
        index.index_history(&[main]).expect("indexed");
        let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
        let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal)
            .expect("opens")
            .with_cache(&scratch.cache);
        for tape in 0..9u32 {
            let ops: Vec<_> = (0..1_000)
                .map(|n| intermediate(AstNodeId([9; 16]), 1_000_000 + tape * 1_000 + n))
                .collect();
            log.record(&person(), None, &ops).expect("recorded");
            log.publish(root).expect("published");
        }
    }

    // 50 branches of one commit each, published and then rewritten away: 1,000 detached events.
    let mut branches = Vec::new();
    for b in 0..BRANCHES {
        let values = BTreeMap::from([(b, 1_000 + b as u32)]);
        let work = commit(&scratch, &[root], &values, false);
        let mut index = IdentityIndex::open(&scratch.objects, &parser, &scratch.cache).expect("opens");
        index.index_history(&[work]).expect("indexed");
        publish_branch(&scratch, &parser, work, root, b as u32);
        // The commit a server squash (or a rebase onto main) makes of the branch.
        let rewritten = commit(&scratch, &[main], &values, true);
        branches.push((work, rewritten));
    }
    let rewritten: Vec<GitOid> = branches.iter().map(|(_, new)| *new).collect();
    let mut index = IdentityIndex::open(&scratch.objects, &parser, &scratch.cache).expect("opens");
    index.index_history(&rewritten).expect("indexed");

    let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
    let mut squashes = Vec::new();
    let mut pairs = Vec::new();
    for (position, (work, new)) in branches.iter().enumerate() {
        let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal)
            .expect("opens")
            .with_cache(&scratch.cache);
        let tips = [*new];
        let started = Instant::now();
        let report = if position % 2 == 0 {
            log.reattach_new(&[*new], &tips, &index).expect("re-attached")
        } else {
            log.reattach_rewritten(&[(*work, *new)], &index)
                .expect("re-attached")
        };
        let elapsed = started.elapsed();
        assert_eq!(report.attached(), 1, "{report:?}");
        if position % 2 == 0 {
            squashes.push(elapsed);
        } else {
            assert!(report.commits[0].fast_path);
            pairs.push(elapsed);
        }
    }
    report(
        "re-attach one fetched squash (10,000 events, 1,000 detached)",
        squashes,
    );
    report("re-attach one rebased commit (fast path)", pairs);
}

fn report_cache_fill() {
    let scratch = scratch();
    let root = commit(&scratch, &[], &BTreeMap::new(), false);
    {
        let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
        let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal).expect("opens");
        for tape in 0..500u32 {
            let context = RecordingContext::from_texts(
                "bench@gedo.invalid",
                None,
                Some(&format!("dc.milestone=m{}", tape % 10)),
            )
            .expect("valid");
            let ops: Vec<_> = (0..20)
                .map(|n| intermediate(AstNodeId([7; 16]), tape * 20 + n))
                .collect();
            log.record(&context, Some(root), &ops).expect("recorded");
            log.publish(root).expect("published");
        }
    }
    let key = LabelKey::new("dc.milestone").expect("valid");
    let filter = EventFilter {
        labels: vec![(key.clone(), LabelValue::new(&key, "m3").expect("valid"))],
        ..EventFilter::default()
    };
    let mut fills = Vec::new();
    let mut filters = Vec::new();
    let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
    for _ in 0..5 {
        let empty = tempfile::tempdir().expect("temporary directory");
        let cache = RedbCache::open(empty.path(), &Empty).expect("cache opens");
        let log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal)
            .expect("opens")
            .with_cache(&cache);
        let started = Instant::now();
        let fill = log.fill_cache().expect("filled");
        fills.push(started.elapsed());
        assert_eq!(fill.cached, 10_000);
        let started = Instant::now();
        assert_eq!(log.filter(&filter).expect("filter").len(), 1_000);
        filters.push(started.elapsed());
    }
    report("fill an empty cache with 10,000 events", fills);
    report("first filter by one label after filling", filters);
}

fn reattachment(criterion: &mut Criterion) {
    report_reattachment();
    report_cache_fill();
    // Criterion needs one measured function; the numbers that matter are printed above.
    criterion.bench_function("semantic signature of one change", |bencher| {
        let deltas = vec![SemanticDelta::BodyModified {
            id: AstNodeId([1; 16]),
            before: StructuralHash([2; 32]),
            after: StructuralHash([3; 32]),
        }];
        bencher.iter(|| gedo_crdt::semantic_signature(&deltas, &[], |_| Some(StructuralHash([3; 32]))));
    });
}

criterion_group!(benches, reattachment);
criterion_main!(benches);
