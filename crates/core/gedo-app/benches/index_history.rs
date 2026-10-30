//! Indexing history (SC-005): the whole history once, then single commits on top (p95 under
//! 100 ms).
//!
//! The generated history edits the formatted fixtures of feature 002 (real workspace files) on
//! the in-memory store: each commit renames a function, adds statements or moves a file. When
//! `GEDO_BENCH_REPO` names a Git repository (CI clones ripgrep at a pinned tag), its history up to
//! `HEAD` is indexed as well; revisions are resolved with `git rev-parse` and `git rev-list`, which
//! only this developer tool runs. Results are printed; criterion measures the generated history.

use std::collections::BTreeMap;
use std::process::Command;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_app::IdentityIndex;
use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::memory::{MemoryCache, MemoryObjectStore};
use gedo_core::ports::{CacheEntry, CacheRebuildSource, EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_parse::TreeSitterParser;
use gedo_store::{GitObjectStore, GitRepository};

const COMMITS: usize = 60;
/// Commits of the real repository indexed one at a time after the rest of its history.
const SINGLE_COMMITS: usize = 20;

struct Empty;

impl CacheRebuildSource for Empty {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
}

fn cache() -> MemoryCache {
    MemoryCache::open(&Empty).expect("cache opens")
}

fn fixtures() -> BTreeMap<String, String> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/gedo-parse/tests/fixtures/rust/formatted/default/src");
    std::fs::read_dir(&directory)
        .expect("fixtures are readable")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| {
            let name = path.file_name().expect("name").to_string_lossy().into_owned();
            (
                format!("src/{name}"),
                std::fs::read_to_string(&path).expect("UTF-8"),
            )
        })
        .collect()
}

fn tree(objects: &MemoryObjectStore, files: &BTreeMap<String, String>) -> GitOid {
    let mut sources = Vec::new();
    for (path, text) in files {
        let name = path.trim_start_matches("src/");
        sources.push(TreeEntry {
            name: name.into(),
            mode: EntryMode::Blob,
            oid: objects.write_blob(text.as_bytes()).expect("blob"),
        });
    }
    let src = objects.write_tree(&sources).expect("tree");
    objects
        .write_tree(&[TreeEntry {
            name: "src".into(),
            mode: EntryMode::Tree,
            oid: src,
        }])
        .expect("tree")
}

/// A linear history of `COMMITS` commits; returns the store and the commits, oldest first.
fn generated() -> (MemoryObjectStore, Vec<GitOid>) {
    let objects = MemoryObjectStore::new();
    let mut files = fixtures();
    let mut commits = Vec::new();
    for step in 0..COMMITS {
        if step > 0 {
            let paths: Vec<String> = files.keys().cloned().collect();
            let path = &paths[step % paths.len()];
            let text = files[path].clone();
            let edited = match step % 3 {
                0 => text.replacen("fn ", &format!("fn r{step}_"), 1),
                1 => text.replacen("{\n", &format!("{{\n    let gedo_bench_{step} = {step};\n"), 1),
                _ => {
                    files.remove(path);
                    files.insert(path.replace(".rs", &format!("_{step}.rs")), text.clone());
                    text
                }
            };
            if step % 3 != 2 {
                files.insert(path.clone(), edited);
            }
        }
        let parents = commits.last().map(|parent| vec![*parent]).unwrap_or_default();
        let commit = objects
            .write_commit(&NewCommit {
                tree: tree(&objects, &files),
                parents,
                message: format!("step {step}"),
            })
            .expect("commit");
        commits.push(commit);
    }
    (objects, commits)
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index]
}

/// Indexes `commits` one call each, oldest first, and prints the distribution.
fn report_singles<O: ObjectStore>(label: &str, objects: &O, commits: &[GitOid], cache: &mut MemoryCache) {
    let parser = TreeSitterParser::new();
    let mut samples: Vec<Duration> = commits
        .iter()
        .map(|commit| {
            let mut index = IdentityIndex::open(objects, &parser, cache).expect("opens");
            let started = Instant::now();
            index.index_history(&[*commit]).expect("indexed");
            started.elapsed()
        })
        .collect();
    samples.sort();
    println!(
        "{label}: one commit over {} commits: p50 {:?}, p95 {:?}, max {:?}",
        samples.len(),
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        samples.last().copied().unwrap_or_default(),
    );
}

fn git(repository: &str, arguments: &[&str]) -> Vec<GitOid> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {arguments:?} failed");
    String::from_utf8(output.stdout)
        .expect("UTF-8")
        .lines()
        .map(|line| GitOid::from_hex(line.trim()).expect("commit id"))
        .collect()
}

/// The history of the repository named by `GEDO_BENCH_REPO`, when set.
fn real_repository() {
    let Ok(repository) = std::env::var("GEDO_BENCH_REPO") else {
        println!("index_history: GEDO_BENCH_REPO is not set, the real repository is skipped");
        return;
    };
    let mut recent = git(
        &repository,
        &[
            "rev-list",
            "--first-parent",
            "-n",
            &(SINGLE_COMMITS + 1).to_string(),
            "HEAD",
        ],
    );
    recent.reverse();
    let objects = GitObjectStore::new(GitRepository::open(&repository).expect("repository opens"));
    let parser = TreeSitterParser::new();
    let mut cache = cache();
    let started = Instant::now();
    let report = IdentityIndex::open(&objects, &parser, &cache)
        .expect("opens")
        .index_history(&recent[..1])
        .expect("indexed");
    println!(
        "index_history {repository}: {} commits, {} files parsed in {:.2?}",
        report.commits_indexed,
        report.files_parsed,
        started.elapsed()
    );
    report_singles("index_history real", &objects, &recent[1..], &mut cache);
}

fn bench(criterion: &mut Criterion) {
    let (objects, commits) = generated();
    let parser = TreeSitterParser::new();
    let tip = *commits.last().expect("commits");
    let mut group = criterion.benchmark_group("index_history");
    group.sample_size(10);
    group.bench_function("generated_60_commits", |bencher| {
        bencher.iter(|| {
            let cache = cache();
            IdentityIndex::open(&objects, &parser, &cache)
                .expect("opens")
                .index_history(&[tip])
                .expect("indexed")
        })
    });
    group.finish();
    report_singles("index_history generated", &objects, &commits, &mut cache());
    real_repository();
}

criterion_group!(benches, bench);
criterion_main!(benches);
