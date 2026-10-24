use std::collections::BTreeMap;

use crate::error::StoreError;
use crate::ports::cache::validate_namespace;
use crate::ports::{CacheRebuildSource, CacheStatus, RebuildReason, SymbolCache};

/// A symbol cache in memory.
#[derive(Clone, Debug)]
pub struct MemoryCache {
    entries: BTreeMap<(String, Vec<u8>), Vec<u8>>,
    status: CacheStatus,
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
            entries,
            status: CacheStatus::Rebuilt { reason },
        })
    }

    /// The same cache, as found valid on a later open.
    pub fn reopen(&self) -> Self {
        MemoryCache {
            entries: self.entries.clone(),
            status: CacheStatus::Ready,
        }
    }
}

impl SymbolCache for MemoryCache {
    fn open_status(&self) -> &CacheStatus {
        &self.status
    }

    fn get(&self, namespace: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        validate_namespace(namespace)?;
        Ok(self.entries.get(&(namespace.to_owned(), key.to_vec())).cloned())
    }

    fn put(&mut self, namespace: &str, entries: &[(Vec<u8>, Vec<u8>)]) -> Result<(), StoreError> {
        validate_namespace(namespace)?;
        for (key, value) in entries {
            self.entries
                .insert((namespace.to_owned(), key.clone()), value.clone());
        }
        Ok(())
    }

    fn clear(&mut self) -> Result<(), StoreError> {
        self.entries.clear();
        Ok(())
    }
}
