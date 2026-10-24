//! Waiting for the log, dropping a torn record and rebuilding the cache are visible as `tracing`
//! events.
//!
//! One test in its own binary: tracing caches whether a call site is enabled process-wide, so
//! concurrent tests without a subscriber would disable the events for this one.

use std::sync::{Arc, Mutex};

use gedo_core::contract_tests::FixedSource;
use gedo_core::ports::WriteAheadLog;
use gedo_store::{FileWal, RedbCache};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

#[derive(Clone, Default)]
struct Messages(Arc<Mutex<Vec<String>>>);

impl Visit for Messages {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.lock().expect("not poisoned").push(format!("{value:?}"));
        }
    }
}

impl Subscriber for Messages {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _span: &Id, _values: &Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
    fn event(&self, event: &Event<'_>) {
        event.record(&mut self.clone());
    }
    fn enter(&self, _span: &Id) {}
    fn exit(&self, _span: &Id) {}
}

#[test]
fn lock_waits_torn_records_and_cache_rebuilds_are_logged() {
    let messages = Messages::default();
    let directory = tempfile::tempdir().expect("temporary directory");
    tracing::subscriber::with_default(messages.clone(), || {
        let mut wal = FileWal::open(directory.path()).expect("opens");
        wal.append(b"record").expect("append");
        wal.sync().expect("sync");
        let path = directory.path().to_path_buf();
        let waiter = std::thread::spawn({
            let messages = messages.clone();
            move || tracing::subscriber::with_default(messages, || FileWal::open(&path).map(drop))
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        let log = wal.path().to_path_buf();
        drop(wal);
        waiter
            .join()
            .expect("thread")
            .expect("opens after the first writer");

        let bytes = std::fs::read(&log).expect("log");
        std::fs::write(&log, &bytes[..bytes.len() - 2]).expect("tear the last record");
        FileWal::open(directory.path()).expect("opens");

        RedbCache::open(directory.path(), &FixedSource(vec![])).expect("opens");
    });
    let messages = messages.0.lock().expect("not poisoned").clone();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("held by another process")),
        "{messages:?}"
    );
    assert!(
        messages.iter().any(|message| message.contains("torn record")),
        "{messages:?}"
    );
}
