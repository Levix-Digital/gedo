//! Temporary repositories for the integration tests, set up with the `git` executable (test
//! tooling only; the store itself never runs `git`).
#![allow(dead_code)]

pub mod ssh;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A temporary directory holding repositories, with its own empty global Git configuration.
pub struct Sandbox {
    root: TempDir,
    ssh_command: RefCell<Option<String>>,
}

impl Sandbox {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary directory");
        std::fs::write(root.path().join("global.gitconfig"), "").expect("global config");
        Sandbox {
            root,
            ssh_command: RefCell::new(None),
        }
    }

    /// Makes every later `git` invocation reach SSH remotes with `command`.
    pub fn set_ssh_command(&self, command: String) {
        *self.ssh_command.borrow_mut() = Some(command);
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    /// Runs `git` in `directory` isolated from the machine's configuration; panics on failure.
    pub fn git(&self, directory: &Path, args: &[&str]) -> String {
        let output = self.git_command(directory, args).output().expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf-8 output")
    }

    pub fn git_command(&self, directory: &Path, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command
            .current_dir(directory)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.path().join("global.gitconfig"))
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_AUTHOR_NAME")
            .env_remove("GIT_AUTHOR_EMAIL")
            .env_remove("GIT_COMMITTER_NAME")
            .env_remove("GIT_COMMITTER_EMAIL");
        if let Some(ssh) = self.ssh_command.borrow().as_ref() {
            command.env("GIT_SSH_COMMAND", ssh);
        }
        command
    }

    /// A non-bare repository with an identity and one commit on `main`.
    pub fn repository(&self, name: &str) -> PathBuf {
        let path = self.path().join(name);
        std::fs::create_dir_all(&path).expect("directory");
        self.git(&path, &["init", "--quiet", "--initial-branch=main"]);
        self.set_identity(&path);
        std::fs::write(path.join("README.md"), "# test\n").expect("file");
        self.git(&path, &["add", "README.md"]);
        self.git(&path, &["commit", "--quiet", "-m", "initial"]);
        path
    }

    /// A bare repository with an identity.
    pub fn bare_repository(&self, name: &str) -> PathBuf {
        let path = self.path().join(name);
        std::fs::create_dir_all(&path).expect("directory");
        self.git(&path, &["init", "--quiet", "--bare", "--initial-branch=main"]);
        self.set_identity(&path);
        path
    }

    /// A repository without any identity configured.
    pub fn repository_without_identity(&self, name: &str) -> PathBuf {
        let path = self.path().join(name);
        std::fs::create_dir_all(&path).expect("directory");
        self.git(&path, &["init", "--quiet", "--initial-branch=main"]);
        path
    }

    pub fn set_identity(&self, repository: &Path) {
        self.git(repository, &["config", "user.name", "Gedo Test"]);
        self.git(repository, &["config", "user.email", "test@gedo.invalid"]);
    }
}
