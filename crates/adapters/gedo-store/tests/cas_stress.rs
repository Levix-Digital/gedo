//! Concurrent writers never lose an update (FR-005, FR-006, SC-002): 8 writers in 2 processes
//! make 10,000 compare-and-swap updates of one reference.
//!
//! The second process is this test binary run again with `GEDO_STRESS_REPOSITORY` set; it runs
//! only `stress_writer_process` and prints the values it wrote.

mod support;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use gedo_core::ports::{Expected, ObjectStore, RefOutcome, RefStore, RefUpdate};
use gedo_core::{Collection, EntryId, GedoRefName, GitOid, StoreError};
use gedo_store::{GitObjectStore, GitRefStore, GitRepository};
use support::Sandbox;

const PROCESSES: usize = 2;
const THREADS_PER_PROCESS: usize = 4;
const TOTAL_UPDATES: usize = 10_000;
const UPDATES_PER_WRITER: usize = TOTAL_UPDATES / (PROCESSES * THREADS_PER_PROCESS);
const REPOSITORY_VARIABLE: &str = "GEDO_STRESS_REPOSITORY";
const PROCESS_VARIABLE: &str = "GEDO_STRESS_PROCESS";

fn reference() -> GedoRefName {
    GedoRefName::new("refs/gedo/stress").expect("valid name")
}

/// One writer: `UPDATES_PER_WRITER` successful updates, each to a new value; returns the values.
fn write_updates(repository: &Path, writer: String) -> Vec<GitOid> {
    let repository = GitRepository::open_isolated(repository).expect("opens");
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository);
    let mut written = Vec::with_capacity(UPDATES_PER_WRITER);
    let mut expected = refs.read(&reference()).expect("read");
    for update in 0..UPDATES_PER_WRITER {
        let new = objects
            .write_blob(format!("{writer}-{update}").as_bytes())
            .expect("blob");
        loop {
            let request = RefUpdate {
                name: reference(),
                expected: Expected::from_current(expected),
                new,
                log_message: format!("gedo: stress {writer}"),
            };
            match refs.compare_and_swap(&request).expect("compare-and-swap") {
                RefOutcome::Updated => {
                    expected = Some(new);
                    written.push(new);
                    break;
                }
                RefOutcome::Moved { current } => {
                    assert_ne!(
                        current, expected,
                        "a refusal reports a value other than the expected one"
                    );
                    assert!(current.is_some(), "the reference only ever grows");
                    expected = current;
                }
            }
        }
    }
    written
}

/// Runs `THREADS_PER_PROCESS` writers in this process.
fn run_writers(repository: &Path, process: &str) -> Vec<GitOid> {
    thread::scope(|scope| {
        let writers: Vec<_> = (0..THREADS_PER_PROCESS)
            .map(|thread| scope.spawn(move || write_updates(repository, format!("{process}.{thread}"))))
            .collect();
        writers
            .into_iter()
            .flat_map(|writer| writer.join().expect("writer finished"))
            .collect()
    })
}

#[test]
fn stress_writer_process() {
    let Ok(repository) = std::env::var(REPOSITORY_VARIABLE) else {
        return;
    };
    let process = std::env::var(PROCESS_VARIABLE).expect("process name");
    for value in run_writers(Path::new(&repository), &process) {
        println!("written {value}");
    }
}

#[test]
fn concurrent_writers_never_lose_an_update() {
    let sandbox = Sandbox::new();
    let repository = sandbox.repository("work");

    let mut others = Vec::new();
    for process in 1..PROCESSES {
        let child = Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "stress_writer_process",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(REPOSITORY_VARIABLE, &repository)
            .env(PROCESS_VARIABLE, format!("p{process}"))
            .stdout(Stdio::piped())
            .spawn()
            .expect("second process starts");
        others.push(child);
    }
    let mut written: Vec<GitOid> = run_writers(&repository, "p0");
    for child in others {
        let output = child.wait_with_output().expect("second process finishes");
        assert!(output.status.success(), "second process failed");
        let stdout = String::from_utf8(output.stdout).expect("utf-8");
        written.extend(
            stdout
                .lines()
                // libtest may print `test stress_writer_process ... ` in front of the first line.
                .filter_map(|line| line.split_once("written ").map(|(_, hex)| hex.trim()))
                .map(|hex| GitOid::from_hex(hex).expect("an object id")),
        );
    }

    assert_eq!(written.len(), TOTAL_UPDATES, "every writer completed its updates");
    let distinct: BTreeSet<GitOid> = written.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        TOTAL_UPDATES,
        "no value was reported written twice"
    );

    let reflog = GitRefStore::new(GitRepository::open_isolated(&repository).expect("opens"))
        .reflog(&reference())
        .expect("reflog");
    assert_eq!(
        reflog.len(),
        TOTAL_UPDATES,
        "the reference log has one line per update"
    );
    assert_eq!(
        reflog.iter().copied().collect::<BTreeSet<_>>(),
        distinct,
        "and lists every value"
    );

    let final_value = GitRefStore::new(GitRepository::open_isolated(&repository).expect("opens"))
        .read(&reference())
        .expect("read");
    assert_eq!(
        final_value,
        reflog.last().copied(),
        "the last logged value is the current one"
    );
}

#[test]
fn concurrent_collection_puts_keep_every_entry() {
    const WRITERS: usize = 8;
    const PUTS_PER_WRITER: usize = 20;
    let sandbox = Sandbox::new();
    let repository = sandbox.repository("work");
    let name = GedoRefName::new("refs/gedo/tapes").expect("valid name");

    thread::scope(|scope| {
        for writer in 0..WRITERS {
            let (repository, name) = (&repository, &name);
            scope.spawn(move || {
                let repository = GitRepository::open_isolated(repository).expect("opens");
                let objects = GitObjectStore::new(repository.clone());
                let refs = GitRefStore::new(repository);
                let collection = Collection::new(name.clone(), &objects, &refs);
                for put in 0..PUTS_PER_WRITER {
                    let id = EntryId::new(&format!("{writer:02x}{put:04x}")).expect("valid id");
                    let entries = [(id, format!("{writer}-{put}").into_bytes())];
                    // A put that keeps losing races reports `Contended`; the caller tries again.
                    loop {
                        match collection.put(&entries, "gedo: stress") {
                            Ok(_) => break,
                            Err(StoreError::Contended { .. }) => continue,
                            Err(error) => panic!("put failed: {error}"),
                        }
                    }
                }
            });
        }
    });

    let repository = GitRepository::open_isolated(&repository).expect("opens");
    let (objects, refs) = (
        GitObjectStore::new(repository.clone()),
        GitRefStore::new(repository),
    );
    let entries = Collection::new(name, &objects, &refs).entries().expect("entries");
    assert_eq!(entries.len(), WRITERS * PUTS_PER_WRITER, "no put was lost");
}
