# Contract: Core Ports

The ports `gedo-core` exposes to the rest of Gedo. Every port has a real adapter (`gedo-store`) and an in-memory adapter (`gedo_core::memory`, feature `memory`) that pass the **same contract test suite** (`gedo_core::contract_tests`, run against both). Signatures are indicative; names and error types are binding.

## ObjectStore

```rust
pub trait ObjectStore {
    fn object_hash(&self) -> ObjectHash;
    fn write_blob(&self, data: &[u8]) -> Result<GitOid, StoreError>;
    fn read_blob(&self, oid: &GitOid) -> Result<Vec<u8>, StoreError>;          // ObjectNotFound, TooLarge
    fn write_tree(&self, entries: &[TreeEntry]) -> Result<GitOid, StoreError>;
    fn read_tree(&self, oid: &GitOid) -> Result<Vec<TreeEntry>, StoreError>;
    fn write_commit(&self, commit: &NewCommit) -> Result<GitOid, StoreError>;  // MissingIdentity
    fn read_commit(&self, oid: &GitOid) -> Result<CommitInfo, StoreError>;
}
```

Guarantees: writing the same content twice returns the same id; nothing outside the object database changes (working tree, index, `refs/heads/*`, `HEAD`).

## RefStore

```rust
pub trait RefStore {
    fn read(&self, name: &GedoRefName) -> Result<Option<GitOid>, StoreError>;
    fn list(&self, prefix: &GedoRefName) -> Result<Vec<(GedoRefName, GitOid)>, StoreError>;
    fn list_remote(&self, remote: &str) -> Result<Vec<(String, GitOid)>, StoreError>; // refs/gedo-remote/<remote>/*
    fn compare_and_swap(&self, update: &RefUpdate) -> Result<RefOutcome, StoreError>;  // Updated | Moved { current }
    fn delete(&self, name: &GedoRefName, expected: GitOid) -> Result<RefOutcome, StoreError>;
}
```

Guarantees: only names under `refs/gedo/` are accepted for writes (`GedoRefName` cannot be built otherwise); updates are atomic and recorded in the reflog; concurrent writers in different processes cannot both get `Updated` for the same expected value.

## WriteAheadLog

```rust
pub trait WriteAheadLog {
    fn append(&mut self, payload: &[u8]) -> Result<LogSequence, StoreError>;
    fn sync(&mut self) -> Result<(), StoreError>;                       // durable now
    fn read_all(&self) -> Result<Vec<(LogSequence, Vec<u8>)>, StoreError>; // Corrupted { offset }
    fn truncate_through(&mut self, sequence: LogSequence) -> Result<(), StoreError>;
    fn open_report(&self) -> &OpenReport;                               // torn tail dropped, if any
}
```

Guarantees: records are returned in append order; any record older than 100 ms is durable; `sync` makes every appended record durable before returning; a torn last record is dropped and reported; corruption elsewhere is an error with its offset; a second writer gets `WalBusy` after waiting up to 2 s.

## SymbolCache

```rust
pub trait SymbolCache {
    fn open_status(&self) -> &CacheStatus;                               // Ready | Rebuilt { reason }
    fn get(&self, namespace: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;
    fn put(&mut self, namespace: &str, entries: &[(Vec<u8>, Vec<u8>)]) -> Result<(), StoreError>;
    fn clear(&mut self) -> Result<(), StoreError>;
}

pub trait CacheRebuildSource {
    fn entries(&self) -> Result<Vec<(String, Vec<u8>, Vec<u8>)>, StoreError>; // (namespace, key, value)
}
```

Guarantees: opening with a missing, corrupted or other-schema cache rebuilds it from the `CacheRebuildSource` before returning; lookups after a rebuild equal lookups before it.

## Collection (core logic over the ports)

```rust
pub struct Collection<'a, O: ObjectStore, R: RefStore> { /* name, stores */ }
impl Collection<'_, O, R> {
    pub fn get(&self, id: &EntryId) -> Result<Option<Vec<u8>>, StoreError>;
    pub fn put(&self, entries: &[(EntryId, Vec<u8>)], message: &str) -> Result<GitOid, StoreError>; // retries on Moved (8 attempts)
    pub fn merge_remote(&self, remote: &str) -> Result<MergeOutcome, StoreError>;                    // union merge commit
}
```

Pure logic: no I/O except through the ports; tested with the in-memory adapters.
