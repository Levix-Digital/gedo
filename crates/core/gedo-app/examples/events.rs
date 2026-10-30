//! Records, publishes and reads events on a scratch repository (quickstart step 3).
//!
//! `cargo run -p gedo-app --example events -- <repository> <revision>`
//!
//! Indexes the history up to the revision, prints the events reconstructed for it, records them
//! again as live events of an agent with labels on top of its first parent, publishes them as a
//! tape of the revision (written to `refs/gedo/events`, `tapes` and `commit-tapes`: use a scratch
//! repository), and prints the tape and a label filter. The revision is resolved with
//! `git rev-parse`, which only this developer tool runs.

use std::process::{Command, ExitCode};

use gedo_app::{EventLog, IdentityIndex};
use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, ObjectStore};
use gedo_crdt::{EventFilter, RecordingContext, event_id, parse_labels};
use gedo_parse::TreeSitterParser;
use gedo_store::{FileWal, GitObjectStore, GitRefStore, GitRepository, RedbCache};

struct NothingToRebuild;

impl CacheRebuildSource for NothingToRebuild {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (Some(repository), Some(revision)) = (arguments.first(), arguments.get(1)) else {
        eprintln!("usage: events <repository> <revision>");
        return ExitCode::FAILURE;
    };
    match run(repository, revision) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(repository: &str, revision: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args([
            "-C",
            repository,
            "rev-parse",
            "--verify",
            &format!("{revision}^{{commit}}"),
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git rev-parse {revision}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let commit = GitOid::from_hex(String::from_utf8(output.stdout)?.trim())?;
    let opened = GitRepository::open(repository)?;
    let gedo_dir = opened.gedo_dir()?;
    std::fs::create_dir_all(&gedo_dir)?;
    let cache = RedbCache::open(&gedo_dir, &NothingToRebuild)?;
    let objects = GitObjectStore::new(opened.clone());
    let refs = GitRefStore::new(opened.clone());
    let parser = TreeSitterParser::new();
    let mut index = IdentityIndex::open(&objects, &parser, &cache)?;
    index.index_history(&[commit])?;

    let mut wal = FileWal::open(&gedo_dir)?;
    let mut log = EventLog::open(&objects, &refs, &mut wal)?.with_cache(&cache);
    let events = log.events_of_commit(&commit, &index)?;
    let events = events.events();
    println!("{} events at {commit}", events.len());
    for event in &events {
        println!("  {} {:?} {:?}", event_id(event), event.clock, event.origin);
    }

    let email = opened.identity()?.email;
    let context = RecordingContext::from_texts(
        &email,
        Some("example-agent"),
        Some("dc.plan=example,dc.milestone=M1"),
    )?;
    let base = objects.read_commit(&commit)?.parents.first().copied();
    let ops: Vec<_> = events
        .iter()
        .map(|event| (event.operation.clone(), event.result_hash))
        .collect();
    log.record(&context, base, &ops)?;
    match log.publish(commit)? {
        Some(tape) => println!(
            "published tape {tape} with {} events",
            log.tape(&tape)?.events.len()
        ),
        None => println!("nothing to publish"),
    }
    let labels = parse_labels("dc.milestone=M1")?;
    let filter = EventFilter {
        labels: labels
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        ..EventFilter::default()
    };
    println!(
        "{} stored events carry dc.milestone=M1",
        log.filter(&filter)?.len()
    );
    Ok(())
}
