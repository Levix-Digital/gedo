//! `cargo xtask <command>`: repository automation.

use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    match command.as_deref() {
        None | Some("help") => {
            println!("usage: cargo xtask <command>");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown command: {other}");
            ExitCode::FAILURE
        }
    }
}
