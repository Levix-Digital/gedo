//! `cargo xtask format-corpus`: regenerates the formatter corpus of `gedo-parse`
//! (`tests/fixtures/rust/formatted/`, research R5 of `specs/002-syntax-model`).
//!
//! Each source is formatted once with `rustfmt`'s default configuration (`default/`), then that
//! baseline is reformatted with each variant configuration (`<variant>/`). The formatting test
//! checks that every entity keeps its hashes from the baseline to every variant. The output is
//! committed, so the tests never run `rustfmt`.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

/// Variant configurations, by directory name. `imports_granularity` and `use_try_shorthand` are
/// left out on purpose: they change the code, not only its layout (spec edge cases).
const VARIANTS: &[(&str, &str)] = &[
    ("narrow", "max_width=60,trailing_comma=Never"),
    ("wide", "max_width=140"),
    ("tabs", "hard_tabs=true,match_block_trailing_comma=true"),
];

/// Workspace sources added to the fixtures, with their corpus paths.
const WORKSPACE_SOURCES: &[(&str, &str)] = &[
    ("crates/core/gedo-core/src/collection.rs", "src/collection.rs"),
    ("crates/core/gedo-core/src/ids.rs", "src/ids.rs"),
    ("crates/core/gedo-core/src/record.rs", "src/record.rs"),
    ("crates/adapters/gedo-store/src/wal.rs", "src/wal.rs"),
    ("crates/adapters/gedo-store/src/refs.rs", "src/refs.rs"),
    ("crates/adapters/gedo-store/src/cache.rs", "src/cache.rs"),
    ("tools/xtask/src/scale_study.rs", "src/scale_study.rs"),
];

const FIXTURES: &str = "crates/adapters/gedo-parse/tests/fixtures/rust";

pub fn run(workspace_root: &Path) -> Result<(), String> {
    let fixtures = workspace_root.join(FIXTURES);
    let output = fixtures.join("formatted");
    if output.exists() {
        std::fs::remove_dir_all(&output).map_err(|error| format!("{}: {error}", output.display()))?;
    }
    let mut sources = vec![
        (fixtures.join("src/lib.rs"), "src/lib.rs".to_owned()),
        (fixtures.join("src/a/mod.rs"), "src/a/mod.rs".to_owned()),
    ];
    sources.extend(
        WORKSPACE_SOURCES
            .iter()
            .map(|(source, corpus_path)| (workspace_root.join(source), (*corpus_path).to_owned())),
    );
    for (source, corpus_path) in &sources {
        let baseline = output.join("default").join(corpus_path);
        copy(source, &baseline)?;
        rustfmt(&baseline, None)?;
        for (variant, config) in VARIANTS {
            let target = output.join(variant).join(corpus_path);
            copy(&baseline, &target)?;
            rustfmt(&target, Some(config))?;
        }
    }
    println!(
        "format-corpus: {} sources, {} variants, written to {}",
        sources.len(),
        VARIANTS.len(),
        output.display()
    );
    Ok(())
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", from.display()))
}

/// Formats `file` in place through standard input, so `rustfmt` does not follow `mod` declarations
/// to files that are not in the corpus.
fn rustfmt(file: &Path, config: Option<&str>) -> Result<(), String> {
    let mut command = Command::new("rustfmt");
    command.args(["--edition", "2024"]);
    if let Some(config) = config {
        command.args(["--config", config]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("rustfmt could not start: {error}"))?;
    let source = std::fs::read(file).map_err(|error| format!("{}: {error}", file.display()))?;
    child
        .stdin
        .take()
        .ok_or("rustfmt has no standard input")?
        .write_all(&source)
        .map_err(|error| format!("rustfmt input: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("rustfmt: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustfmt failed on {} ({})",
            file.display(),
            output.status
        ));
    }
    std::fs::write(file, output.stdout).map_err(|error| format!("{}: {error}", file.display()))
}
