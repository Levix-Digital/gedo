//! Re-attaches the tapes of a squashed branch on a scratch repository (quickstart step 3).
//!
//! `cargo run -p gedo-app --example reattach -- <repository>`
//!
//! On top of `HEAD`, writes a branch of two commits (one adds a function, the next edits it) as
//! loose objects, publishes their changes as live events (written to `refs/gedo/*`: use a scratch
//! repository), writes the squash commit a server would make of the branch, re-attaches it with
//! the branch gone from the history, and prints the attached tapes, the remainder of its events and
//! the detached tapes. No branch or working tree is changed. `HEAD` is resolved with
//! `git rev-parse`, which only this developer tool runs.

use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use gedo_app::{CommitEvents, DEFAULT_GRACE_DAYS, EventLog, IdentityIndex};
use gedo_core::StoreError;
use gedo_core::event::Labels;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_crdt::{RecordingContext, event_id};
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
    let Some(repository) = arguments.first() else {
        eprintln!("usage: reattach <repository>");
        return ExitCode::FAILURE;
    };
    match run(repository) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// A commit on `parent` whose tree is the parent's with `gedo_example.rs` holding `source`.
fn with_file(objects: &GitObjectStore, parent: GitOid, source: &str) -> Result<GitOid, StoreError> {
    let base = objects.read_commit(&parent)?.tree;
    let mut entries: Vec<TreeEntry> = objects
        .read_tree(&base)?
        .into_iter()
        .filter(|entry| entry.name != "gedo_example.rs")
        .collect();
    entries.push(TreeEntry {
        name: "gedo_example.rs".into(),
        mode: EntryMode::Blob,
        oid: objects.write_blob(source.as_bytes())?,
    });
    let tree = objects.write_tree(&entries)?;
    objects.write_commit(&NewCommit {
        tree,
        parents: vec![parent],
        message: "gedo example".into(),
    })
}

fn run(repository: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["-C", repository, "rev-parse", "--verify", "HEAD^{commit}"])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git rev-parse HEAD: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let head = GitOid::from_hex(String::from_utf8(output.stdout)?.trim())?;
    let opened = GitRepository::open(repository)?;
    let gedo_dir = opened.gedo_dir()?;
    std::fs::create_dir_all(&gedo_dir)?;
    let cache = RedbCache::open(&gedo_dir, &NothingToRebuild)?;
    let objects = GitObjectStore::new(opened.clone());
    let refs = GitRefStore::new(opened.clone());
    let parser = TreeSitterParser::new();
    let context = RecordingContext::person(&opened.identity()?.email, Labels::new())?;

    let first = with_file(&objects, head, "pub fn example() -> u32 {\n    1\n}\n")?;
    let second = with_file(&objects, first, "pub fn example() -> u32 {\n    2\n}\n")?;
    let squash = with_file(&objects, head, "pub fn example() -> u32 {\n    2\n}\n")?;
    let mut index = IdentityIndex::open(&objects, &parser, &cache)?;
    index.index_history(&[second, squash])?;

    let mut wal = FileWal::open(&gedo_dir)?;
    let mut log = EventLog::open(&objects, &refs, &mut wal)?.with_cache(&cache);
    for (commit, parent) in [(first, head), (second, first)] {
        let events = log.events_of_commit(&commit, &index)?;
        let ops: Vec<_> = events
            .events()
            .iter()
            .map(|event| (event.operation.clone(), event.result_hash))
            .collect();
        log.record(&context, Some(parent), &ops)?;
        if let Some(tape) = log.publish(commit)? {
            println!("published tape {tape} for {commit}");
        }
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    for tape in log.detached(&[squash], now, DEFAULT_GRACE_DAYS)? {
        println!("detached before: tape {} ({} events)", tape.tape, tape.events);
    }
    let report = log.reattach_new(&[squash], &[squash], &index)?;
    for commit in &report.commits {
        println!(
            "{}: {} tapes attached, {} unmatched changes, {} links",
            commit.commit,
            commit.attached.len(),
            commit.unmatched,
            commit.links
        );
    }
    if let CommitEvents::Published { tapes, remainder } = log.events_of_commit(&squash, &index)? {
        for (tape, events) in &tapes {
            println!("  tape {tape}: {} live events", events.len());
        }
        for event in &remainder {
            println!("  remainder {} {:?}", event_id(event), event.operation);
        }
    }
    println!(
        "detached after: {}",
        log.detached(&[squash], now, DEFAULT_GRACE_DAYS)?.len()
    );
    Ok(())
}
