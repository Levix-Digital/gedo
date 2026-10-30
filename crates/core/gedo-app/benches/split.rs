//! Split publish on the Git store adapter (SC-003 of `specs/007-semantic-lanes`): a rename whose
//! callers span 100 files, with a feature edit in each file, split-published with its 1,000 events,
//! p95 under 1 s. `GEDO_BENCH_SAMPLES=1` prints every run in order.
//!
//! The scratch repository is created with `git init`, which only this developer tool runs.

use std::process::Command;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_app::{CommitEvents, EventLog, IdentityIndex, SplitOutcome, SplitRequest};
use gedo_core::StoreError;
use gedo_core::event::Labels;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_crdt::RecordingContext;
use gedo_parse::TreeSitterParser;
use gedo_store::{FileWal, GitObjectStore, GitRefStore, GitRepository, RedbCache};

/// Modules of the scratch crate, one file each.
const FILES: usize = 100;
/// Functions in each module, each calling the renamed helper.
const FUNCTIONS: usize = 10;
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

fn blob(scratch: &Scratch, text: &str) -> TreeEntry {
    TreeEntry {
        name: String::new(),
        mode: EntryMode::Blob,
        oid: scratch.objects.write_blob(text.as_bytes()).expect("blob"),
    }
}

/// The crate with the helper named `helper`, and `+ extra` in the first function of every module.
fn tree(scratch: &Scratch, helper: &str, extra: u32) -> GitOid {
    let mut lib = String::new();
    for module in 0..FILES {
        lib.push_str(&format!("pub mod m{module};\n"));
    }
    lib.push_str(&format!("\npub fn {helper}() -> u32 {{\n    1\n}}\n"));
    let mut entries = vec![TreeEntry {
        name: "lib.rs".into(),
        ..blob(scratch, &lib)
    }];
    for module in 0..FILES {
        let mut text = String::new();
        for function in 0..FUNCTIONS {
            let plus = if function == 0 { extra } else { function as u32 };
            text.push_str(&format!(
                "pub fn f{function}() -> u32 {{\n    crate::{helper}() + {plus}\n}}\n\n"
            ));
        }
        entries.push(TreeEntry {
            name: format!("m{module}.rs"),
            ..blob(scratch, &text)
        });
    }
    let src = scratch.objects.write_tree(&entries).expect("tree");
    scratch
        .objects
        .write_tree(&[TreeEntry {
            name: "src".into(),
            mode: EntryMode::Tree,
            oid: src,
        }])
        .expect("tree")
}

fn commit(scratch: &Scratch, tree: GitOid, parents: &[GitOid], message: &str) -> GitOid {
    scratch
        .objects
        .write_commit(&NewCommit {
            tree,
            parents: parents.to_vec(),
            message: message.into(),
        })
        .expect("commit")
}

fn splitting(criterion: &mut Criterion) {
    let scratch = scratch();
    let parser = TreeSitterParser::new();
    let base = commit(&scratch, tree(&scratch, "helper", 0), &[], "base");
    let final_tree = tree(&scratch, "assist", 100);
    let work = commit(&scratch, final_tree, &[base], "work");
    let mut index = IdentityIndex::open(&scratch.objects, &parser, &scratch.cache).expect("opens");
    index.index_history(&[work]).expect("indexed");
    // The events a developer working with Gedo would have recorded.
    let ops: Vec<_> = {
        let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
        let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal).expect("opens");
        let CommitEvents::Reconstructed(events) = log.events_of_commit(&work, &index).expect("events") else {
            panic!("published");
        };
        events
            .iter()
            .map(|event| (event.operation.clone(), event.result_hash))
            .collect()
    };
    assert!(ops.len() >= FILES * FUNCTIONS, "{} events", ops.len());
    let context = RecordingContext::person("bench@gedo.invalid", Labels::new()).expect("valid");
    let request = SplitRequest {
        base,
        tree: final_tree,
        message: "feat: bench".into(),
        refactor_message: None,
    };

    let run = |index: &mut IdentityIndex<'_, _, _, _>| {
        let mut wal = FileWal::open(&scratch.gedo_dir).expect("log opens");
        let mut log = EventLog::open(&scratch.objects, &scratch.refs, &mut wal)
            .expect("opens")
            .with_cache(&scratch.cache);
        log.record(&context, Some(base), &ops).expect("recorded");
        let started = Instant::now();
        let outcome = log.publish_split(&request, index, &parser).expect("published");
        let elapsed = started.elapsed();
        assert!(matches!(outcome, SplitOutcome::Split { .. }), "{outcome:?}");
        elapsed
    };
    let samples = (0..RUNS).map(|_| run(&mut index)).collect();
    report("split-publish 100 files and 1,000 events", samples);

    criterion.bench_function("split-publish 100 files and 1,000 events", |bencher| {
        bencher.iter(|| run(&mut index));
    });
}

criterion_group!(benches, splitting);
criterion_main!(benches);
