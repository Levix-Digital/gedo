//! Storing and reading Gedo data never changes the working tree, the index, `HEAD` or branches
//! (FR-003, SC-001).

mod support;

use std::path::{Path, PathBuf};

use gedo_core::ports::{EntryMode, NewCommit, ObjectStore, TreeEntry};
use gedo_store::{GitObjectStore, GitRepository};
use support::Sandbox;

/// Everything a developer sees of a repository, captured byte for byte.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    status: String,
    index: Option<Vec<u8>>,
    head: Vec<u8>,
    branches: String,
    files: Vec<(PathBuf, Vec<u8>)>,
}

fn snapshot(sandbox: &Sandbox, repository: &Path, git_dir: &Path) -> Snapshot {
    let bare = sandbox
        .git(repository, &["rev-parse", "--is-bare-repository"])
        .trim()
        == "true";
    let status = if bare {
        String::new()
    } else {
        sandbox.git(repository, &["status", "--porcelain=v2", "--branch"])
    };
    let mut files = Vec::new();
    if !bare {
        collect_files(repository, repository, &mut files);
    }
    Snapshot {
        status,
        index: std::fs::read(git_dir.join("index")).ok(),
        head: std::fs::read(git_dir.join("HEAD")).expect("HEAD"),
        branches: sandbox.git(
            repository,
            &["for-each-ref", "refs/heads", "refs/tags", "refs/remotes"],
        ),
        files,
    }
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .expect("readable")
        .map(|entry| entry.expect("entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).expect("inside the root").to_path_buf();
            files.push((relative, std::fs::read(&path).expect("readable")));
        }
    }
}

/// Every object store operation, including failing ones.
fn exercise(objects: &GitObjectStore) {
    let blob = objects.write_blob(b"gedo data").expect("blob");
    assert_eq!(objects.read_blob(&blob).expect("read"), b"gedo data");
    let fan_out = objects
        .write_tree(&[TreeEntry {
            name: "cdef".into(),
            mode: EntryMode::Blob,
            oid: blob,
        }])
        .expect("tree");
    let root = objects
        .write_tree(&[TreeEntry {
            name: "ab".into(),
            mode: EntryMode::Tree,
            oid: fan_out,
        }])
        .expect("tree");
    objects.read_tree(&root).expect("read");
    let commit = objects
        .write_commit(&NewCommit {
            tree: root,
            parents: vec![],
            message: "gedo: test".into(),
        })
        .expect("commit");
    objects.read_commit(&commit).expect("read");
    let merge = NewCommit {
        tree: root,
        parents: vec![commit, commit],
        message: "gedo: merge".into(),
    };
    objects.write_commit(&merge).expect("commit");
    assert!(objects.read_tree(&blob).is_err());
}

fn assert_invariant(sandbox: &Sandbox, repository: &Path) {
    let opened = GitRepository::open_isolated(repository).expect("opens");
    let git_dir = opened.git_dir().to_path_buf();
    let before = snapshot(sandbox, repository, &git_dir);
    exercise(&GitObjectStore::new(opened));
    assert_eq!(snapshot(sandbox, repository, &git_dir), before);
    sandbox.git(repository, &["fsck", "--strict", "--no-dangling"]);
}

#[test]
fn uncommitted_edits_and_untracked_files_stay_untouched() {
    let sandbox = Sandbox::new();
    let repository = sandbox.repository("work");
    std::fs::write(repository.join("README.md"), "# edited\n").expect("edit");
    std::fs::write(repository.join("untracked.txt"), "new\n").expect("file");
    assert_invariant(&sandbox, &repository);
}

#[test]
fn staged_changes_stay_staged() {
    let sandbox = Sandbox::new();
    let repository = sandbox.repository("work");
    std::fs::write(repository.join("staged.txt"), "staged\n").expect("file");
    sandbox.git(&repository, &["add", "staged.txt"]);
    assert_invariant(&sandbox, &repository);
}

#[test]
fn a_detached_head_stays_detached() {
    let sandbox = Sandbox::new();
    let repository = sandbox.repository("work");
    sandbox.git(&repository, &["checkout", "--quiet", "--detach"]);
    assert_invariant(&sandbox, &repository);
}

#[test]
fn a_linked_worktree_stays_untouched() {
    let sandbox = Sandbox::new();
    let repository = sandbox.repository("work");
    let linked = sandbox.path().join("linked");
    sandbox.git(
        &repository,
        &[
            "worktree",
            "add",
            "--quiet",
            linked.to_str().expect("utf-8"),
            "-b",
            "side",
        ],
    );
    std::fs::write(linked.join("README.md"), "# edited in the worktree\n").expect("edit");
    assert_invariant(&sandbox, &linked);
    assert_invariant(&sandbox, &repository);
}

#[test]
fn a_bare_repository_keeps_its_branches() {
    let sandbox = Sandbox::new();
    let source = sandbox.repository("source");
    let bare = sandbox.path().join("bare.git");
    sandbox.git(
        sandbox.path(),
        &[
            "clone",
            "--quiet",
            "--bare",
            source.to_str().expect("utf-8"),
            "bare.git",
        ],
    );
    sandbox.set_identity(&bare);
    assert_invariant(&sandbox, &bare);
}
