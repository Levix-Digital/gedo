//! The port contract suite run against the real adapters.

mod support;

use gedo_store::{GitObjectStore, GitRepository};
use support::Sandbox;

/// A store over a fresh repository; the sandbox is leaked so the directory outlives the test's
/// stores (it is removed with the system's temporary files).
fn fresh_repository() -> GitRepository {
    let sandbox = Box::leak(Box::new(Sandbox::new()));
    GitRepository::open_isolated(sandbox.repository("work")).expect("opens")
}

mod objects {
    use super::*;
    gedo_core::object_store_contract!(|| GitObjectStore::new(fresh_repository()));
}

mod refs {
    use super::*;
    use gedo_store::GitRefStore;
    gedo_core::ref_store_contract!(|| {
        let repository = fresh_repository();
        (
            GitObjectStore::new(repository.clone()),
            GitRefStore::new(repository),
        )
    });
}

mod collections {
    use super::*;
    use gedo_store::GitRefStore;
    gedo_core::collection_contract!(|| {
        let repository = fresh_repository();
        (
            GitObjectStore::new(repository.clone()),
            GitRefStore::new(repository),
        )
    });
}

mod wal {
    use gedo_store::FileWal;
    gedo_core::wal_contract!(|| {
        let directory = Box::leak(Box::new(tempfile::tempdir().expect("temporary directory")));
        FileWal::open(directory.path()).expect("opens")
    });
}
