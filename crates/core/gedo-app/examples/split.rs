//! Split-publishes the work of a commit on its base (quickstart step 4 of
//! `specs/007-semantic-lanes`).
//!
//! `cargo run -p gedo-app --example split -- <repository> <base> <final-commit>`
//!
//! Both revisions are resolved with `git rev-parse`, which only this developer tool runs. The tree
//! of the final commit is split-published on the base with an empty log: the refactor and feature
//! commits (or the single commit and the reason) are written as objects and printed with their
//! changes; no branch moves. Only Git objects and the local identity cache are written.

use std::process::{Command, ExitCode};

use gedo_app::{EventLog, IdentityIndex, SplitOutcome, SplitRequest};
use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, ObjectStore};
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
    if arguments.len() != 3 {
        eprintln!("usage: split <repository> <base> <final-commit>");
        return ExitCode::FAILURE;
    }
    match run(&arguments[0], &arguments[1], &arguments[2]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn resolve(repository: &str, revision: &str) -> Result<GitOid, Box<dyn std::error::Error>> {
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
    Ok(GitOid::from_hex(String::from_utf8(output.stdout)?.trim())?)
}

fn run(repository: &str, base: &str, work: &str) -> Result<(), Box<dyn std::error::Error>> {
    let base = resolve(repository, base)?;
    let work = resolve(repository, work)?;
    let opened = GitRepository::open(repository)?;
    let gedo_dir = opened.gedo_dir()?;
    std::fs::create_dir_all(&gedo_dir)?;
    let cache = RedbCache::open(&gedo_dir, &NothingToRebuild)?;
    let objects = GitObjectStore::new(opened.clone());
    let refs = GitRefStore::new(opened);
    let parser = TreeSitterParser::new();
    let mut index = IdentityIndex::open(&objects, &parser, &cache)?;
    index.index_history(&[base])?;

    let work_commit = objects.read_commit(&work)?;
    let request = SplitRequest {
        base,
        tree: work_commit.tree,
        message: work_commit.message,
        refactor_message: None,
    };
    let mut wal = FileWal::open(&gedo_dir)?;
    if !gedo_core::ports::WriteAheadLog::read_all(&wal)?.is_empty() {
        return Err("the write-ahead log is not empty; publish it first".into());
    }
    let mut log = EventLog::open(&objects, &refs, &mut wal)?.with_cache(&cache);
    let outcome = log.publish_split(&request, &mut index, &parser)?;
    let print = |label: &str, commit: GitOid| -> Result<(), Box<dyn std::error::Error>> {
        let message = objects.read_commit(&commit)?.message;
        println!("{label} {commit}: {}", message.lines().next().unwrap_or(""));
        for delta in index.changes_of(&commit)?.deltas {
            println!("  {delta:?}");
        }
        Ok(())
    };
    match outcome {
        SplitOutcome::Split { refactor, feature } => {
            print("refactor", refactor.commit)?;
            print("feature", feature.commit)?;
        }
        SplitOutcome::Single { commit, reason } => {
            println!("not split: {reason:?}");
            print("single", commit.commit)?;
        }
        SplitOutcome::Nothing => println!("nothing to publish"),
    }
    Ok(())
}
