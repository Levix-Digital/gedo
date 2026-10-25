//! Indexes a repository's history and prints identities (quickstart steps 3 and 4).
//!
//! `cargo run -p gedo-app --example index_history -- <repository> <revision> [<path> | --changes]`
//!
//! The revision is resolved with `git rev-parse`, which only this developer tool runs; Gedo itself
//! never spawns `git` outside the pass-through.

use std::process::{Command, ExitCode};

use gedo_app::IdentityIndex;
use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource};
use gedo_parse::TreeSitterParser;
use gedo_store::{GitObjectStore, GitRepository, RedbCache};

struct NothingToRebuild;

impl CacheRebuildSource for NothingToRebuild {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (Some(repository), Some(revision)) = (arguments.first(), arguments.get(1)) else {
        eprintln!("usage: index_history <repository> <revision> [<path> | --changes]");
        return ExitCode::FAILURE;
    };
    match run(repository, revision, arguments.get(2).map(String::as_str)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(repository: &str, revision: &str, query: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
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
    let mut cache = RedbCache::open(&opened.gedo_dir()?, &NothingToRebuild)?;
    let objects = GitObjectStore::new(opened);
    let parser = TreeSitterParser::new();
    let mut index = IdentityIndex::open(&objects, &parser, &mut cache)?;
    let started = std::time::Instant::now();
    let report = index.index_history(&[commit])?;
    println!(
        "indexed {} commits, parsed {} files in {:.2?} ({} refused)",
        report.commits_indexed,
        report.files_parsed,
        started.elapsed(),
        report.files_refused.len()
    );
    match query {
        Some("--changes") => {
            let changes = index.changes_of(&commit)?;
            changes.deltas.iter().for_each(|delta| println!("{delta:?}"));
            changes.merges.iter().for_each(|merge| println!("{merge:?}"));
        }
        Some(path) => match index.map_at(&commit, path)? {
            Some(map) => {
                for node in &map.nodes {
                    println!("{} {} {}", node.id, node.kind, node.qualified_path);
                }
            }
            None => println!("{path}: no indexed file at {commit}"),
        },
        None => {}
    }
    Ok(())
}
