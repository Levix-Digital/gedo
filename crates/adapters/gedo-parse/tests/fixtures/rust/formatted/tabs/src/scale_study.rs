//! `cargo xtask scale-study [--remote file|<url>] [--entries 100,1000,...]`: push and fetch costs
//! of collections of many entries, stored as one commit chain (ADR-0011) or as one reference per
//! entry (FR-015, SC-007). Writes `target/scale-study/report.md`.
//!
//! For each size: the initial push, a fresh clone plus remote setup plus fetch (time and size
//! added over a clone without Gedo data), then one more entry pushed and fetched incrementally.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gedo_core::ports::ObjectStore as _;
use gedo_core::{Collection, EntryId, GedoRefName};
use gedo_store::remote::{configure_remote, push_arguments};
use gedo_store::{GitObjectStore, GitRefStore, GitRepository};

use crate::git::{Git, directory_size};

const DEFAULT_ENTRIES: [usize; 5] = [100, 1_000, 10_000, 50_000, 100_000];
/// Entries written per chain commit while building a large collection.
const ENTRIES_PER_COMMIT: usize = 1_000;
/// SC-007: fetching 10,000 entries may add at most this much time and data over a plain fetch.
const LIMIT_ENTRIES: usize = 10_000;
const LIMIT_SECONDS: f64 = 5.0;
const LIMIT_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
	Chain,
	ReferencePerEntry,
}

impl Layout {
	fn label(self) -> &'static str {
		match self {
			Layout::Chain => "commit chain",
			Layout::ReferencePerEntry => "reference per entry",
		}
	}
}

struct Row {
	layout: Layout,
	entries: usize,
	initial_push: Duration,
	clone_and_fetch: Duration,
	added_bytes: u64,
	incremental_push: Duration,
	incremental_fetch: Duration,
}

struct Options {
	remote: Option<String>,
	entries: Vec<usize>,
}

fn parse(arguments: &[String]) -> Result<Options, String> {
	let mut options = Options {
		remote: None,
		entries: DEFAULT_ENTRIES.to_vec(),
	};
	let mut arguments = arguments.iter();
	while let Some(argument) = arguments.next() {
		let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
		match argument.as_str() {
			"--remote" => {
				let remote = value()?;
				options.remote = (remote != "file").then(|| remote.clone());
			},
			"--entries" => {
				options.entries = value()?
					.split(',')
					.map(|count| {
						count
							.trim()
							.replace('_', "")
							.parse()
							.map_err(|_| format!("not a count: {count}"))
					})
					.collect::<Result<_, _>>()?;
			},
			other => return Err(format!("unknown option {other}")),
		}
	}
	Ok(options)
}

