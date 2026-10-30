//! Convergence of commits on the Git store adapter (SC-005 of `specs/006-tree-crdt`): two
//! branches of 100 commits each converged from their tips, with their identities indexed, p95 under
//! 500 ms. `GEDO_BENCH_SAMPLES=1` prints every run in order.
//!
//! The scratch repository is created with `git init`, which only this developer tool runs.

use std::collections::BTreeMap;
use std::process::Command;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_app::{EventLog, IdentityIndex, Side};
use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_parse::TreeSitterParser;
use gedo_store::{FileWal, GitObjectStore, GitRefStore, GitRepository, RedbCache};

/// Functions in the scratch crate.
const FUNCTIONS: usize = 200;
/// Commits on each branch.
const COMMITS: usize = 100;
const RUNS: usize = 25;

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

/// The crate's source with function `i` returning `values[i]`.
fn commit(scratch: &Scratch, parents: &[GitOid], values: &BTreeMap<usize, u32>) -> GitOid {
    let mut lib = String::new();
    for i in 0..FUNCTIONS {
        lib.push_str(&format!(
            "pub fn f{i}() -> u32 {{\n    {}\n}}\n\n",
            values.get(&i).copied().unwrap_or(0)
        ));
    }
    let src = scratch
        .objects
        .write_tree(&[TreeEntry {
            name: "lib.rs".into(),
            mode: EntryMode::Blob,
            oid: scratch.objects.write_blob(lib.as_bytes()).expect("blob"),
        }])
        .expect("tree");
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

/// A branch of `COMMITS` commits from `base`, each changing one function of `first..`.
fn branch(scratch: &Scratch, base: GitOid, first: usize) -> GitOid {
    let mut values = BTreeMap::new();
    let mut tip = base;
    for step in 0..COMMITS {
        values.insert(first + step % (FUNCTIONS / 2), step as u32 + 1);
        tip = commit(scratch, &[tip], &values);
    }
    tip
}

fn converging(criterion: &mut Criterion) {
    let scratch = scratch();
    let parser = TreeSitterParser::new();
    let base = commit(&scratch, &[], &BTreeMap::new());
    let ours = branch(&scratch, base, 0);
    let theirs = branch(&scratch, base, FUNCTIONS / 2);
    let mut index = IdentityIndex::open(&scratch.objects, &parser, &scratch.cache).expect("opens");
    index.index_history(&[ours, theirs]).expect("indexed");
    let sides = [Side::commit(ours), Side::commit(theirs)];

    let samples = (0..RUNS)
        .map(|_| {
            let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
            let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal)
                .expect("opens")
                .with_cache(&scratch.cache);
            let started = Instant::now();
            let report = log.converge(&base, &sides, &index).expect("converges");
            let elapsed = started.elapsed();
            assert!(
                report.convergence.is_clean(),
                "{:?}",
                report.convergence.conflicts
            );
            assert_eq!(report.commits.iter().map(Vec::len).sum::<usize>(), 2 * COMMITS);
            elapsed
        })
        .collect();
    report("converge two branches of 100 commits", samples);

    criterion.bench_function("converge two branches of 100 commits", |bencher| {
        bencher.iter(|| {
            let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
            let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal).expect("opens");
            log.converge(&base, &sides, &index).expect("converges")
        });
    });
}

criterion_group!(benches, converging);
criterion_main!(benches);
