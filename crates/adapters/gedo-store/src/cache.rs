//! [`SymbolCache`] in `.git/gedo/symbol_cache.db`, a redb database (ADR-0004).
//!
//! The cache only speeds up lookups: it holds nothing that `refs/gedo/*` does not. A missing,
//! unreadable or other-schema cache is therefore discarded and rebuilt on open. The rebuild is
//! written to a temporary file that replaces the cache only once complete, so an interrupted
//! rebuild never leaves a partial cache that opens as valid.

use std::path::{Path, PathBuf};

use gedo_core::StoreError;
use gedo_core::ports::cache::{CACHE_SCHEMA_VERSION, validate_namespace};
use gedo_core::ports::{CacheRebuildSource, CacheStatus, RebuildReason, SymbolCache};
use redb::{Database, ReadableDatabase, TableDefinition, TableError, TableHandle};

use crate::fs::{retry_sharing_violation, sync_directory};

/// File name of the cache inside `.git/gedo/`.
pub const CACHE_FILE_NAME: &str = "symbol_cache.db";

const META: TableDefinition<&str, u32> = TableDefinition::new("meta");
const SCHEMA_VERSION_KEY: &str = "schema_version";
const NAMESPACE_PREFIX: &str = "ns:";

/// The symbol cache of one repository.
pub struct RedbCache {
    path: PathBuf,
    database: Database,
    status: CacheStatus,
}

impl std::fmt::Debug for RedbCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbCache")
            .field("path", &self.path)
            .field("status", &self.status)
            .finish()
    }
}

fn backend(operation: &'static str, path: &Path, error: impl std::fmt::Display) -> StoreError {
    StoreError::backend(operation, format!("{}: {error}", path.display()))
}

fn table_name(namespace: &str) -> String {
    format!("{NAMESPACE_PREFIX}{namespace}")
}

impl RedbCache {
    /// Opens the cache in `gedo_dir` (normally `.git/gedo/`), rebuilding it from `source` when it
    /// is missing, unreadable or of another schema version.
    pub fn open(gedo_dir: &Path, source: &dyn CacheRebuildSource) -> Result<Self, StoreError> {
        let path = gedo_dir.join(CACHE_FILE_NAME);
        let reason = if path.exists() {
            match RedbCache::open_existing(&path) {
                Ok(database) => {
                    return Ok(RedbCache {
                        path,
                        database,
                        status: CacheStatus::Ready,
                    });
                }
                Err(reason) => reason,
            }
        } else {
            RebuildReason::Missing
        };
        tracing::info!(path = %path.display(), ?reason, "rebuilding the symbol cache");
        let database = RedbCache::rebuild(gedo_dir, &path, source)?;
        Ok(RedbCache {
            path,
            database,
            status: CacheStatus::Rebuilt { reason },
        })
    }

    /// Opens an existing file; the error says why it must be rebuilt instead.
    fn open_existing(path: &Path) -> Result<Database, RebuildReason> {
        // Wait out another program holding the file (Windows), then let redb open it.
        if let Err(error) = retry_sharing_violation(path, || {
            std::fs::OpenOptions::new().read(true).write(true).open(path)
        }) {
            tracing::warn!(path = %path.display(), %error, "cannot open the symbol cache");
            return Err(RebuildReason::Corrupted);
        }
        let database = Database::open(path).map_err(|error| {
            tracing::warn!(path = %path.display(), %error, "symbol cache unreadable");
            RebuildReason::Corrupted
        })?;
        let found = (|| {
            let read = database.begin_read().ok()?;
            let meta = read.open_table(META).ok()?;
            meta.get(SCHEMA_VERSION_KEY).ok()?.map(|value| value.value())
        })()
        .unwrap_or(0);
        if found != CACHE_SCHEMA_VERSION {
            return Err(RebuildReason::SchemaMismatch { found });
        }
        Ok(database)
    }

