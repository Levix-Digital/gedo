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
