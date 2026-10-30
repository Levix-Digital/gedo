//! Port to the local, rebuildable symbol cache (`.git/gedo/symbol_cache.db`).

use crate::error::StoreError;

/// Schema version of the cache this build writes.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

/// Why a cache was rebuilt on open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildReason {
    /// There was no cache.
    Missing,
    /// The cache had another schema version.
    SchemaMismatch {
        /// The version found.
        found: u32,
    },
    /// The cache could not be read.
    Corrupted,
}

/// The state of a cache after opening it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheStatus {
    /// The existing cache was valid.
    Ready,
    /// The cache was discarded and rebuilt from the repository data.
    Rebuilt {
        /// Why.
        reason: RebuildReason,
    },
}

/// A local key-value cache, split into namespaces, that can always be rebuilt from `refs/gedo/*`.
///
/// Writes take `&self`, so the use cases of one process (the identity index and the event log)
/// can share one cache.
pub trait SymbolCache {
    /// What opening found.
    fn open_status(&self) -> &CacheStatus;
    /// The value of `key` in `namespace`.
    fn get(&self, namespace: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;
    /// Inserts or replaces entries of `namespace` in one transaction.
    fn put(&self, namespace: &str, entries: &[(Vec<u8>, Vec<u8>)]) -> Result<(), StoreError>;
    /// Removes every entry of every namespace.
    fn clear(&self) -> Result<(), StoreError>;
}

/// One cache entry: `(namespace, key, value)`.
pub type CacheEntry = (String, Vec<u8>, Vec<u8>);

/// The repository data a cache is rebuilt from.
pub trait CacheRebuildSource {
    /// Every entry to put into a fresh cache.
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError>;
}

/// Validates a namespace name: 1 to 64 characters of `[a-z0-9-_]`, never the reserved `meta`.
pub fn validate_namespace(namespace: &str) -> Result<(), StoreError> {
    const MAX_NAMESPACE_LEN: usize = 64;
    let valid = !namespace.is_empty()
        && namespace.len() <= MAX_NAMESPACE_LEN
        && namespace != "meta"
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(StoreError::invalid_input(
            "cache",
            format!("invalid namespace {namespace:?}"),
        ))
    }
}
