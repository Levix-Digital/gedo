//! `cargo xtask <command>`: repository automation.
//!
//! - `arch-check`: the core depends on no forbidden crate (`.dev-cycle.json`).
//! - `host-validate`: push and fetch Gedo data against the GitHub and GitLab test repositories.
//! - `scale-study`: measure collections of many entries against one reference per entry.

mod arch_check;
mod git;
mod host_validate;

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
        Some("host-validate") => host_validate::run(&workspace_root()),
        None | Some("help" | "--help") => {
            println!("usage: cargo xtask <arch-check | host-validate | scale-study [options]>");
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
