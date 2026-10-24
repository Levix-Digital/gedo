//! A missing, corrupted or other-schema cache is rebuilt and answers the same lookups (FR-010, SC-005).

use std::path::Path;

use gedo_core::StoreError;
use gedo_core::contract_tests::FixedSource;
use gedo_core::ports::{CacheEntry, CacheRebuildSource, CacheStatus, RebuildReason, SymbolCache};
use gedo_store::RedbCache;
use gedo_store::cache::CACHE_FILE_NAME;
use redb::{Database, TableDefinition};

fn source() -> FixedSource {
    FixedSource(
        (0..200u32)
            .map(|index| {
                let namespace = if index % 2 == 0 { "symbols" } else { "files" };
                (
                    namespace.to_owned(),
                    format!("key-{index}").into_bytes(),
                    format!("value-{index}").into_bytes(),
                )
            })
            .collect(),
    )
}

fn assert_answers_every_lookup(cache: &RedbCache) {
    for (namespace, key, value) in source().0 {
        assert_eq!(cache.get(&namespace, &key).expect("get"), Some(value));
    }
    assert_eq!(cache.get("symbols", b"absent").expect("get"), None);
}

fn set_schema_version(path: &Path, version: u32) {
    let database = Database::open(path).expect("opens");
    let write = database.begin_write().expect("write");
    {
        let mut meta = write
            .open_table(TableDefinition::<&str, u32>::new("meta"))
            .expect("meta");
        meta.insert("schema_version", version).expect("insert");
    }
    write.commit().expect("commit");
}

#[test]
fn a_valid_cache_opens_ready_with_its_contents() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut cache = RedbCache::open(directory.path(), &source()).expect("opens");
    assert_eq!(
        *cache.open_status(),
        CacheStatus::Rebuilt {
            reason: RebuildReason::Missing
        }
    );
    cache
        .put("symbols", &[(b"extra".to_vec(), b"kept".to_vec())])
        .expect("put");
    drop(cache);

    let cache = RedbCache::open(directory.path(), &FixedSource(vec![])).expect("opens");
    assert_eq!(*cache.open_status(), CacheStatus::Ready);
    assert_answers_every_lookup(&cache);
    assert_eq!(
        cache.get("symbols", b"extra").expect("get"),
        Some(b"kept".to_vec())
    );
}

#[test]
fn a_deleted_cache_is_rebuilt() {
    let directory = tempfile::tempdir().expect("temporary directory");
    drop(RedbCache::open(directory.path(), &source()).expect("opens"));
    std::fs::remove_file(directory.path().join(CACHE_FILE_NAME)).expect("delete");
    let cache = RedbCache::open(directory.path(), &source()).expect("opens");
    assert_eq!(
        *cache.open_status(),
        CacheStatus::Rebuilt {
            reason: RebuildReason::Missing
        }
    );
    assert_answers_every_lookup(&cache);
}

#[test]
fn a_corrupted_cache_is_rebuilt() {
    let directory = tempfile::tempdir().expect("temporary directory");
    drop(RedbCache::open(directory.path(), &source()).expect("opens"));
    let path = directory.path().join(CACHE_FILE_NAME);
    let mut state = 0x2545_f491_u32;
    let garbage: Vec<u8> = (0..64 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    std::fs::write(&path, garbage).expect("corrupt");
    let cache = RedbCache::open(directory.path(), &source()).expect("opens");
    assert_eq!(
        *cache.open_status(),
        CacheStatus::Rebuilt {
            reason: RebuildReason::Corrupted
        }
    );
    assert_answers_every_lookup(&cache);
}

#[test]
fn other_schema_versions_are_rebuilt() {
    for version in [0, 2] {
        let directory = tempfile::tempdir().expect("temporary directory");
        drop(RedbCache::open(directory.path(), &source()).expect("opens"));
        set_schema_version(&directory.path().join(CACHE_FILE_NAME), version);
        let cache = RedbCache::open(directory.path(), &source()).expect("opens");
        assert_eq!(
            *cache.open_status(),
            CacheStatus::Rebuilt {
                reason: RebuildReason::SchemaMismatch { found: version }
            }
        );
        assert_answers_every_lookup(&cache);
    }
}

/// A source that fails while the cache is being rebuilt.
struct FailingSource;

impl CacheRebuildSource for FailingSource {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Err(StoreError::backend("read rebuild source", "simulated failure"))
    }
}

#[test]
fn an_interrupted_rebuild_never_leaves_a_ready_cache() {
    let directory = tempfile::tempdir().expect("temporary directory");
    drop(RedbCache::open(directory.path(), &source()).expect("opens"));
    set_schema_version(&directory.path().join(CACHE_FILE_NAME), 2);
    assert!(RedbCache::open(directory.path(), &FailingSource).is_err());
    let leftovers: Vec<_> = std::fs::read_dir(directory.path())
        .expect("readable")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    let cache = RedbCache::open(directory.path(), &source()).expect("opens");
    assert!(
        matches!(cache.open_status(), CacheStatus::Rebuilt { .. }),
        "the next open rebuilds again"
    );
    assert_answers_every_lookup(&cache);
}

#[cfg(windows)]
#[test]
fn a_cache_held_without_sharing_is_retried_then_rebuilt_or_fails_with_context() {
    use std::os::windows::fs::OpenOptionsExt;
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().expect("temporary directory");
    drop(RedbCache::open(directory.path(), &source()).expect("opens"));
    let path = directory.path().join(CACHE_FILE_NAME);
    let holder = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .expect("held");
    let started = Instant::now();
    let error = RedbCache::open(directory.path(), &source()).expect_err("cannot replace a held cache");
    assert!(started.elapsed() >= Duration::from_millis(gedo_store::fs::SHARING_RETRY_MS));
    assert!(matches!(&error, StoreError::Io { .. }), "{error}");
    drop(holder);
    RedbCache::open(directory.path(), &source()).expect("opens once released");
}
