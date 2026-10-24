# Data Model: Store Foundation

Types live in `gedo-core` unless noted; adapters map them to `gix` and `redb` types.

## ObjectHash and GitOid

- `ObjectHash`: `Sha1` (supported) or `Sha256` (detected and refused until FEAT-008).
- `GitOid`: the object hash kind plus its bytes (20 for SHA-1, 32 for SHA-256). Parsed from and displayed as lowercase hex. Equality and ordering by bytes.

## StoredObject

- `Blob(bytes)`: opaque payload.
- `Tree(entries)`: sorted entries `{ name, mode (blob | tree), oid }`; names are ASCII, no `/`, unique.
- `Commit { tree, parents, message }`: written only for collection chains; author and committer come from the repository identity (refused when missing).

Validation: tree entries sorted with Git's ordering; names non-empty.

## GedoRefName

- Full name under `refs/gedo/` (for example `refs/gedo/tapes`). Construction fails for any name outside the namespace, with `..`, uppercase letters or characters outside `[a-z0-9-/]`.
- Read-only remote mirror names: `refs/gedo-remote/<remote>/<collection>`, produced only by `git fetch`.

## RefUpdate (compare-and-swap)

- `expected`: `Absent` or `Value(GitOid)`.
- `new`: `GitOid`.
- `log_message`: text for the reflog (`gedo: <operation>`).
- Outcome: `Updated`, or `Moved { current: Option<GitOid> }` when the expected value no longer matches. Creating with `Absent` when the reference already holds the same new value is `Updated` (idempotent).

## Collection (commit chain)

- Name: a `GedoRefName`.
- Head: the commit the reference points to; its tree maps entry ids to blobs through a two-level fan-out (`ab/cdef...`).
- Operations: `put(entries)` writes blobs, builds the new tree from the head tree, writes a commit with the head as parent, and compare-and-swaps the reference; on `Moved`, it reloads and retries (bounded, 8 attempts). `merge(remote_head)` writes a merge commit whose tree is the union of both trees; the same id with different content is impossible for content-addressed entries and is reported as corruption if seen.
- State transitions: `Absent → Head(c1) → Head(c2) ...`; merges add commits with two parents.

## RecordEnvelope

- `format: u16`, `payload: bytes` (postcard). Limits: total size ≤ `MAX_RECORD_BYTES` (64 MiB). Unknown `format` → `UnsupportedFormat { found, supported }`.

## WAL file (`.git/gedo/wal.log`)

- Header: magic `GEDOWAL\0` (8 bytes), file format version `u16 LE` (currently 1).
- Record frame: `length u32 LE` (bytes of version and payload), `crc32 u32 LE` (over version and payload), `version u16 LE`, `payload`.
- `LogSequence`: zero-based index of a record in the file since the last truncation.
- Open result: `records` (ordered), `torn_tail: Option<offset>` (dropped and truncated), or the error `Corrupted { offset }`.
- Operations: `append(payload) -> LogSequence`, `sync()`, `read_all()`, `truncate_through(sequence)` (keeps later records, rewrites atomically through a temporary file and rename).
- Lock: `.git/gedo/wal.lock`, exclusive OS lock held by the writer for its lifetime.

## Symbol cache (`.git/gedo/symbol_cache.db`)

- Table `meta`: `schema_version: u32` (currently 1).
- Tables per namespace: byte key → byte value.
- Open result: `Ready` or `Rebuilt { reason: Missing | SchemaMismatch { found } | Corrupted }`.

## Remote setup

- Per remote: one `remote.<name>.fetch` value `+refs/gedo/*:refs/gedo-remote/<name>/*`, added once; other values untouched; never a `remote.<name>.push` value.
- Push arguments: `push <remote> refs/gedo/*:refs/gedo/*` (no `+`).

## Errors (`StoreError`, in `gedo-core`)

`UnsupportedObjectFormat`, `NotInGedoNamespace { name }`, `ObjectNotFound { oid }`, `RefMoved` (as an outcome, not an error), `MissingIdentity`, `WalBusy`, `Corrupted { path, offset }`, `UnsupportedFormat { found, supported }`, `TooLarge { size, limit }`, `Io { operation, path, source }`, `Backend { operation, message }`. Every error carries the operation and the path or reference involved.
