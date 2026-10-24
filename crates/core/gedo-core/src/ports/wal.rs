//! Port to the local write-ahead log (`.git/gedo/wal.log`).

use std::fmt;

use crate::error::StoreError;

/// Time after which an appended record is durable without an explicit `sync` (group commit).
pub const DURABILITY_WINDOW_MS: u64 = 100;
/// How long a second writer waits for the log before getting `WalBusy`.
pub const WRITER_WAIT_MS: u64 = 2_000;

/// Position of a record in the log: zero-based, counted since the last truncation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LogSequence(pub u64);

impl fmt::Display for LogSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// What opening the log found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenReport {
    /// Complete records found.
    pub records: u64,
    /// Offset of a torn last record that was dropped, if any.
    pub torn_tail: Option<u64>,
}

/// An append-only log of opaque records owned by one writer at a time.
///
/// Records come back in append order. A record older than [`DURABILITY_WINDOW_MS`] is durable;
/// `sync` makes every appended record durable before it returns. A torn last record is dropped on
/// open and reported; other damage is a `Corrupted` error with its offset.
pub trait WriteAheadLog {
    /// Appends one record.
    fn append(&mut self, payload: &[u8]) -> Result<LogSequence, StoreError>;
    /// Makes every appended record durable now.
    fn sync(&mut self) -> Result<(), StoreError>;
    /// Every record, in append order.
    fn read_all(&self) -> Result<Vec<(LogSequence, Vec<u8>)>, StoreError>;
    /// Removes every record up to and including `sequence`; later records are kept and renumbered
    /// from zero.
    fn truncate_through(&mut self, sequence: LogSequence) -> Result<(), StoreError>;
    /// What the last open found.
    fn open_report(&self) -> &OpenReport;
}
