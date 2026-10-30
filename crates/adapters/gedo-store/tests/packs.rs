//! Large blob batches are written as one pack (BUG-009): no loose object, readable by `git` and
//! by a store opened later, and a process can keep writing packs past gitoxide's default slots.

mod support;

use std::path::Path;

use gedo_core::Collection;
use gedo_core::ids::{EntryId, GedoRefName};
use gedo_core::ports::ObjectStore;
use gedo_store::{GitObjectStore, GitRefStore, GitRepository};
use support::Sandbox;

fn contents(prefix: &str, count: usize) -> Vec<Vec<u8>> {
    (0..count).map(|n| format!("{prefix} {n}").into_bytes()).collect()
}

fn write(objects: &GitObjectStore, contents: &[Vec<u8>]) -> Vec<gedo_core::GitOid> {
    let batch: Vec<&[u8]> = contents.iter().map(Vec::as_slice).collect();
    objects.write_blobs(&batch).expect("written")
}

fn loose_objects(git_dir: &Path) -> usize {
    std::fs::read_dir(git_dir.join("objects"))
        .expect("objects directory")
        .map(|entry| entry.expect("entry"))
        .filter(|entry| entry.file_name().len() == 2)
        .map(|entry| {
            std::fs::read_dir(entry.path())
                .expect("fan-out directory")
                .count()
        })
        .sum()
}

fn pack_files(git_dir: &Path, extension: &str) -> usize {
    match std::fs::read_dir(git_dir.join("objects/pack")) {
        Ok(entries) => entries
            .filter(|entry| {
                entry
                    .as_ref()
                    .expect("entry")
                    .path()
                    .extension()
                    .is_some_and(|found| found == extension)
            })
            .count(),
        Err(_) => 0,
    }
}

#[test]
fn a_large_batch_is_one_pack() {
    let sandbox = Sandbox::new();
    let work = sandbox.repository("work");
    let repository = GitRepository::open_isolated(&work).expect("opens");
    let git_dir = repository.git_dir().to_path_buf();
    let objects = GitObjectStore::new(repository);
    let loose_before = loose_objects(&git_dir);

    let blobs = contents("packed", 100);
    let ids = write(&objects, &blobs);

    assert_eq!(loose_objects(&git_dir), loose_before, "no loose object written");
    assert_eq!(pack_files(&git_dir, "pack"), 1);
    assert_eq!(pack_files(&git_dir, "idx"), 1);
    assert_eq!(pack_files(&git_dir, "keep"), 0, "the pack can be repacked");
    let reopened = GitObjectStore::new(GitRepository::open_isolated(&work).expect("opens"));
    for (id, content) in ids.iter().zip(&blobs) {
        assert_eq!(objects.read_blob(id).expect("read"), *content);
        assert_eq!(reopened.read_blob(id).expect("read"), *content);
    }
    let shown = sandbox.git(&work, &["cat-file", "-p", &ids[42].to_hex()]);
    assert_eq!(shown.as_bytes(), blobs[42]);
    sandbox.git(&work, &["fsck", "--strict", "--no-dangling"]);

    // Writing the same batch again writes nothing.
    assert_eq!(write(&objects, &blobs), ids);
    assert_eq!(pack_files(&git_dir, "pack"), 1);
}

#[test]
fn a_small_batch_and_known_objects_stay_loose() {
    let sandbox = Sandbox::new();
    let work = sandbox.repository("work");
    let repository = GitRepository::open_isolated(&work).expect("opens");
    let git_dir = repository.git_dir().to_path_buf();
    let objects = GitObjectStore::new(repository);
    let loose_before = loose_objects(&git_dir);

    write(&objects, &contents("loose", 99));
    assert_eq!(loose_objects(&git_dir), loose_before + 99);
    assert_eq!(pack_files(&git_dir, "pack"), 0);

    // 99 known blobs and 50 new ones: under the threshold of new objects.
    let mut mixed = contents("loose", 99);
    mixed.extend(contents("more", 50));
    write(&objects, &mixed);
    assert_eq!(loose_objects(&git_dir), loose_before + 149);
    assert_eq!(pack_files(&git_dir, "pack"), 0);
}

#[test]
fn one_process_keeps_writing_packs_past_the_default_slots() {
    let sandbox = Sandbox::new();
    let work = sandbox.repository("work");
    let objects = GitObjectStore::new(GitRepository::open_isolated(&work).expect("opens"));
    let mut written = Vec::new();
    for batch in 0..40 {
        let blobs = contents(&format!("batch {batch}"), 100);
        let ids = write(&objects, &blobs);
        written.extend(ids.into_iter().zip(blobs));
    }
    for (id, content) in &written {
        assert_eq!(objects.read_blob(id).expect("read"), *content);
    }
}

#[test]
fn a_pack_left_without_its_index_is_ignored() {
    let sandbox = Sandbox::new();
    let work = sandbox.repository("work");
    let repository = GitRepository::open_isolated(&work).expect("opens");
    let git_dir = repository.git_dir().to_path_buf();
    // What an interrupted write could leave: pack data without the index renamed next to it.
    let pack_dir = git_dir.join("objects/pack");
    std::fs::create_dir_all(&pack_dir).expect("pack directory");
    std::fs::write(
        pack_dir.join("pack-0123456789abcdef0123456789abcdef01234567.pack"),
        b"PACK\0",
    )
    .expect("written");
    let objects = GitObjectStore::new(repository);
    let blobs = contents("after a crash", 120);
    let ids = write(&objects, &blobs);
    for (id, content) in ids.iter().zip(&blobs) {
        assert_eq!(objects.read_blob(id).expect("read"), *content);
    }
}

#[test]
fn a_large_collection_write_leaves_only_its_root_and_commit_loose() {
    let sandbox = Sandbox::new();
    let work = sandbox.repository("work");
    let repository = GitRepository::open_isolated(&work).expect("opens");
    let git_dir = repository.git_dir().to_path_buf();
    let objects = GitObjectStore::new(repository.clone());
    let refs = GitRefStore::new(repository);
    let collection = Collection::new(GedoRefName::collection("events").expect("valid"), &objects, &refs);
    let entries: Vec<(EntryId, Vec<u8>)> = (0..1_000u32)
        .map(|n| {
            let id = format!("{:08x}{:024x}", n.wrapping_mul(2_654_435_761), n);
            (
                EntryId::new(&id).expect("valid"),
                format!("entry {n}").into_bytes(),
            )
        })
        .collect();
    let loose_before = loose_objects(&git_dir);
    collection.put(&entries, "gedo: test").expect("put");
    assert_eq!(
        loose_objects(&git_dir),
        loose_before + 2,
        "the root tree and the commit"
    );
    assert_eq!(
        pack_files(&git_dir, "pack"),
        2,
        "the blobs, then the fan-out trees"
    );
    assert_eq!(collection.entries().expect("entries").len(), 1_000);
    sandbox.git(&work, &["fsck", "--strict", "--no-dangling"]);
}
