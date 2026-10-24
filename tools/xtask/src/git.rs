//! Running the `git` executable from the tools, with secrets kept out of arguments and output.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A `git` runner: extra configuration passed through the environment, and texts to redact.
#[derive(Clone, Debug, Default)]
pub struct Git {
    config: Vec<(String, String)>,
    secrets: Vec<String>,
}

impl Git {
    /// Adds a configuration value given to every invocation through `GIT_CONFIG_*` variables, so
    /// it never appears on a command line; `secret` marks it for redaction.
    pub fn with_config(mut self, key: &str, value: &str, secret: bool) -> Self {
        self.config.push((key.to_owned(), value.to_owned()));
        if secret {
            self.secrets.push(value.to_owned());
        }
        self
    }

    /// Marks a text to be replaced by `***` in any message.
    pub fn with_secret(mut self, secret: &str) -> Self {
        if !secret.is_empty() {
            self.secrets.push(secret.to_owned());
        }
        self
    }

    pub fn redact(&self, text: &str) -> String {
        self.secrets.iter().fold(text.to_owned(), |text, secret| {
            text.replace(secret.as_str(), "***")
        })
    }

    /// Runs `git args` in `directory` and returns its standard output and the time it took.
    pub fn run(&self, directory: &Path, args: &[&str]) -> Result<(String, Duration), String> {
        self.run_with_input(directory, args, &[])
    }

    /// Like [`Git::run`], with `input` written to the standard input.
    pub fn run_with_input(
        &self,
        directory: &Path,
        args: &[&str],
        input: &[u8],
    ) -> Result<(String, Duration), String> {
        let mut command = Command::new("git");
        command
            .current_dir(directory)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0");
        command.env("GIT_CONFIG_COUNT", self.config.len().to_string());
        for (index, (key, value)) in self.config.iter().enumerate() {
            command
                .env(format!("GIT_CONFIG_KEY_{index}"), key)
                .env(format!("GIT_CONFIG_VALUE_{index}"), value);
        }
        let started = Instant::now();
        let spawn_error = |error: std::io::Error| format!("git {}: {error}", args.join(" "));
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(spawn_error)?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input).map_err(spawn_error)?;
        }
        let output = child.wait_with_output().map_err(spawn_error)?;
        let elapsed = started.elapsed();
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(self.redact(&format!("git {} failed: {}", args.join(" "), stderr.trim())));
        }
        Ok((String::from_utf8_lossy(&output.stdout).into_owned(), elapsed))
    }
}

/// Total size in bytes of the files under `directory`.
pub fn directory_size(directory: &Path) -> u64 {
    let mut total = 0;
    let mut pending: Vec<PathBuf> = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.metadata() {
                Ok(metadata) if metadata.is_dir() => pending.push(entry.path()),
                Ok(metadata) => total += metadata.len(),
                Err(_) => {}
            }
        }
    }
    total
}
