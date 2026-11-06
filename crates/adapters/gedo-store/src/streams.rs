//! [`StreamStore`] over files in `.git/gedo/streams/` and local references under
//! `refs/gedo-local/` (`specs/009-virtual-streams/research.md` R2).
//!
//! Each stream is a directory named by the hexadecimal key of its name, shared by every working
//! tree, holding its record (`stream`) and its write-ahead log (`wal.log`). The state of a working
//! tree is `gedo/stream-state` in that working tree's own Git directory. Pins are references to
//! commits outside `refs/gedo/`, so they are never pushed or fetched, and `git gc` keeps what they
//! reach.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gedo_core::ports::StreamStore;
use gedo_core::stream::StreamName;
use gedo_core::{GitOid, StoreError};
use gix::refs::Target;
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};

use crate::convert::to_gix_oid;
use crate::fs::{retry_sharing_violation, sync_directory};
use crate::refs::REF_LOCK_WAIT_MS;
use crate::repository::GitRepository;
use crate::wal::FileWal;

/// Directory of the streams, inside `.git/gedo/`.
pub const STREAMS_DIR_NAME: &str = "streams";
/// File of a stream's record, inside its directory.
pub const STREAM_RECORD_FILE_NAME: &str = "stream";
/// File of a working tree's state, inside its own `gedo/` directory.
pub const STREAM_STATE_FILE_NAME: &str = "stream-state";
/// Prefix of the pins of streams.
pub const STREAM_PIN_PREFIX: &str = "refs/gedo-local/streams/";
/// Prefix of the pins of the outside work of working trees.
pub const WORKTREE_PIN_PREFIX: &str = "refs/gedo-local/worktrees/";

/// The streams of a repository, seen from one of its working trees.
#[derive(Clone, Debug)]
pub struct FileStreamStore {
    repository: GitRepository,
    streams: PathBuf,
    state: PathBuf,
    worktree: String,
}

impl FileStreamStore {
    /// The streams of `repository`, seen from the working tree it was opened in.
    pub fn open(repository: &GitRepository) -> Result<Self, StoreError> {
        let streams = repository.gedo_dir()?.join(STREAMS_DIR_NAME);
        fs::create_dir_all(&streams).map_err(|error| StoreError::io("create directory", &streams, error))?;
        let state = repository.worktree_gedo_dir()?.join(STREAM_STATE_FILE_NAME);
        Ok(FileStreamStore {
            repository: repository.clone(),
            streams,
            state,
            worktree: repository.worktree_id(),
        })
    }

    fn directory(&self, name: &StreamName) -> PathBuf {
        self.streams.join(name.key())
    }

    fn record(&self, name: &StreamName) -> PathBuf {
        self.directory(name).join(STREAM_RECORD_FILE_NAME)
    }

    /// Points `reference` at `commit`, or deletes it.
    fn set_reference(&self, reference: &str, commit: Option<GitOid>) -> Result<(), StoreError> {
        let repo = self.repository.local();
        let name = gix::refs::FullName::try_from(reference)
            .map_err(|error| StoreError::backend("reference name", error))?;
        let change = match commit {
            Some(commit) => Change::Update {
                log: LogChange {
                    mode: RefLog::AndReference,
                    force_create_reflog: false,
                    message: "gedo stream".into(),
                },
                expected: PreviousValue::Any,
                new: Target::Object(to_gix_oid(&commit)?),
            },
            None => {
                let exists = repo
                    .try_find_reference(reference)
                    .map_err(|error| StoreError::backend("read reference", format!("{reference}: {error}")))?
                    .is_some();
                if !exists {
                    return Ok(());
                }
                Change::Delete {
                    expected: PreviousValue::Any,
                    log: RefLog::AndReference,
                }
            }
        };
        let edit = RefEdit {
            change,
            name,
            deref: false,
        };
        let signature = self.repository.signature()?;
        let mut time = gix::date::parse::TimeBuf::default();
        let wait =
            gix::lock::acquire::Fail::AfterDurationWithBackoff(Duration::from_millis(REF_LOCK_WAIT_MS));
        repo.refs
            .transaction()
            .prepare(Some(edit), wait, wait)
            .and_then(|transaction| transaction.commit(signature.to_ref(&mut time)))
            .map_err(|error| StoreError::backend("update reference", format!("{reference}: {error}")))?;
        Ok(())
    }
}

