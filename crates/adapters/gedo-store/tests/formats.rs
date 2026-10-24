//! Repository formats and identities the store refuses (FR-018, missing identity).

mod support;

use gedo_core::StoreError;
use gedo_core::ports::{NewCommit, ObjectStore};
use gedo_store::{GitObjectStore, GitRepository};
use support::Sandbox;

#[test]
fn sha256_repositories_are_refused() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("sha256");
    std::fs::create_dir_all(&path).expect("directory");
    sandbox.git(&path, &["init", "--quiet", "--object-format=sha256"]);
    let error = GitRepository::open_isolated(&path).expect_err("refused");
    assert!(
        matches!(error, StoreError::UnsupportedObjectFormat { ref found, .. } if found == "sha256"),
        "{error}"
    );
}

#[test]
fn sha1_repositories_open_in_every_layout() {
    let sandbox = Sandbox::new();
    let work = sandbox.repository("work");
    let bare = sandbox.bare_repository("bare.git");
    let linked = sandbox.path().join("linked");
    sandbox.git(
        &work,
        &[
            "worktree",
            "add",
            "--quiet",
            linked.to_str().expect("utf-8"),
            "-b",
            "side",
        ],
    );

    for path in [work.clone(), work.join(".git"), bare, linked] {
        let repository = GitRepository::open_isolated(&path).expect("opens");
        let gedo = repository.gedo_dir().expect("created");
        assert!(gedo.is_dir());
        assert!(
            gedo.starts_with(repository.common_dir()),
            "{} is shared by worktrees",
            gedo.display()
        );
    }
}

#[test]
fn the_identity_comes_from_the_repository_configuration() {
    let sandbox = Sandbox::new();
    let repository = GitRepository::open_isolated(sandbox.repository("work")).expect("opens");
    let identity = repository.identity().expect("configured");
    assert_eq!(
        (identity.name.as_str(), identity.email.as_str()),
        ("Gedo Test", "test@gedo.invalid")
    );
}

#[test]
fn a_missing_identity_is_refused_before_a_commit_is_written() {
    let sandbox = Sandbox::new();
    let repository =
        GitRepository::open_isolated(sandbox.repository_without_identity("anonymous")).expect("opens");
    assert!(matches!(
        repository.identity(),
        Err(StoreError::MissingIdentity { .. })
    ));

    let objects = GitObjectStore::new(repository);
    let tree = objects.write_tree(&[]).expect("trees need no identity");
    let error = objects
        .write_commit(&NewCommit {
            tree,
            parents: vec![],
            message: "gedo: test".into(),
        })
        .expect_err("refused");
    assert!(matches!(error, StoreError::MissingIdentity { .. }), "{error}");
}
