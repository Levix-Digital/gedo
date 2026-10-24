//! `cargo xtask host-validate`: round trip and retention of Gedo data on GitHub and GitLab
//! (FR-014, SC-006), against dedicated private test repositories.
//!
//! Each configured host gets a new entry tagged with the run id, pushed with the push arguments
//! of the pre-push hook; then a fresh clone with the remote setup fetches it and checks that every
//! entry any earlier run pushed is still there. A host without its secrets is "not configured".

use std::path::Path;
use std::time::Duration;

use gedo_core::{Collection, EntryId, GedoRefName};
use gedo_store::remote::{configure_remote, push_arguments};
use gedo_store::{GitObjectStore, GitRefStore, GitRepository};

use crate::git::{Git, directory_size};

const COLLECTION: &str = "host-validation";

struct Host {
    name: &'static str,
    url_variable: &'static str,
    token_variable: &'static str,
    /// User name the host expects with a token over HTTPS.
    token_user: &'static str,
}

const HOSTS: [Host; 2] = [
    Host {
        name: "GitHub",
        url_variable: "GEDO_TEST_GITHUB_URL",
        token_variable: "GEDO_TEST_GITHUB_TOKEN",
        token_user: "x-access-token",
    },
    Host {
        name: "GitLab",
        url_variable: "GEDO_TEST_GITLAB_URL",
        token_variable: "GEDO_TEST_GITLAB_TOKEN",
        token_user: "oauth2",
    },
];

struct Measurements {
    entries: usize,
    push: Duration,
    clone: Duration,
    fetch: Duration,
    clone_bytes: u64,
}