/// Writes `bytes` to a temporary file next to `path`, synced, and returns it.
fn write_temporary(path: &Path, bytes: &[u8]) -> Result<PathBuf, StoreError> {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!("{file_name}.tmp-{}-{}", std::process::id(), unique()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| StoreError::io("create temporary file", &temporary, error))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| StoreError::io("write temporary file", &temporary, error))?;
    Ok(temporary)
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Replaces `path` with `bytes` atomically.
fn replace_file(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let temporary = write_temporary(path, bytes)?;
    retry_sharing_violation(path, || fs::rename(&temporary, path)).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        StoreError::io("replace file", path, error)
    })?;
    if let Some(parent) = path.parent() {
        sync_directory(parent).map_err(|error| StoreError::io("sync directory", parent, error))?;
    }
    Ok(())
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
    match retry_sharing_violation(path, || fs::read(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(StoreError::io("read file", path, error)),
    }
}

impl StreamStore for FileStreamStore {
    type Log = FileWal;

    fn names(&self) -> Result<Vec<StreamName>, StoreError> {
        let entries = fs::read_dir(&self.streams)
            .map_err(|error| StoreError::io("list streams", &self.streams, error))?;
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| StoreError::io("list streams", &self.streams, error))?;
            let key = entry.file_name().to_string_lossy().into_owned();
            let Ok(name) = StreamName::from_key(&key) else {
                tracing::warn!(entry = %key, "ignoring an entry Gedo cannot have written in the streams directory");
                continue;
            };
            if entry.path().join(STREAM_RECORD_FILE_NAME).is_file() {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    fn read(&self, name: &StreamName) -> Result<Option<Vec<u8>>, StoreError> {
        read_optional(&self.record(name))
    }

    fn create(&self, name: &StreamName, record: &[u8]) -> Result<bool, StoreError> {
        let directory = self.directory(name);
        fs::create_dir_all(&directory)
            .map_err(|error| StoreError::io("create directory", &directory, error))?;
        let path = self.record(name);
        let temporary = write_temporary(&path, record)?;
        // A hard link fails when the record exists, so of two creations exactly one wins.
        let linked = fs::hard_link(&temporary, &path);
        let _ = fs::remove_file(&temporary);
        match linked {
            Ok(()) => {
                sync_directory(&directory)
                    .map_err(|error| StoreError::io("sync directory", &directory, error))?;
                Ok(true)
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(StoreError::io("create stream record", &path, error)),
        }
    }

    fn replace(&self, name: &StreamName, record: &[u8]) -> Result<(), StoreError> {
        let path = self.record(name);
        if !path.is_file() {
            return Err(StoreError::invalid_input(
                "replace a stream record",
                format!("stream {name} does not exist"),
            ));
        }
        replace_file(&path, record)
    }

    fn remove(&self, name: &StreamName) -> Result<(), StoreError> {
        self.pin(name, None)?;
        let directory = self.directory(name);
        match retry_sharing_violation(&directory, || fs::remove_dir_all(&directory)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StoreError::io("remove stream", &directory, error)),
        }
    }

    fn open_log(&self, name: &StreamName) -> Result<FileWal, StoreError> {
        let directory = self.directory(name);
        fs::create_dir_all(&directory)
            .map_err(|error| StoreError::io("create directory", &directory, error))?;
        FileWal::open(&directory)
    }

    fn pin(&self, name: &StreamName, commit: Option<GitOid>) -> Result<(), StoreError> {
        self.set_reference(&format!("{STREAM_PIN_PREFIX}{}", name.key()), commit)
    }

    fn read_state(&self) -> Result<Option<Vec<u8>>, StoreError> {
        read_optional(&self.state)
    }

    fn write_state(&self, state: &[u8], outside: Option<GitOid>) -> Result<(), StoreError> {
        self.set_reference(&format!("{WORKTREE_PIN_PREFIX}{}", self.worktree), outside)?;
        replace_file(&self.state, state)
    }
}
