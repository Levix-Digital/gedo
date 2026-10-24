//! The port contract suite run against the in-memory adapters.

use gedo_core::memory::{MemoryCache, MemoryObjectStore, MemoryRefStore, MemoryWal};

mod objects {
    use super::*;
    gedo_core::object_store_contract!(MemoryObjectStore::new);
}

mod refs {
    use super::*;
    gedo_core::ref_store_contract!(|| (MemoryObjectStore::new(), MemoryRefStore::new()));
}

mod wal {
    use super::*;
    gedo_core::wal_contract!(MemoryWal::new);
}

mod cache {
    use super::*;
    gedo_core::cache_contract!(|source| MemoryCache::open(source).expect("opens"));
}

mod memory_only {
    use gedo_core::ports::{NewCommit, ObjectStore, WriteAheadLog};
    use gedo_core::{StoreError, memory::MemoryObjectStore, memory::MemoryWal};

    #[test]
    fn commits_need_an_identity() {
        let objects = MemoryObjectStore::without_identity();
        let tree = objects.write_tree(&[]).expect("written");
        let error = objects
            .write_commit(&NewCommit {
                tree,
                parents: vec![],
                message: "gedo: x".into(),
            })
            .expect_err("refused");
        assert!(matches!(error, StoreError::MissingIdentity { .. }));
    }

    #[test]
    fn a_crash_keeps_only_durable_records() {
        let mut wal = MemoryWal::new();
        wal.append(b"synced").expect("append");
        wal.sync().expect("sync");
        wal.append(b"in the window").expect("append");
        assert_eq!(wal.crash().read_all().expect("read").len(), 1);
        wal.elapse_durability_window();
        assert_eq!(wal.crash().read_all().expect("read").len(), 2);
    }
}
