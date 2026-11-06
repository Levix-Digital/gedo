//! Creates a virtual stream on `HEAD` and lists the streams with its labels (quickstart step 4 of
//! `specs/009-virtual-streams`).
//!
//! `cargo run -p gedo-app --example streams -- <repository> <name> [key=value,...]`
//!
//! `HEAD` is resolved with `git rev-parse`, which only this developer tool runs. The stream is
//! created when it does not exist yet; then every stream carrying the given labels is printed.
//! Only `.git/gedo/streams` and the local pin under `refs/gedo-local/` are written.

use std::process::{Command, ExitCode};

use gedo_app::{StreamError, Streams};
use gedo_core::ids::GitOid;
use gedo_core::stream::StreamName;
use gedo_crdt::{StreamFilter, format_labels, parse_labels};
use gedo_store::{FileStreamStore, GitObjectStore, GitRefStore, GitRepository};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&arguments.len()) {
        eprintln!("usage: streams <repository> <name> [key=value,...]");
        return ExitCode::FAILURE;
    }
    let labels = arguments.get(2).map(String::as_str).unwrap_or("");
    match run(&arguments[0], &arguments[1], labels) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str, name: &str, labels: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["-C", path, "rev-parse", "HEAD"])
        .output()?;
    let head = GitOid::from_hex(String::from_utf8(output.stdout)?.trim())?;
    let repository = GitRepository::open(path)?;
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository.clone());
    let store = FileStreamStore::open(&repository)?;
    let streams = Streams::new(&objects, &refs, &store);
    let name = StreamName::new(name)?;
    match streams.create(&name, None, parse_labels(labels)?, Some(head)) {
        Ok(_) => println!("created {name} on {head}"),
        Err(StreamError::Exists(_)) => println!("{name} already exists"),
        Err(error) => return Err(error.into()),
    }
    for stream in streams.list(&StreamFilter::from_texts(Some(labels), None)?)? {
        println!(
            "{}  base {}  labels [{}]  work {}",
            stream.name,
            stream.base.map_or("-".into(), |base| base.to_hex()),
            format_labels(&stream.labels),
            if stream.work.is_some() { "saved" } else { "base" }
        );
    }
    Ok(())
}
