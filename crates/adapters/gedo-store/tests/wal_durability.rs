//! Group commit within 100 ms, immediate `sync`, and a single writer (FR-008, FR-009).

use std::process::Command;
use std::time::{Duration, Instant};

use gedo_core::StoreError;
use gedo_core::ports::WriteAheadLog;
use gedo_core::ports::wal::{DURABILITY_WINDOW_MS, WRITER_WAIT_MS};
use gedo_store::FileWal;

const CRASH_VARIABLE: &str = "GEDO_WAL_CRASH_DIRECTORY";

#[test]
fn appended_records_become_durable_within_the_window() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut wal = FileWal::open(directory.path()).expect("opens");
    let appended = Instant::now();
    wal.append(b"record").expect("append");
    assert!(wal.has_unsynced_records());
    while wal.has_unsynced_records() {
        assert!(
            appended.elapsed() < Duration::from_millis(DURABILITY_WINDOW_MS),
            "not synced within the window"
        );
        std::thread::sleep(Duration::from_millis(2));
    }

    // A second handle sees the record on disk.
    let on_disk = std::fs::read(wal.path()).expect("log file");
    assert!(on_disk.ends_with(b"record"));
}

#[test]
fn sync_is_immediate() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut wal = FileWal::open(directory.path()).expect("opens");
    wal.append(b"record").expect("append");
    wal.sync().expect("sync");
    assert!(!wal.has_unsynced_records());
}

#[test]
fn a_second_writer_waits_then_gets_busy() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let first = FileWal::open(directory.path()).expect("opens");
    let started = Instant::now();
    let error = FileWal::open(directory.path()).expect_err("busy");
    let waited = started.elapsed();
    assert!(matches!(error, StoreError::WalBusy { .. }), "{error}");
    assert!(
        waited >= Duration::from_millis(WRITER_WAIT_MS),
        "waited {waited:?}"
    );
    assert!(
        waited < Duration::from_millis(WRITER_WAIT_MS + 1_000),
        "waited {waited:?}"
    );

    drop(first);
    FileWal::open(directory.path()).expect("opens once the first writer is gone");
}

#[test]
fn a_waiting_writer_gets_the_log_when_the_first_one_leaves() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut first = FileWal::open(directory.path()).expect("opens");
    first.append(b"from the first").expect("append");
    let path = directory.path().to_path_buf();
    let second = std::thread::spawn(move || FileWal::open(&path).map(|wal| wal.read_all()));
    std::thread::sleep(Duration::from_millis(200));
    drop(first);
    let records = second.join().expect("thread").expect("opens").expect("reads");
    assert_eq!(records.len(), 1);
}

/// Child side of the crash test: opens the log, appends, syncs and aborts without cleanup.
#[test]
fn crash_writer_process() {
    let Ok(directory) = std::env::var(CRASH_VARIABLE) else {
        return;
    };
    let mut wal = FileWal::open(std::path::Path::new(&directory)).expect("opens");
    wal.append(b"before the crash").expect("append");
    wal.sync().expect("sync");
    std::process::abort();
}

#[test]
fn a_crashed_writer_leaves_no_stale_lock() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let status = Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "crash_writer_process", "--test-threads=1"])
        .env(CRASH_VARIABLE, directory.path())
        .status()
        .expect("child runs");
    assert!(!status.success(), "the child aborted");

    let started = Instant::now();
    let wal = FileWal::open(directory.path()).expect("the lock died with the process");
    assert!(started.elapsed() < Duration::from_millis(WRITER_WAIT_MS));
    let records = wal.read_all().expect("read");
    assert_eq!(records.len(), 1, "the synced record survived");
}

#[cfg(windows)]
#[test]
fn a_log_held_without_sharing_is_retried_then_fails_with_context() {
    use std::os::windows::fs::OpenOptionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    drop(FileWal::open(directory.path()).expect("creates the log"));
    let log = directory.path().join(gedo_store::wal::WAL_FILE_NAME);
    let holder = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&log)
        .expect("held");
    let started = Instant::now();
    let error = FileWal::open(directory.path()).expect_err("cannot open a held log");
    assert!(started.elapsed() >= Duration::from_millis(gedo_store::fs::SHARING_RETRY_MS));
    assert!(
        matches!(&error, StoreError::Io { path, .. } if path == &log),
        "{error}"
    );
    drop(holder);
    FileWal::open(directory.path()).expect("opens once released");
}