    /// Builds a fresh cache from `source` in a temporary file and moves it into place.
    fn rebuild(
        gedo_dir: &Path,
        path: &Path,
        source: &dyn CacheRebuildSource,
    ) -> Result<Database, StoreError> {
        let temporary = path.with_extension("db.tmp");
        remove_if_present(&temporary)?;
        let filled = RedbCache::fill(&temporary, source);
        if let Err(error) = filled {
            if let Err(cleanup) = remove_if_present(&temporary) {
                tracing::warn!(path = %temporary.display(), %cleanup, "could not remove an incomplete cache rebuild");
            }
            return Err(error);
        }
        remove_if_present(path)?;
        retry_sharing_violation(path, || std::fs::rename(&temporary, path))
            .map_err(|error| StoreError::io("replace symbol cache", path, error))?;
        sync_directory(gedo_dir).map_err(|error| StoreError::io("sync cache directory", gedo_dir, error))?;
        Database::open(path).map_err(|error| backend("open symbol cache", path, error))
    }

    fn fill(temporary: &Path, source: &dyn CacheRebuildSource) -> Result<(), StoreError> {
        let entries = source.entries()?;
        let database =
            Database::create(temporary).map_err(|error| backend("create symbol cache", temporary, error))?;
        let write = database
            .begin_write()
            .map_err(|error| backend("write symbol cache", temporary, error))?;
        for (namespace, key, value) in &entries {
            validate_namespace(namespace)?;
            let name = table_name(namespace);
            let mut table = write
                .open_table(TableDefinition::<&[u8], &[u8]>::new(&name))
                .map_err(|error| backend("write symbol cache", temporary, error))?;
            table
                .insert(key.as_slice(), value.as_slice())
                .map_err(|error| backend("write symbol cache", temporary, error))?;
        }
        {
            // The schema version goes in last, in the same transaction: a cache without it is
            // never taken as valid.
            let mut meta = write
                .open_table(META)
                .map_err(|error| backend("write symbol cache", temporary, error))?;
            meta.insert(SCHEMA_VERSION_KEY, CACHE_SCHEMA_VERSION)
                .map_err(|error| backend("write symbol cache", temporary, error))?;
        }
        write
            .commit()
            .map_err(|error| backend("commit symbol cache", temporary, error))
    }

    /// The cache file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn remove_if_present(path: &Path) -> Result<(), StoreError> {
    match retry_sharing_violation(path, || std::fs::remove_file(path)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(StoreError::io("remove symbol cache file", path, error)),
    }
}

impl SymbolCache for RedbCache {
    fn open_status(&self) -> &CacheStatus {
        &self.status
    }

    fn get(&self, namespace: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        validate_namespace(namespace)?;
        let read = self
            .database
            .begin_read()
            .map_err(|error| backend("read symbol cache", &self.path, error))?;
        let name = table_name(namespace);
        let table = match read.open_table(TableDefinition::<&[u8], &[u8]>::new(&name)) {
            Ok(table) => table,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(error) => return Err(backend("read symbol cache", &self.path, error)),
        };
        let value = table
            .get(key)
            .map_err(|error| backend("read symbol cache", &self.path, error))?;
        Ok(value.map(|value| value.value().to_vec()))
    }

    fn put(&mut self, namespace: &str, entries: &[(Vec<u8>, Vec<u8>)]) -> Result<(), StoreError> {
        validate_namespace(namespace)?;
        let write = self
            .database
            .begin_write()
            .map_err(|error| backend("write symbol cache", &self.path, error))?;
        {
            let name = table_name(namespace);
            let mut table = write
                .open_table(TableDefinition::<&[u8], &[u8]>::new(&name))
                .map_err(|error| backend("write symbol cache", &self.path, error))?;
            for (key, value) in entries {
                table
                    .insert(key.as_slice(), value.as_slice())
                    .map_err(|error| backend("write symbol cache", &self.path, error))?;
            }
        }
        write
            .commit()
            .map_err(|error| backend("commit symbol cache", &self.path, error))
    }

    fn clear(&mut self) -> Result<(), StoreError> {
        let write = self
            .database
            .begin_write()
            .map_err(|error| backend("write symbol cache", &self.path, error))?;
        let namespaces: Vec<String> = write
            .list_tables()
            .map_err(|error| backend("list symbol cache tables", &self.path, error))?
            .map(|table| table.name().to_owned())
            .filter(|name| name.starts_with(NAMESPACE_PREFIX))
            .collect();
        for name in namespaces {
            write
                .delete_table(TableDefinition::<&[u8], &[u8]>::new(&name))
                .map_err(|error| backend("clear symbol cache", &self.path, error))?;
        }
        write
            .commit()
            .map_err(|error| backend("commit symbol cache", &self.path, error))
    }
}
