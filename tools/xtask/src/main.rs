//! `cargo xtask <command>`: repository automation.
//!
//! - `arch-check`: the core depends on no forbidden crate (`.dev-cycle.json`).
//! - `format-corpus`: regenerate the formatter corpus of `gedo-parse` with `rustfmt`.
//! - `parse-sweep`: parse every Rust file of the workspace and its locked dependencies.
//! - `host-validate`: push and fetch Gedo data against the GitHub and GitLab test repositories.
//! - `scale-study`: measure collections of many entries against one reference per entry.

mod arch_check;
mod format_corpus;
mod git;
mod host_validate;
mod parse_sweep;
mod scale_study;

use std::path::PathBuf;
use std::process::ExitCode;

fn workspace_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.first().map(String::as_str) {
        Some("arch-check") => arch_check::run(&workspace_root()),
        Some("format-corpus") => format_corpus::run(&workspace_root()),
        Some("parse-sweep") => parse_sweep::run(&workspace_root()),
        Some("host-validate") => host_validate::run(&workspace_root()),
        Some("scale-study") => scale_study::run(&workspace_root(), &arguments[1..]),
        None | Some("help" | "--help") => {
            println!(
                "usage: cargo xtask <arch-check | format-corpus | parse-sweep | host-validate | scale-study [options]>"
            );
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::FAILURE
        }
    }
}
