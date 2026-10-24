//! Gedo data travels with plain `git push` and `git fetch` (FR-013, FR-014, SC-006).
//!
//! The tests run the `git` executable as tooling; the store itself never does. The file remote
//! runs everywhere; the SSH remote needs `GEDO_TEST_SSH=1` and an `sshd` on the machine (Linux CI).

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gedo_core::collection::EntryMap;
use gedo_core::ports::RefStore;
use gedo_core::{Collection, EntryId, GedoRefName, MergeOutcome};
use gedo_store::remote::{RemoteSetup, configure_remote, fetch_refspec, push_arguments};
use gedo_store::{GitObjectStore, GitRefStore, GitRepository};
use support::Sandbox;

fn tapes() -> GedoRefName {
    GedoRefName::new("refs/gedo/tapes").expect("valid name")
}

struct Clone {
    path: PathBuf,
    objects: GitObjectStore,
    refs: GitRefStore,
}

impl Clone {
    fn open(path: PathBuf) -> Self {
        let repository = GitRepository::open_isolated(&path).expect("opens");
        Clone {
            path,
            objects: GitObjectStore::new(repository.clone()),
            refs: GitRefStore::new(repository),
        }
    }

    fn collection(&self) -> Collection<'_, GitObjectStore, GitRefStore> {
        Collection::new(tapes(), &self.objects, &self.refs)
    }

    fn put(&self, id: &str, content: &str) {
        let entry = (EntryId::new(id).expect("valid id"), content.as_bytes().to_vec());
        self.collection().put(&[entry], "gedo: test").expect("put");
    }

    fn entries(&self) -> EntryMap {
        self.collection().entries().expect("entries")
    }
}

/// A remote reachable at `url` and the directory holding it.
struct Remote {
    url: String,
}

/// A clone of the remote with an identity and the Gedo remote setup.
fn clone(sandbox: &Sandbox, remote: &Remote, name: &str) -> Clone {
    let path = sandbox.path().join(name);
    sandbox.git(sandbox.path(), &["clone", "--quiet", &remote.url, name]);
    sandbox.set_identity(&path);
    let repository = GitRepository::open_isolated(&path).expect("opens");
    assert_eq!(
        configure_remote(&repository, "origin").expect("setup"),
        RemoteSetup::Added
    );
    Clone::open(path)
}

fn push_gedo(sandbox: &Sandbox, clone: &Clone) -> bool {
    let arguments = push_arguments("origin");
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let output = sandbox
        .git_command(&clone.path, &arguments)
        .output()
        .expect("git runs");
    output.status.success()
}

fn file_remote(sandbox: &Sandbox) -> Remote {
    let source = sandbox.repository("source");
    sandbox.git(
        sandbox.path(),
        &[
            "clone",
            "--quiet",
            "--bare",
            source.to_str().expect("utf-8"),
            "remote.git",
        ],
    );
    Remote {
        url: sandbox
            .path()
            .join("remote.git")
            .to_str()
            .expect("utf-8")
            .to_owned(),
    }
}

fn round_trip(sandbox: &Sandbox, remote: &Remote) {
    let alice = clone(sandbox, remote, "alice");
    alice.put("aaaa01", "from alice");
    assert!(push_gedo(sandbox, &alice), "first push");

    let bob = clone(sandbox, remote, "bob");
    assert!(
        bob.refs.list_remote("origin").expect("list").is_empty(),
        "a clone alone carries no Gedo data"
    );
    sandbox.git(&bob.path, &["fetch", "--quiet", "origin"]);
    let mirrors: BTreeMap<String, _> = bob
        .refs
        .list_remote("origin")
        .expect("list")
        .into_iter()
        .collect();
    assert_eq!(
        mirrors.get("tapes"),
        alice.refs.read(&tapes()).expect("read").as_ref()
    );
    assert!(matches!(
        bob.collection().merge_remote("origin").expect("merge"),
        MergeOutcome::FastForwarded { .. }
    ));
    assert_eq!(bob.entries(), alice.entries());

    // Unpushed local work survives a fetch of newer remote data.
    bob.put("bbbb01", "from bob, not pushed");
    let bob_head = bob.collection().head().expect("head");
    alice.put("aaaa02", "more from alice");
    assert!(push_gedo(sandbox, &alice), "fast-forward push");
    sandbox.git(&bob.path, &["fetch", "--quiet", "origin"]);
    assert_eq!(
        bob.collection().head().expect("head"),
        bob_head,
        "fetch never overwrites local chains"
    );

    // The diverged push is refused, then resolved by fetch, merge and push.
    assert!(!push_gedo(sandbox, &bob), "a diverged chain needs a merge first");
    assert!(matches!(
        bob.collection().merge_remote("origin").expect("merge"),
        MergeOutcome::Merged { .. }
    ));
    assert!(push_gedo(sandbox, &bob), "the merged chain fast-forwards");
    sandbox.git(&alice.path, &["fetch", "--quiet", "origin"]);
    assert!(matches!(
        alice.collection().merge_remote("origin").expect("merge"),
        MergeOutcome::FastForwarded { .. }
    ));
    assert_eq!(alice.entries().len(), 3);
    assert_eq!(alice.entries(), bob.entries());

    // A fresh clone with the setup gets everything.
    let carol = clone(sandbox, remote, "carol");
    sandbox.git(&carol.path, &["fetch", "--quiet", "origin"]);
    carol.collection().merge_remote("origin").expect("merge");
    assert_eq!(carol.entries(), alice.entries());
}

