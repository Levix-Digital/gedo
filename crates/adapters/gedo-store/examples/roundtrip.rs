//! Writes one entry into the `refs/gedo/roundtrip` chain of a repository and prints what
//! `refs/gedo/` holds afterwards (quickstart step 7).
//!
//! Usage: `cargo run -p gedo-store --example roundtrip -- <repository>`

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use gedo_core::ports::RefStore;
use gedo_core::{Collection, EntryId, GedoRefName, StoreError};
use gedo_store::{GitObjectStore, GitRefStore, GitRepository};

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: roundtrip <repository>");
        return ExitCode::FAILURE;
    };
    match run(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("roundtrip failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str) -> Result<(), StoreError> {
    let repository = GitRepository::open(path)?;
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository);
    let collection = Collection::new(GedoRefName::collection("roundtrip")?, &objects, &refs);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let id = EntryId::new(&format!("{nanos:032x}"))?;
    let content = format!("roundtrip entry written at {nanos}");
    let head = collection.put(
        &[(id.clone(), content.clone().into_bytes())],
        "gedo: roundtrip example",
    )?;
    let read_back = collection.get(&id)?;
    if read_back.as_deref() != Some(content.as_bytes()) {
        return Err(StoreError::Corrupted {
            path: collection.name().to_string(),
            offset: None,
            reason: format!("entry {id} did not read back"),
        });
    }
    println!("wrote entry {id} in commit {head}");

    for (name, oid) in refs.list(collection.name())? {
        let entries = Collection::new(name.clone(), &objects, &refs).entries()?.len();
        println!("{name} {oid} ({entries} entries)");
    }
    Ok(())
}
