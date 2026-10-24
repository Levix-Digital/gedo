use crate::error::StoreError;
use crate::ports::{LogSequence, OpenReport, WriteAheadLog};

/// A write-ahead log in memory with simulated durability.
///
/// Appended records are volatile until [`MemoryWal::sync`] or [`MemoryWal::elapse_durability_window`]
/// (what the real log's flusher does after 100 ms); [`MemoryWal::crash`] keeps only durable records.
#[derive(Clone, Debug, Default)]
pub struct MemoryWal {
    records: Vec<Vec<u8>>,
    durable: usize,
    report: OpenReport,
}

impl MemoryWal {
    /// An empty log.
    pub fn new() -> Self {
        MemoryWal::default()
    }

    /// Makes every appended record durable, as the group commit does after the window.
    pub fn elapse_durability_window(&mut self) {
        self.durable = self.records.len();
    }

    /// The log as reopened after a crash: only durable records remain.
    pub fn crash(&self) -> MemoryWal {
        let records = self.records[..self.durable].to_vec();
        let report = OpenReport {
            records: records.len() as u64,
            torn_tail: None,
        };
        MemoryWal {
            durable: records.len(),
            records,
            report,
        }
    }
}

impl WriteAheadLog for MemoryWal {
    fn append(&mut self, payload: &[u8]) -> Result<LogSequence, StoreError> {
        self.records.push(payload.to_vec());
        Ok(LogSequence(self.records.len() as u64 - 1))
    }

    fn sync(&mut self) -> Result<(), StoreError> {
        self.durable = self.records.len();
        Ok(())
    }

    fn read_all(&self) -> Result<Vec<(LogSequence, Vec<u8>)>, StoreError> {
        Ok(self
            .records
            .iter()
            .enumerate()
            .map(|(index, record)| (LogSequence(index as u64), record.clone()))
            .collect())
    }

    fn truncate_through(&mut self, sequence: LogSequence) -> Result<(), StoreError> {
        let removed = usize::try_from(sequence.0)
            .ok()
            .and_then(|index| index.checked_add(1));
        let Some(removed) = removed.filter(|removed| *removed <= self.records.len()) else {
            return Err(StoreError::invalid_input(
                "truncate write-ahead log",
                format!(
                    "record {sequence} does not exist ({} records)",
                    self.records.len()
                ),
            ));
        };
        self.records.drain(..removed);
        // Truncation rewrites the file and syncs it, so what remains is durable.
        self.durable = self.records.len();
        Ok(())
    }

    fn open_report(&self) -> &OpenReport {
        &self.report
    }
}
