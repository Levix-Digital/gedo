//! A torn last record is dropped and reported; any other damage is reported with its offset and
//! never returned as data (FR-007, SC-004).

use std::path::Path;

use gedo_core::StoreError;
use gedo_core::ports::WriteAheadLog;
use gedo_store::FileWal;
use gedo_store::wal::{WAL_FILE_NAME, WAL_MAGIC};

const HEADER_LEN: usize = 10;
const FRAME_OVERHEAD: usize = 10;

fn payloads() -> Vec<Vec<u8>> {
    vec![
        b"first record".to_vec(),
        Vec::new(),
        b"third".to_vec(),
        vec![0xab; 300],
    ]
}

/// A closed log holding `payloads()`; returns its bytes and where each record starts.
fn write_log(directory: &Path) -> (Vec<u8>, Vec<usize>) {
    let mut wal = FileWal::open(directory).expect("opens");
    for payload in payloads() {
        wal.append(&payload).expect("append");
    }
    wal.sync().expect("sync");
    drop(wal);
    let bytes = std::fs::read(directory.join(WAL_FILE_NAME)).expect("log file");
    let mut starts = Vec::new();
    let mut offset = HEADER_LEN;
    for payload in payloads() {
        starts.push(offset);
        offset += FRAME_OVERHEAD + payload.len();
    }
    assert_eq!(offset, bytes.len(), "frame layout as documented");
    (bytes, starts)
}

fn reopen_with(directory: &Path, bytes: &[u8]) -> Result<FileWal, StoreError> {
    std::fs::write(directory.join(WAL_FILE_NAME), bytes).expect("write log");
    FileWal::open(directory)
}

fn records(wal: &FileWal) -> Vec<Vec<u8>> {
    wal.read_all()
        .expect("read")
        .into_iter()
        .map(|(_, payload)| payload)
        .collect()
}

#[test]
fn a_torn_last_record_is_dropped_at_every_byte_offset() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (bytes, starts) = write_log(directory.path());
    let last = *starts.last().expect("records");
    for cut in last..bytes.len() {
        let wal = reopen_with(directory.path(), &bytes[..cut]).expect("opens");
        let expected = &payloads()[..payloads().len() - 1];
        assert_eq!(records(&wal), expected, "cut at {cut}");
        let torn = if cut == last { None } else { Some(last as u64) };
        assert_eq!(wal.open_report().torn_tail, torn, "cut at {cut}");
        assert_eq!(wal.open_report().records, expected.len() as u64);
        drop(wal);
        let on_disk = std::fs::read(directory.path().join(WAL_FILE_NAME)).expect("log file");
        assert_eq!(
            on_disk.len(),
            last,
            "the torn record is truncated away (cut at {cut})"
        );
    }
}

#[test]
fn appends_after_a_torn_tail_continue_the_log() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (bytes, starts) = write_log(directory.path());
    let mut wal = reopen_with(directory.path(), &bytes[..starts[3] + 5]).expect("opens");
    assert_eq!(wal.append(b"after").expect("append").0, 3);
    wal.sync().expect("sync");
    drop(wal);
    let wal = FileWal::open(directory.path()).expect("opens");
    assert_eq!(wal.open_report().torn_tail, None);
    assert_eq!(records(&wal).last().map(Vec::as_slice), Some(&b"after"[..]));
}

#[test]
fn damage_inside_earlier_records_is_reported_with_its_offset() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (bytes, starts) = write_log(directory.path());
    // Every byte of every record but the last: length, checksum, version and payload.
    for (index, start) in starts.iter().enumerate().take(starts.len() - 1) {
        let end = starts[index + 1];
        for position in *start..end {
            let mut damaged = bytes.clone();
            damaged[position] ^= 0x5a;
            match reopen_with(directory.path(), &damaged) {
                Err(StoreError::Corrupted {
                    offset: Some(offset), ..
                }) => {
                    assert_eq!(
                        offset as usize, *start,
                        "flip at {position} reported at its record"
                    );
                }
                Err(other) => panic!("flip at {position}: unexpected error {other}"),
                Ok(wal) => panic!("flip at {position} was not detected: {:?}", records(&wal)),
            }
        }
    }
    // Nothing was truncated by the failed opens' damage detection.
    let wal = reopen_with(directory.path(), &bytes).expect("opens");
    assert_eq!(records(&wal), payloads());
}

#[test]
fn a_torn_header_starts_an_empty_log_and_a_foreign_file_is_refused() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (bytes, _) = write_log(directory.path());
    for cut in 0..HEADER_LEN {
        let wal = reopen_with(directory.path(), &bytes[..cut]).expect("opens");
        assert!(records(&wal).is_empty(), "cut at {cut}");
        assert_eq!(
            wal.open_report().torn_tail,
            None,
            "a file being created lost nothing"
        );
    }
    let mut foreign = bytes.clone();
    foreign[0] = b'X';
    assert!(matches!(
        reopen_with(directory.path(), &foreign),
        Err(StoreError::Corrupted { offset: Some(0), .. })
    ));
    let mut future = bytes.clone();
    future[WAL_MAGIC.len()] = 9;
    assert!(matches!(
        reopen_with(directory.path(), &future),
        Err(StoreError::UnsupportedFormat { found: 9, .. })
    ));
}

#[test]
fn records_of_an_unknown_version_are_refused() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (mut bytes, starts) = write_log(directory.path());
    // Rewrite the first record with version 7 and a matching checksum.
    let start = starts[0];
    let body_end = starts[1];
    bytes[start + 8] = 7;
    let checksum = crc32fast::hash(&bytes[start + 8..body_end]);
    bytes[start + 4..start + 8].copy_from_slice(&checksum.to_le_bytes());
    let wal = reopen_with(directory.path(), &bytes).expect("frames are intact");
    assert!(matches!(
        wal.read_all(),
        Err(StoreError::UnsupportedFormat { found: 7, .. })
    ));
}

#[test]
fn a_failing_write_never_leaves_a_partial_record() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut wal = FileWal::open(directory.path()).expect("opens");
    wal.append(b"kept").expect("append");
    for limit in [0, 3, 9, 12] {
        wal.inject_write_failure_after(limit);
        assert!(
            matches!(wal.append(b"lost record"), Err(StoreError::Io { .. })),
            "limit {limit}"
        );
        assert_eq!(records(&wal), vec![b"kept".to_vec()], "limit {limit}");
    }
    assert_eq!(wal.append(b"next").expect("append").0, 1);
    drop(wal);
    let wal = FileWal::open(directory.path()).expect("opens");
    assert_eq!(wal.open_report().torn_tail, None);
    assert_eq!(records(&wal), vec![b"kept".to_vec(), b"next".to_vec()]);
}
