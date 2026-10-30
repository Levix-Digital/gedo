use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::error::StoreError;
use crate::ports::cache::validate_namespace;
use crate::ports::{CacheRebuildSource, CacheStatus, RebuildReason, SymbolCache};

type Entries = BTreeMap<(String, Vec<u8>), Vec<u8>>;

/// A symbol cache in memory.
#[derive(Debug)]
pub struct MemoryCache {
    entries: Mutex<Entries>,
    status: CacheStatus,
}

impl Clone for MemoryCache {
    fn clone(&self) -> Self {
        MemoryCache {
            entries: Mutex::new(self.lock().clone()),
            status: self.status,
        }
    }
}

impl MemoryCache {
    /// Opens a cache that does not exist yet: it is built from `source` (`Rebuilt { Missing }`).
    pub fn open(source: &dyn CacheRebuildSource) -> Result<Self, StoreError> {
        MemoryCache::rebuild(source, RebuildReason::Missing)
    }

    /// Discards the cache and rebuilds it from `source` for `reason`.
    pub fn rebuild(source: &dyn CacheRebuildSource, reason: RebuildReason) -> Result<Self, StoreError> {
        let mut entries = BTreeMap::new();
        for (namespace, key, value) in source.entries()? {
            validate_namespace(&namespace)?;
            entries.insert((namespace, key), value);
        }
        Ok(MemoryCache {
            entries: Mutex::new(entries),
            status: CacheStatus::Rebuilt { reason },
        })
    }

    /// The same cache, as found valid on a later open.
    pub fn reopen(&self) -> Self {
        MemoryCache {
            entries: Mutex::new(self.lock().clone()),
            status: CacheStatus::Ready,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Entries> {
        // A panic while holding the lock leaves the map whole: every write is one insert.
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SymbolCache for MemoryCache {
    fn open_status(&self) -> &CacheStatus {
        &self.status
    }

    fn get(&self, namespace: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        validate_namespace(namespace)?;
        Ok(self.lock().get(&(namespace.to_owned(), key.to_vec())).cloned())
    }

    fn put(&self, namespace: &str, entries: &[(Vec<u8>, Vec<u8>)]) -> Result<(), StoreError> {
        validate_namespace(namespace)?;
        let mut stored = self.lock();
        for (key, value) in entries {
            stored.insert((namespace.to_owned(), key.clone()), value.clone());
        }
        Ok(())
    }

    fn clear(&self) -> Result<(), StoreError> {
        self.lock().clear();
        Ok(())
    }
}
