//! The file stream store on real repositories: pins keep saved work through `git gc`, linked
//! working trees have their own state, and pins are never pushed (`specs/009-virtual-streams`
//! research R2).

mod support;

use gedo_core::GitOid;
use gedo_core::ports::{EntryMode, NewCommit, ObjectStore, StreamStore, TreeEntry, WriteAheadLog};
use gedo_core::stream::StreamName;
use gedo_store::{FileStreamStore, FileWal, GitObjectStore, GitRepository};
use support::Sandbox;

fn unreferenced_commit(objects: &GitObjectStore, text: &str) -> (GitOid, GitOid) {
    let blob = objects.write_blob(text.as_bytes()).expect("blob");
    let tree = objects
        .write_tree(&[TreeEntry {
            name: "work.rs".into(),
            mode: EntryMode::Blob,
            oid: blob,
        }])
        .expect("tree");
    let commit = objects
        .write_commit(&NewCommit {
            tree,
            parents: vec![],
            message: "gedo stream work\n".into(),
        })
        .expect("commit");
    (commit, blob)
}

fn exists(sandbox: &Sandbox, repository: &std::path::Path, oid: &GitOid) -> bool {
    sandbox
        .git_command(repository, &["cat-file", "-e", &oid.to_hex()])
        .status()
        .expect("git runs")
        .success()
}

#[test]
fn pinned_work_survives_gc_and_unpinned_work_does_not() {
    let sandbox = Sandbox::new();
    let path = sandbox.repository("work");
    let repository = GitRepository::open_isolated(&path).expect("opens");
    let objects = GitObjectStore::new(repository.clone());
    let streams = FileStreamStore::open(&repository).expect("opens");
    let name = StreamName::new("dc/m2").expect("name");
    assert!(streams.create(&name, b"record").expect("created"));

    let (stream_commit, stream_blob) = unreferenced_commit(&objects, "pub fn stream() {}\n");
    let (outside_commit, outside_blob) = unreferenced_commit(&objects, "pub fn outside() {}\n");
    let (_, loose_blob) = unreferenced_commit(&objects, "pub fn nobody() {}\n");
    streams.pin(&name, Some(stream_commit)).expect("pinned");
    streams.write_state(b"state").expect("state");
    streams.pin_outside(Some(outside_commit)).expect("pinned");

    sandbox.git(&path, &["gc", "--quiet", "--prune=now"]);
    assert!(exists(&sandbox, &path, &stream_blob), "the stream's work is kept");
    assert!(exists(&sandbox, &path, &outside_blob), "the outside work is kept");
    assert!(!exists(&sandbox, &path, &loose_blob), "unpinned work is pruned");

    let log = streams.open_log(&name).expect("log");
    streams.remove(&name, log).expect("removed");
    sandbox.git(&path, &["reflog", "expire", "--expire=now", "--all"]);
    sandbox.git(&path, &["gc", "--quiet", "--prune=now"]);
    assert!(
        !exists(&sandbox, &path, &stream_blob),
        "a discarded stream's work may be pruned"
    );
    assert!(
        sandbox
            .git(&path, &["for-each-ref", "refs/gedo-local/streams"])
            .is_empty()
    );
}

#[test]
fn linked_working_trees_share_streams_and_keep_their_own_state() {
    let sandbox = Sandbox::new();
    let path = sandbox.repository("work");
    let linked = sandbox.path().join("linked");
    sandbox.git(
        &path,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "side",
            linked.to_str().expect("utf-8"),
        ],
    );
    let main = FileStreamStore::open(&GitRepository::open_isolated(&path).expect("opens")).expect("opens");
    let other_repository = GitRepository::open_isolated(&linked).expect("opens");
    assert_eq!(other_repository.worktree_id(), "linked");
    let other = FileStreamStore::open(&other_repository).expect("opens");

    let name = StreamName::new("a").expect("name");
    assert!(main.create(&name, b"record").expect("created"));
    assert_eq!(
        other.names().expect("names"),
        vec![name.clone()],
        "streams are shared"
    );
    assert!(
        !other.create(&name, b"again").expect("create"),
        "one stream per name across working trees"
    );

    main.write_state(b"main state").expect("state");
    other.write_state(b"linked state").expect("state");
    assert_eq!(
        main.read_state().expect("read").as_deref(),
        Some(&b"main state"[..])
    );
    assert_eq!(
        other.read_state().expect("read").as_deref(),
        Some(&b"linked state"[..])
    );
}

#[test]
fn names_that_file_systems_reserve_are_stored() {
    let sandbox = Sandbox::new();
    let path = sandbox.repository("work");
    let streams = FileStreamStore::open(&GitRepository::open_isolated(&path).expect("opens")).expect("opens");
    for text in ["con", "nul", "aux/com1", "a.", "dc/m2"] {
        let name = StreamName::new(text).expect("name");
        assert!(streams.create(&name, text.as_bytes()).expect("created"), "{text}");
        assert_eq!(
            streams.read(&name).expect("read").as_deref(),
            Some(text.as_bytes())
        );
    }
    assert_eq!(streams.names().expect("names").len(), 5);
}