#[test]
fn file_remote_round_trip() {
    let sandbox = Sandbox::new();
    let remote = file_remote(&sandbox);
    round_trip(&sandbox, &remote);
}

#[test]
fn branch_pushes_are_unchanged_by_the_setup() {
    let sandbox = Sandbox::new();
    let remote = file_remote(&sandbox);
    let alice = clone(&sandbox, &remote, "alice");
    alice.put("aaaa01", "gedo data");
    std::fs::write(alice.path.join("file.txt"), "change\n").expect("file");
    sandbox.git(&alice.path, &["add", "file.txt"]);
    sandbox.git(&alice.path, &["commit", "--quiet", "-m", "change"]);

    let output = sandbox.git(&alice.path, &["push", "--porcelain", "origin"]);
    let pushed: Vec<&str> = output.lines().filter(|line| line.contains("refs/")).collect();
    assert_eq!(pushed.len(), 1, "{output}");
    assert!(pushed[0].contains("refs/heads/main:refs/heads/main"), "{output}");
    let remote_refs = sandbox.git(Path::new(&remote.url), &["for-each-ref", "--format=%(refname)"]);
    assert!(
        !remote_refs.contains("refs/gedo"),
        "a plain push does not send Gedo data: {remote_refs}"
    );
}

#[test]
fn the_setup_is_idempotent_and_adds_no_push_refspec() {
    let sandbox = Sandbox::new();
    let remote = file_remote(&sandbox);
    let alice = clone(&sandbox, &remote, "alice");
    let repository = GitRepository::open_isolated(&alice.path).expect("opens");
    assert_eq!(
        configure_remote(&repository, "origin").expect("setup"),
        RemoteSetup::AlreadyConfigured
    );

    let fetch = sandbox.git(&alice.path, &["config", "--get-all", "remote.origin.fetch"]);
    let fetch: Vec<&str> = fetch.lines().collect();
    assert_eq!(
        fetch,
        [
            "+refs/heads/*:refs/remotes/origin/*",
            fetch_refspec("origin").as_str()
        ]
    );
    let push = sandbox
        .git_command(&alice.path, &["config", "--get-all", "remote.origin.push"])
        .output()
        .expect("git runs");
    assert!(!push.status.success(), "no push refspec");

    assert!(
        configure_remote(&repository, "missing").is_err(),
        "unknown remotes are refused"
    );
    assert!(configure_remote(&repository, "../x").is_err());
}

#[test]
#[ignore = "needs sshd; run with GEDO_TEST_SSH=1 and --include-ignored"]
fn ssh_remote_round_trip() {
    if std::env::var("GEDO_TEST_SSH").as_deref() != Ok("1") {
        eprintln!("GEDO_TEST_SSH is not 1; skipping the SSH remote");
        return;
    }
    let sandbox = Sandbox::new();
    let bare = file_remote(&sandbox);
    let server = support::ssh::Server::start(sandbox.path());
    sandbox.set_ssh_command(server.ssh_command());
    let remote = Remote {
        url: server.url(Path::new(&bare.url)),
    };
    round_trip(&sandbox, &remote);
}