enum Outcome {
    NotConfigured,
    Passed(Measurements),
    Failed(String),
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        for position in 0..4 {
            if position <= chunk.len() {
                text.push(ALPHABET[((value >> (18 - 6 * position)) & 0x3f) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

fn run_id() -> String {
    std::env::var("GITHUB_RUN_ID").unwrap_or_else(|_| {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        format!("local-{seconds}")
    })
}

/// An entry id derived from the run id: lowercase hex of its bytes.
fn entry_id(run: &str) -> Result<EntryId, String> {
    let hex: String = run.bytes().map(|byte| format!("{byte:02x}")).collect();
    EntryId::new(&format!("{hex:0>4}")).map_err(|error| error.to_string())
}

/// Clones `url` into `directory/name`, sets up the remote, fetches and merges the Gedo chain.
fn prepared_clone(
    git: &Git,
    directory: &Path,
    url: &str,
    name: &str,
) -> Result<(GitRepository, Duration, Duration), String> {
    let (_, clone) = git.run(directory, &["clone", "--quiet", url, name])?;
    let path = directory.join(name);
    git.run(&path, &["config", "user.name", "Gedo host validation"])?;
    git.run(&path, &["config", "user.email", "host-validation@gedo.invalid"])?;
    let repository = GitRepository::open_isolated(&path).map_err(|error| error.to_string())?;
    configure_remote(&repository, "origin").map_err(|error| error.to_string())?;
    let (_, fetch) = git.run(&path, &["fetch", "--quiet", "origin"])?;
    let (objects, refs) = (
        GitObjectStore::new(repository.clone()),
        GitRefStore::new(repository.clone()),
    );
    let name = GedoRefName::collection(COLLECTION).map_err(|error| error.to_string())?;
    Collection::new(name, &objects, &refs)
        .merge_remote("origin")
        .map_err(|error| error.to_string())?;
    Ok((repository, clone, fetch))
}

fn collection_entries(repository: &GitRepository) -> Result<usize, String> {
    let (objects, refs) = (
        GitObjectStore::new(repository.clone()),
        GitRefStore::new(repository.clone()),
    );
    let name = GedoRefName::collection(COLLECTION).map_err(|error| error.to_string())?;
    Collection::new(name, &objects, &refs)
        .entries()
        .map(|entries| entries.len())
        .map_err(|error| error.to_string())
}

fn validate(host: &Host, url: &str, token: &str, run: &str) -> Result<Measurements, String> {
    let credentials = base64(format!("{}:{token}", host.token_user).as_bytes());
    let git = Git::default()
        .with_config(
            "http.extraHeader",
            &format!("Authorization: Basic {credentials}"),
            true,
        )
        .with_config("user.useConfigOnly", "true", false)
        .with_secret(token);
    let directory = tempfile::tempdir().map_err(|error| format!("temporary directory: {error}"))?;

    // Writer: add this run's entry on top of what earlier runs left, and push it.
    let (writer, _, _) = prepared_clone(&git, directory.path(), url, "writer")?;
    let earlier = collection_entries(&writer)?;
    let (objects, refs) = (
        GitObjectStore::new(writer.clone()),
        GitRefStore::new(writer.clone()),
    );
    let name = GedoRefName::collection(COLLECTION).map_err(|error| error.to_string())?;
    let id = entry_id(run)?;
    let content = format!("host validation run {run}");
    Collection::new(name.clone(), &objects, &refs)
        .put(
            &[(id.clone(), content.clone().into_bytes())],
            &format!("gedo: host validation {run}"),
        )
        .map_err(|error| error.to_string())?;
    let arguments = push_arguments("origin");
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let writer_path = writer
        .git_dir()
        .parent()
        .unwrap_or(directory.path())
        .to_path_buf();
    let (_, push) = git.run(&writer_path, &arguments)?;

    // Reader: a fresh clone must see this run's entry and every earlier one.
    let (reader, clone, fetch) = prepared_clone(&git, directory.path(), url, "reader")?;
    let entries = collection_entries(&reader)?;
    if entries < earlier + 1 {
        return Err(format!(
            "retention: expected at least {} entries, found {entries}",
            earlier + 1
        ));
    }
    let (objects, refs) = (
        GitObjectStore::new(reader.clone()),
        GitRefStore::new(reader.clone()),
    );
    let read_back = Collection::new(name, &objects, &refs)
        .get(&id)
        .map_err(|error| error.to_string())?;
    if read_back.as_deref() != Some(content.as_bytes()) {
        return Err(format!(
            "round trip: entry of run {run} not found after a fresh clone"
        ));
    }
    Ok(Measurements {
        entries,
        push,
        clone,
        fetch,
        clone_bytes: directory_size(&directory.path().join("reader/.git")),
    })
}

fn summary(run: &str, results: &[(&Host, Outcome)]) -> String {
    let mut text = format!(
        "## Gedo host validation, run {run}\n\n| Host | Result | Entries | Push | Clone | Fetch | Clone size |\n|---|---|---|---|---|---|---|\n"
    );
    for (host, outcome) in results {
        let row = match outcome {
            Outcome::NotConfigured => format!("| {} | not configured | | | | | |\n", host.name),
            Outcome::Failed(reason) => format!("| {} | failed: {reason} | | | | | |\n", host.name),
            Outcome::Passed(measured) => format!(
                "| {} | passed | {} | {:.2} s | {:.2} s | {:.2} s | {:.1} KiB |\n",
                host.name,
                measured.entries,
                measured.push.as_secs_f64(),
                measured.clone.as_secs_f64(),
                measured.fetch.as_secs_f64(),
                measured.clone_bytes as f64 / 1024.0,
            ),
        };
        text.push_str(&row);
    }
    text
}

pub fn run(workspace_root: &Path) -> Result<(), String> {
    let run = run_id();
    let mut results = Vec::new();
    for host in &HOSTS {
        let url = std::env::var(host.url_variable).unwrap_or_default();
        let token = std::env::var(host.token_variable).unwrap_or_default();
        let outcome = if url.is_empty() || token.is_empty() {
            Outcome::NotConfigured
        } else {
            match validate(host, &url, &token, &run) {
                Ok(measured) => Outcome::Passed(measured),
                Err(reason) => Outcome::Failed(Git::default().with_secret(&token).redact(&reason)),
            }
        };
        results.push((host, outcome));
    }
    let text = summary(&run, &results);
    print!("{text}");
    let path = match std::env::var("GITHUB_STEP_SUMMARY") {
        Ok(path) if !path.is_empty() => std::path::PathBuf::from(path),
        _ => workspace_root.join("target/host-validation/summary.md"),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let mut existing = std::fs::read_to_string(&path).unwrap_or_default();
    existing.push_str(&text);
    std::fs::write(&path, existing).map_err(|error| format!("{}: {error}", path.display()))?;
    if results
        .iter()
        .any(|(_, outcome)| matches!(outcome, Outcome::Failed(_)))
    {
        return Err("host validation failed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"x-access-token:abc"), "eC1hY2Nlc3MtdG9rZW46YWJj");
    }

    #[test]
    fn run_ids_become_valid_entry_ids() {
        assert!(entry_id("123456789").is_ok());
        assert!(entry_id("local-1792800000").is_ok());
    }

    #[test]
    fn secrets_never_reach_messages() {
        let git = Git::default().with_secret("s3cret");
        assert_eq!(
            git.redact("fatal: https://u:s3cret@host"),
            "fatal: https://u:***@host"
        );
    }

    #[test]
    fn missing_secrets_mean_not_configured() {
        let text = summary("1", &[(&HOSTS[0], Outcome::NotConfigured)]);
        assert!(text.contains("| GitHub | not configured |"));
    }
}
