//! `cargo xtask parse-sweep`: parses every Rust file of the workspace and of its locked
//! dependencies (SC-006, research R2) and fails on a crash or on a file with syntax errors that
//! is not a listed grammar gap. Reads only the local Cargo registry: run `cargo fetch` first.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;
use gedo_core::ports::Parser;
use gedo_parse::TreeSitterParser;

const KNOWN_GAPS: &str = "crates/adapters/gedo-parse/tests/known_grammar_gaps.toml";

/// Directories that hold no source of the package itself.
const SKIPPED_DIRECTORIES: &[&str] = &["target", ".git", "fuzz"];

#[derive(Default)]
struct Report {
    files: usize,
    lines: usize,
    crashes: Vec<String>,
    unexpected_errors: Vec<String>,
    known_errors: Vec<String>,
    refused: Vec<String>,
}

pub fn run(workspace_root: &Path) -> Result<(), String> {
    let gaps = read_known_gaps(&workspace_root.join(KNOWN_GAPS))?;
    let metadata = MetadataCommand::new()
        .manifest_path(workspace_root.join("Cargo.toml"))
        .exec()
        .map_err(|error| format!("cargo metadata: {error}"))?;
    let mut roots: BTreeSet<PathBuf> = BTreeSet::new();
    for package in &metadata.packages {
        if let Some(directory) = package.manifest_path.parent() {
            roots.insert(directory.as_std_path().to_path_buf());
        }
    }
    let parser = TreeSitterParser::new();
    let mut report = Report::default();
    for root in &roots {
        let label_root = root.parent().unwrap_or(root);
        for file in rust_files(root)? {
            let label = file
                .strip_prefix(label_root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            sweep_file(&parser, &file, &label, &gaps, &mut report)?;
        }
    }
    println!(
        "parse-sweep: {} packages, {} files, {} lines; {} crashes, {} refused, {} files with syntax errors \
         ({} known grammar gaps)",
        roots.len(),
        report.files,
        report.lines,
        report.crashes.len(),
        report.refused.len(),
        report.unexpected_errors.len() + report.known_errors.len(),
        report.known_errors.len(),
    );
    for (title, files) in [
        ("crashed", &report.crashes),
        ("refused", &report.refused),
        ("syntax errors outside the known gaps", &report.unexpected_errors),
    ] {
        for file in files {
            println!("  {title}: {file}");
        }
    }
    if report.crashes.is_empty() && report.unexpected_errors.is_empty() {
        Ok(())
    } else {
        Err("parse-sweep found crashes or unexpected syntax errors".to_owned())
    }
}

fn sweep_file(
    parser: &TreeSitterParser,
    file: &Path,
    label: &str,
    gaps: &BTreeSet<String>,
    report: &mut Report,
) -> Result<(), String> {
    let source = std::fs::read(file).map_err(|error| format!("{}: {error}", file.display()))?;
    report.files += 1;
    report.lines += source.iter().filter(|byte| **byte == b'\n').count();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        parser
            .parse("src/lib.rs", &source)
            .and_then(|parsed| gedo_ast::normalize(&parsed, &source))
    }));
    match outcome {
        Err(_) => report.crashes.push(label.to_owned()),
        Ok(Err(error)) => report.refused.push(format!("{label}: {error}")),
        Ok(Ok(map)) if map.has_error => {
            if gaps.iter().any(|gap| label.starts_with(gap.as_str())) {
                report.known_errors.push(label.to_owned());
            } else {
                report.unexpected_errors.push(label.to_owned());
            }
        }
        Ok(Ok(_)) => {}
    }
    Ok(())
}

fn rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries =
            std::fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|error| format!("{}: {error}", directory.display()))?
                .path();
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if path.is_dir() {
                if !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                    directories.push(path);
                }
            } else if name.ends_with(".rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// The `path = "..."` values of the known gaps list: prefixes of `<package>-<version>/<file>`.
fn read_known_gaps(path: &Path) -> Result<BTreeSet<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path = "))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .collect())
}
