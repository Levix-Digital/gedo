//! A [`StreamStore`] in memory: streams shared by every view made with
//! [`MemoryStreamStore::worktree`], a state per view, one writer per stream log.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::{MemoryWal, lock};
use crate::error::StoreError;
use crate::ids::GitOid;
use crate::ports::{LogSequence, OpenReport, StreamStore, WriteAheadLog};
use crate::stream::StreamName;

#[derive(Default)]
struct Shared {
    records: BTreeMap<StreamName, Vec<u8>>,
    logs: BTreeMap<StreamName, (Arc<Mutex<MemoryWal>>, Arc<AtomicBool>)>,
    pins: BTreeMap<StreamName, GitOid>,
    outside_pins: BTreeMap<String, GitOid>,
}

/// The streams of one repository in memory, seen from one working tree.
#[derive(Clone)]
pub struct MemoryStreamStore {
    shared: Arc<Mutex<Shared>>,
    worktree: String,
    state: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Default for MemoryStreamStore {
    fn default() -> Self {
        MemoryStreamStore::new()
    }
}

impl MemoryStreamStore {
    /// An empty store seen from the main working tree.
    pub fn new() -> Self {
        MemoryStreamStore {
            shared: Arc::default(),
            worktree: "main".into(),
            state: Arc::default(),
        }
    }

    /// The same streams seen from another working tree `id`, with a state of its own.
    pub fn worktree(&self, id: &str) -> Self {
        MemoryStreamStore {
            shared: Arc::clone(&self.shared),
            worktree: id.to_owned(),
            state: Arc::default(),
        }
    }

    /// The commit pinned for `name`.
    pub fn pin_of(&self, name: &StreamName) -> Option<GitOid> {
        lock(&self.shared).pins.get(name).copied()
    }

    /// The commit pinned for this working tree's outside work.
    pub fn outside_pin(&self) -> Option<GitOid> {
        lock(&self.shared).outside_pins.get(&self.worktree).copied()
    }
}

impl StreamStore for MemoryStreamStore {
    type Log = MemoryStreamLog;

    fn names(&self) -> Result<Vec<StreamName>, StoreError> {
        Ok(lock(&self.shared).records.keys().cloned().collect())
    }

    fn read(&self, name: &StreamName) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(lock(&self.shared).records.get(name).cloned())
    }

    fn create(&self, name: &StreamName, record: &[u8]) -> Result<bool, StoreError> {
        let mut shared = lock(&self.shared);
        if shared.records.contains_key(name) {
            return Ok(false);
        }
        shared.records.insert(name.clone(), record.to_vec());
        Ok(true)
    }

    fn replace(&self, name: &StreamName, record: &[u8]) -> Result<(), StoreError> {
        let mut shared = lock(&self.shared);
        match shared.records.get_mut(name) {
            Some(stored) => {
                *stored = record.to_vec();
                Ok(())
            }
            None => Err(StoreError::invalid_input(
                "replace a stream record",
                format!("stream {name} does not exist"),
            )),
        }
    }

    fn remove(&self, name: &StreamName) -> Result<(), StoreError> {
        let mut shared = lock(&self.shared);
        shared.records.remove(name);
        shared.logs.remove(name);
        shared.pins.remove(name);
        Ok(())
    }

    fn open_log(&self, name: &StreamName) -> Result<MemoryStreamLog, StoreError> {
        let mut shared = lock(&self.shared);
        let (wal, busy) = shared.logs.entry(name.clone()).or_default().clone();
        if busy.swap(true, Ordering::SeqCst) {
            return Err(StoreError::WalBusy {
                path: PathBuf::from(format!("memory/streams/{}/wal.lock", name.key())),
            });
        }
        let report = OpenReport {
            records: lock(&wal).read_all()?.len() as u64,
            torn_tail: None,
        };
        Ok(MemoryStreamLog { wal, busy, report })
    }

    fn pin(&self, name: &StreamName, commit: Option<GitOid>) -> Result<(), StoreError> {
        let mut shared = lock(&self.shared);
        match commit {
            Some(commit) => shared.pins.insert(name.clone(), commit),
            None => shared.pins.remove(name),
        };
        Ok(())
    }

    fn read_state(&self) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(lock(&self.state).clone())
    }

    fn write_state(&self, state: &[u8], outside: Option<GitOid>) -> Result<(), StoreError> {
        *lock(&self.state) = Some(state.to_vec());
        let mut shared = lock(&self.shared);
        match outside {
            Some(commit) => shared.outside_pins.insert(self.worktree.clone(), commit),
            None => shared.outside_pins.remove(&self.worktree),
        };
        Ok(())
    }
}

/// The log of one stream in a [`MemoryStreamStore`]; the stream's writer until dropped.
pub struct MemoryStreamLog {
    wal: Arc<Mutex<MemoryWal>>,
    busy: Arc<AtomicBool>,
    report: OpenReport,
}

impl Drop for MemoryStreamLog {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::SeqCst);
    }
}

impl WriteAheadLog for MemoryStreamLog {
    fn append(&mut self, payload: &[u8]) -> Result<LogSequence, StoreError> {
        let mut wal = lock(&self.wal);
        let sequence = wal.append(payload)?;
        wal.sync()?;
        Ok(sequence)
    }

    fn sync(&mut self) -> Result<(), StoreError> {
        lock(&self.wal).sync()
    }

    fn read_all(&self) -> Result<Vec<(LogSequence, Vec<u8>)>, StoreError> {
        lock(&self.wal).read_all()
    }

    fn truncate_through(&mut self, sequence: LogSequence) -> Result<(), StoreError> {
        lock(&self.wal).truncate_through(sequence)
    }

    fn open_report(&self) -> &OpenReport {
        &self.report
    }
}