#[test]
fn pins_are_not_pushed() {
    let sandbox = Sandbox::new();
    let path = sandbox.repository("work");
    let remote = sandbox.bare_repository("remote.git");
    let repository = GitRepository::open_isolated(&path).expect("opens");
    let objects = GitObjectStore::new(repository.clone());
    let streams = FileStreamStore::open(&repository).expect("opens");
    let name = StreamName::new("a").expect("name");
    assert!(streams.create(&name, b"record").expect("created"));
    let (commit, _) = unreferenced_commit(&objects, "pub fn w() {}\n");
    streams.pin(&name, Some(commit)).expect("pinned");
    sandbox.git(
        &path,
        &[
            "push",
            "--quiet",
            remote.to_str().expect("utf-8"),
            "main",
            "refs/gedo/*:refs/gedo/*",
        ],
    );
    assert!(
        sandbox
            .git(&remote, &["for-each-ref", "refs/gedo-local"])
            .is_empty()
    );
}

#[test]
fn a_new_stream_starts_clear_of_what_an_earlier_one_left() {
    let sandbox = Sandbox::new();
    let path = sandbox.repository("leftovers");
    let repository = GitRepository::open_isolated(&path).expect("opens");
    let streams = FileStreamStore::open(&repository).expect("opens");
    let name = StreamName::new("dc/m2").expect("name");
    // A removal that did not finish: the stream's directory with a log and no record, and a
    // creation that did not finish: a temporary record.
    let directory = repository
        .gedo_dir()
        .expect("gedo dir")
        .join("streams")
        .join(name.key());
    std::fs::create_dir_all(&directory).expect("directory");
    {
        let mut log = FileWal::open(&directory).expect("log");
        log.append(b"an old event").expect("append");
        log.sync().expect("sync");
    }
    std::fs::write(directory.join("stream.tmp-1-0"), b"half a record").expect("write");
    assert!(streams.names().expect("names").is_empty(), "no record, no stream");

    assert!(streams.create(&name, b"record").expect("created"));
    let log = streams.open_log(&name).expect("log");
    assert!(
        log.read_all().expect("read").is_empty(),
        "the old log was cleared"
    );
    let leftovers: Vec<_> = std::fs::read_dir(&directory)
        .expect("list")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|file| file.contains(".tmp-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporary files were removed: {leftovers:?}"
    );
}

#[test]
fn a_pin_written_over_a_packed_one_wins_and_removal_clears_both() {
    let sandbox = Sandbox::new();
    let path = sandbox.repository("packed");
    let repository = GitRepository::open_isolated(&path).expect("opens");
    let objects = GitObjectStore::new(repository.clone());
    let streams = FileStreamStore::open(&repository).expect("opens");
    let name = StreamName::new("dc/m3").expect("name");
    assert!(streams.create(&name, b"record").expect("created"));
    let reference = format!("refs/gedo-local/streams/{}", name.key());
    let (first, _) = unreferenced_commit(&objects, "pub fn first() {}\n");
    let (second, _) = unreferenced_commit(&objects, "pub fn second() {}\n");
    streams.pin(&name, Some(first)).expect("pinned");
    sandbox.git(&path, &["pack-refs", "--all"]);
    assert!(
        std::fs::read_to_string(path.join(".git/packed-refs"))
            .expect("packed refs")
            .contains(&reference),
        "the pin is packed"
    );
    assert!(
        !path.join(".git").join(&reference).exists(),
        "and no longer loose"
    );

    streams.pin(&name, Some(second)).expect("pinned again");
    assert_eq!(
        sandbox.git(&path, &["rev-parse", &reference]).trim(),
        second.to_hex(),
        "the loose pin takes precedence over the packed one"
    );
    sandbox.git(&path, &["fsck", "--no-progress"]);

    let log = streams.open_log(&name).expect("log");
    streams.remove(&name, log).expect("removed");
    assert!(
        sandbox
            .git(&path, &["for-each-ref", "refs/gedo-local/streams"])
            .is_empty(),
        "neither the loose nor the packed pin is left"
    );
}

#[test]
fn a_pin_waits_for_another_writer_of_its_reference() {
    let sandbox = Sandbox::new();
    let path = sandbox.repository("locked");
    let repository = GitRepository::open_isolated(&path).expect("opens");
    let objects = GitObjectStore::new(repository.clone());
    let streams = FileStreamStore::open(&repository).expect("opens");
    let name = StreamName::new("dc/m4").expect("name");
    assert!(streams.create(&name, b"record").expect("created"));
    let (commit, _) = unreferenced_commit(&objects, "pub fn locked() {}\n");
    let directory = path.join(".git/refs/gedo-local/streams");
    std::fs::create_dir_all(&directory).expect("directory");
    let lock = directory.join(format!("{}.lock", name.key()));
    std::fs::write(&lock, b"").expect("another writer's lock");
    let release = {
        let lock = lock.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            std::fs::remove_file(lock).expect("released");
        })
    };
    streams
        .pin(&name, Some(commit))
        .expect("pinned once the lock is released");
    release.join().expect("thread");
    assert!(!lock.exists());
    assert_eq!(
        sandbox
            .git(
                &path,
                &["rev-parse", &format!("refs/gedo-local/streams/{}", name.key())]
            )
            .trim(),
        commit.to_hex()
    );
}
