//! Times each storage step of a stream switch that saves new work, and of a stream publish, on a
//! scratch repository (BUG-012). Every run saves a new work tree, so every switch writes.
//!
//! `cargo run --release -p gedo-app --example stream_steps [runs]`
//!
//! The scratch repository is created with `git init`, which only this developer tool runs.

use std::collections::BTreeMap;
use std::process::Command;
use std::time::{Duration, Instant};

use gedo_core::ids::GitOid;
use gedo_core::ports::{EntryMode, NewCommit, ObjectStore, StreamStore, TreeEntry};
use gedo_core::stream::StreamName;
use gedo_store::{FileStreamStore, GitObjectStore, GitRepository};

struct Steps(BTreeMap<&'static str, Vec<Duration>>);

impl Steps {
    fn time<T>(&mut self, step: &'static str, operation: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let value = operation();
        self.0.entry(step).or_default().push(started.elapsed());
        value
    }

    fn report(self) {
        for (step, mut samples) in self.0 {
            samples.sort();
            let at = |fraction: f64| samples[((samples.len() as f64 - 1.0) * fraction).round() as usize];
            println!(
                "{step:<28} p50 {:>10.3?}  p95 {:>10.3?}  max {:>10.3?}",
                at(0.50),
                at(0.95),
                samples[samples.len() - 1]
            );
        }
    }
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

fn main() {
    let runs: usize = std::env::args()
        .nth(1)
        .map(|text| text.parse().expect("runs is a number"))
        .unwrap_or(50);
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
    let store = FileStreamStore::open(&repository).expect("opens");
    let base_tree = tree(&objects, "pub fn base() {}\n");
    let base = objects
        .write_commit(&NewCommit {
            tree: base_tree,
            parents: vec![],
            message: "base".into(),
        })
        .expect("commit");
    let name = StreamName::new("bench/steps").expect("name");
    gedo_app::Streams::new(
        &objects,
        &gedo_store::GitRefStore::new(repository.clone()),
        &store,
    )
    .create(&name, None, gedo_core::event::Labels::new(), Some(base))
    .expect("created");
    let record = store.read(&name).expect("read").expect("exists");
    let state = store.read_state().expect("state").unwrap_or_default();
    let works: Vec<GitOid> = (0..runs)
        .map(|run| tree(&objects, &format!("pub fn w{run}() {{}}\n")))
        .collect();

    let mut steps = Steps(BTreeMap::new());
    for work in works {
        let total = Instant::now();
        steps.time("1 read record", || store.read(&name).expect("read"));
        steps.time("2 read state", || store.read_state().expect("state"));
        let log = steps.time("3 open log", || store.open_log(&name).expect("log"));
        steps.time("4 read base commit", || objects.read_commit(&base).expect("base"));
        let commit = steps.time("5 write work commit", || {
            objects
                .write_commit(&NewCommit {
                    tree: work,
                    parents: vec![base],
                    message: gedo_app::streams::WORK_COMMIT_MESSAGE.into(),
                })
                .expect("commit")
        });
        steps.time("6 replace record", || {
            store.replace(&name, &record).expect("replace")
        });
        steps.time("7 pin reference", || store.pin(&name, Some(commit)).expect("pin"));
        drop(log);
        steps.time("8 write state", || store.write_state(&state).expect("state"));
        steps.0.entry("0 total").or_default().push(total.elapsed());
    }
    println!("switch that saves new work, {runs} runs:");
    steps.report();

    // The storage steps a stream publish adds to `EventLog::publish` (research R9).
    let mut steps = Steps(BTreeMap::new());
    for run in 0..runs {
        let total = Instant::now();
        steps.time("1 read record", || store.read(&name).expect("read"));
        let log = steps.time("2 open log", || store.open_log(&name).expect("log"));
        steps.time("3 read record again", || store.read(&name).expect("read"));
        steps.time("4 read base commit", || objects.read_commit(&base).expect("base"));
        let commit = steps.time("5 write commit", || {
            objects
                .write_commit(&NewCommit {
                    tree: base_tree,
                    parents: vec![base],
                    message: format!("publish {run}\n"),
                })
                .expect("commit")
        });
        steps.time("6 replace record (publishing)", || {
            store.replace(&name, &record).expect("replace")
        });
        steps.time("7 pin reference", || store.pin(&name, Some(commit)).expect("pin"));
        steps.time("8 replace record (published)", || {
            store.replace(&name, &record).expect("replace")
        });
        drop(log);
        steps.0.entry("0 total").or_default().push(total.elapsed());
    }
    println!("storage steps a publish adds, {runs} runs:");
    steps.report();
}
