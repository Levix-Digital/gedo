//! Converges branches of a repository over their common base (quickstart step 4).
//!
//! `cargo run -p gedo-app --example converge -- <repository> <base> <tip>...`
//!
//! Each argument after the repository is a revision Git can resolve (a branch, a tag, a commit
//! id); they are resolved with `git rev-parse`, which only this developer tool runs. The commits are
//! indexed, the tips converged over the base, and the entities changed, the identity links and
//! the conflicts are printed. Only the local identity cache is written.

use std::process::{Command, ExitCode};

use gedo_app::{EventLog, IdentityIndex, Side};
use gedo_core::StoreError;
use gedo_core::ids::GitOid;
use gedo_core::ports::{CacheEntry, CacheRebuildSource};
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
    if arguments.len() < 3 {
        eprintln!("usage: converge <repository> <base> <tip>...");
        return ExitCode::FAILURE;
    }
    match run(&arguments[0], &arguments[1], &arguments[2..]) {
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

fn run(repository: &str, base: &str, tips: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let base = resolve(repository, base)?;
    let tips = tips
        .iter()
        .map(|tip| resolve(repository, tip))
        .collect::<Result<Vec<_>, _>>()?;
    let opened = GitRepository::open(repository)?;
    let gedo_dir = opened.gedo_dir()?;
    std::fs::create_dir_all(&gedo_dir)?;
    let cache = RedbCache::open(&gedo_dir, &NothingToRebuild)?;
    let objects = GitObjectStore::new(opened.clone());
    let refs = GitRefStore::new(opened);
    let parser = TreeSitterParser::new();
    let mut index = IdentityIndex::open(&objects, &parser, &cache)?;
    index.index_history(&tips)?;

    let mut wal = FileWal::open(&gedo_dir)?;
    let mut log = EventLog::open(&objects, &refs, &mut wal)?.with_cache(&cache);
    let sides: Vec<Side> = tips.iter().map(|tip| Side::commit(*tip)).collect();
    let report = log.converge(&base, &sides, &index)?;
    for (side, commits) in report.commits.iter().enumerate() {
        println!(
            "side {side}: {} commits, {} events",
            commits.len(),
            report.events[side]
        );
    }
    let base_state = {
        let mut maps = Vec::new();
        for path in index.files_at(&base)? {
            maps.extend(index.map_at(&base, &path)?);
        }
        gedo_crdt::TreeState::from_maps(&maps)
    };
    for (id, entity) in report.convergence.state.entities() {
        if base_state.entity(*id) != Some(entity) {
            let state = if entity.present { "changed" } else { "removed" };
            println!("{state} {} {} ({id})", entity.place.path, entity.qualified_path());
        }
    }
    for (kept, merged) in &report.convergence.links {
        println!("linked {merged} to {kept}");
    }
    for conflict in &report.convergence.conflicts {
        let sides: Vec<String> = conflict
            .values
            .iter()
            .map(|value| format!("{:?} on sides {:?}", value.value, value.sides))
            .collect();
        println!(
            "conflict {:?} on {}: {}",
            conflict.kind,
            conflict.entity,
            sides.join("; ")
        );
    }
    Ok(())
}