fn entry_id(index: usize) -> EntryId {
	// Spread ids over the fan-out like content hashes would.
	let mixed = (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (index as u64);
	EntryId::new(&format!("{mixed:016x}{index:08x}")).unwrap_or_else(|_| unreachable!("hex of 24 characters"))
}

fn tapes() -> Result<GedoRefName, String> {
	GedoRefName::collection("tapes").map_err(|error| error.to_string())
}

fn open(path: &Path) -> Result<(GitObjectStore, GitRefStore), String> {
	let repository = GitRepository::open_isolated(path).map_err(|error| error.to_string())?;
	Ok((
		GitObjectStore::new(repository.clone()),
		GitRefStore::new(repository),
	))
}

struct Workspace {
	git: Git,
	directory: tempfile::TempDir,
	remote_url: String,
}

impl Workspace {
	fn new(remote: Option<&str>, name: &str) -> Result<Self, String> {
		let git = Git::default()
			.with_config("user.name", "Gedo scale study", false)
			.with_config("user.email", "scale-study@gedo.invalid", false);
		let directory = tempfile::tempdir().map_err(|error| format!("temporary directory: {error}"))?;
		let source = directory.path().join("source");
		std::fs::create_dir_all(&source).map_err(|error| error.to_string())?;
		git.run(&source, &["init", "--quiet", "--initial-branch=main"])?;
		// The chain commits read the identity from the repository's own configuration.
		git.run(&source, &["config", "user.name", "Gedo scale study"])?;
		git.run(&source, &["config", "user.email", "scale-study@gedo.invalid"])?;
		std::fs::write(source.join("README.md"), "scale study\n").map_err(|error| error.to_string())?;
		git.run(&source, &["add", "README.md"])?;
		git.run(&source, &["commit", "--quiet", "-m", "initial"])?;
		let remote_url = match remote {
			Some(url) => format!("{}/{name}", url.trim_end_matches('/')),
			None => {
				let bare = directory.path().join(format!("{name}.git"));
				git.run(
					directory.path(),
					&["clone", "--quiet", "--bare", path_str(&source)?, path_str(&bare)?],
				)?;
				path_str(&bare)?.to_owned()
			},
		};
		git.run(&source, &["remote", "add", "origin", &remote_url])?;
		if remote.is_some() {
			git.run(&source, &["push", "--quiet", "origin", "main"])?;
		}
		Ok(Workspace {
			git,
			directory,
			remote_url,
		})
	}

	fn source(&self) -> PathBuf {
		self.directory.path().join("source")
	}

	/// A fresh clone with the identity and the Gedo remote setup; returns its path and size.
	fn clone(&self, name: &str) -> Result<(PathBuf, Duration, u64), String> {
		let (_, elapsed) = self.git.run(
			self.directory.path(),
			&["clone", "--quiet", "--no-local", &self.remote_url, name],
		)?;
		let path = self.directory.path().join(name);
		let repository = GitRepository::open_isolated(&path).map_err(|error| error.to_string())?;
		configure_remote(&repository, "origin").map_err(|error| error.to_string())?;
		let size = directory_size(&path.join(".git"));
		Ok((path, elapsed, size))
	}

	/// Pushes `refs/gedo/*` the way the pre-push hook does, for both layouts.
	fn push(&self, repository: &Path) -> Result<Duration, String> {
		let arguments = push_arguments("origin");
		let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
		Ok(self.git.run(repository, &arguments)?.1)
	}
}

fn path_str(path: &Path) -> Result<&str, String> {
	path.to_str()
		.ok_or_else(|| format!("{} is not UTF-8", path.display()))
}

/// Writes `range` entries into the repository in the given layout.
fn write_entries(
	git: &Git,
	repository: &Path,
	layout: Layout,
	range: std::ops::Range<usize>,
) -> Result<(), String> {
	let (objects, refs) = open(repository)?;
	match layout {
		Layout::Chain => {
			let collection = Collection::new(tapes()?, &objects, &refs);
			let ids: Vec<usize> = range.collect();
			for batch in ids.chunks(ENTRIES_PER_COMMIT) {
				let entries: Vec<_> = batch
					.iter()
					.map(|index| (entry_id(*index), format!("entry {index}").into_bytes()))
					.collect();
				collection
					.put(&entries, "gedo: scale study")
					.map_err(|error| error.to_string())?;
			}
		},
		Layout::ReferencePerEntry => {
			let mut commands = String::new();
			for index in range {
				let blob = objects
					.write_blob(format!("entry {index}").as_bytes())
					.map_err(|error| error.to_string())?;
				let _ = writeln!(commands, "create refs/gedo/entries/{} {blob}", entry_id(index));
			}
			git.run_with_input(repository, &["update-ref", "--stdin"], commands.as_bytes())?;
			git.run(repository, &["pack-refs", "--all"])?;
		},
	}
	Ok(())
}

fn measure(remote: Option<&str>, layout: Layout, entries: usize) -> Result<Row, String> {
	let name = format!(
		"{}-{entries}",
		if layout == Layout::Chain { "chain" } else { "refs" }
	);
	let workspace = Workspace::new(remote, &name)?;
	let source = workspace.source();
	write_entries(&workspace.git, &source, layout, 0..entries)?;
	let initial_push = workspace.push(&source)?;

	let (baseline, _, _) = workspace.clone("baseline")?;
	let before = directory_size(&baseline.join(".git"));
	let (reader, clone_time, _) = workspace.clone("reader")?;
	let (_, fetch_time) = workspace.git.run(&reader, &["fetch", "--quiet", "origin"])?;
	let added_bytes = directory_size(&reader.join(".git")).saturating_sub(before);
	let (mirrored, _) = workspace.git.run(
		&reader,
		&["for-each-ref", "--format=x", "refs/gedo-remote/origin/"],
	)?;
	if mirrored.lines().count() == 0 {
		return Err(format!("{name}: the fetch mirrored no Gedo references"));
	}

	write_entries(&workspace.git, &source, layout, entries..entries + 1)?;
	let incremental_push = workspace.push(&source)?;
	let (_, incremental_fetch) = workspace.git.run(&reader, &["fetch", "--quiet", "origin"])?;

	Ok(Row {
		layout,
		entries,
		initial_push,
		clone_and_fetch: clone_time + fetch_time,
		added_bytes,
		incremental_push,
		incremental_fetch,
	})
}

fn report(
	remote: &str,
	rows: &[Row],
	failures: &[(Layout, usize, String)],
	baseline_clone: Duration,
) -> String {
	let mut text = format!(
		"# Scale study\n\nRemote: {remote}. Plain clone without Gedo data: {:.2} s. Chains are built {ENTRIES_PER_COMMIT} entries per commit.\n\n\
         | Layout | Entries | Initial push | Clone + fetch | Data added | Incremental push | Incremental fetch |\n\
         |---|---:|---:|---:|---:|---:|---:|\n",
		baseline_clone.as_secs_f64()
	);
	for row in rows {
		let _ = writeln!(
			text,
			"| {} | {} | {:.2} s | {:.2} s | {:.0} KiB | {:.2} s | {:.2} s |",
			row.layout.label(),
			row.entries,
			row.initial_push.as_secs_f64(),
			row.clone_and_fetch.as_secs_f64(),
			row.added_bytes as f64 / 1024.0,
			row.incremental_push.as_secs_f64(),
			row.incremental_fetch.as_secs_f64(),
		);
	}
	for (layout, entries, error) in failures {
		let error = error.lines().next().unwrap_or_default();
		let _ = writeln!(
			text,
			"| {} | {entries} | failed: {error} | | | | |",
			layout.label()
		);
	}
	if let Some(row) = rows
		.iter()
		.find(|row| row.layout == Layout::Chain && row.entries == LIMIT_ENTRIES)
	{
		let added_seconds = (row.clone_and_fetch.saturating_sub(baseline_clone)).as_secs_f64();
		let pass = added_seconds <= LIMIT_SECONDS && row.added_bytes <= LIMIT_BYTES;
		let _ = writeln!(
			text,
			"\nSC-007 (chain, {LIMIT_ENTRIES} entries): {added_seconds:.2} s and {:.1} MiB over a plain clone; limit {LIMIT_SECONDS} s and 50 MiB: **{}**.",
			row.added_bytes as f64 / (1024.0 * 1024.0),
			if pass { "pass" } else { "fail" }
		);
	}
	text
}

pub fn run(workspace_root: &Path, arguments: &[String]) -> Result<(), String> {
	let options = parse(arguments)?;
	let remote = options.remote.as_deref();
	let baseline = {
		let workspace = Workspace::new(remote, "baseline")?;
		workspace.clone("plain")?.1
	};
	let mut rows = Vec::new();
	let mut failures = Vec::new();
	for &entries in &options.entries {
		for layout in [Layout::Chain, Layout::ReferencePerEntry] {
			eprintln!("scale-study: {} with {entries} entries", layout.label());
			match measure(remote, layout, entries) {
				Ok(row) => rows.push(row),
				// A layout that breaks at some size is itself a result.
				Err(error) => failures.push((layout, entries, error)),
			}
		}
	}
	let text = report(remote.unwrap_or("file"), &rows, &failures, baseline);
	print!("{text}");
	let directory = workspace_root.join("target/scale-study");
	std::fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
	std::fs::write(directory.join("report.md"), text).map_err(|error| format!("report: {error}"))?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn options_parse() {
		let options = parse(&[
			"--entries".into(),
			"100, 1_000".into(),
			"--remote".into(),
			"file".into(),
		])
		.expect("valid");
		assert_eq!(options.entries, vec![100, 1000]);
		assert_eq!(options.remote, None);
		assert!(parse(&["--entries".into(), "many".into()]).is_err());
		assert!(parse(&["--other".into()]).is_err());
	}

	#[test]
	fn entry_ids_are_distinct_and_spread() {
		let ids: std::collections::BTreeSet<_> = (0..10_000).map(entry_id).collect();
		assert_eq!(ids.len(), 10_000);
		let directories: std::collections::BTreeSet<_> =
			ids.iter().map(|id| id.fan_out().0.to_owned()).collect();
		assert_eq!(directories.len(), 256);
	}
}
