//! Collection retries are visible as `tracing` events.
//!
//! Alone in its own test binary: tracing caches whether a call site is enabled process-wide, so
//! tests running concurrently without a subscriber would disable the event for this one.

use std::sync::{Arc, Mutex};

use gedo_core::contract_tests::RacingRefs;
use gedo_core::memory::{MemoryObjectStore, MemoryRefStore};
use gedo_core::{Collection, EntryId, GedoRefName};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

/// Collects the `attempt` field of every event.
#[derive(Clone, Default)]
struct Attempts(Arc<Mutex<Vec<u64>>>);

impl Visit for Attempts {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "attempt" {
            self.0.lock().expect("not poisoned").push(value);
        }
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_u64(field, value.unsigned_abs());
    }
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}

impl Subscriber for Attempts {
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
fn each_retry_emits_an_event_with_its_attempt_number() {
    let attempts = Attempts::default();
    let objects = MemoryObjectStore::new();
    let refs = MemoryRefStore::new();
    let racing = RacingRefs::new(&objects, &refs, 2);
    tracing::subscriber::with_default(attempts.clone(), || {
        let collection = Collection::new(
            GedoRefName::new("refs/gedo/tapes").expect("valid"),
            &objects,
            &racing,
        );
        collection
            .put(
                &[(EntryId::new("abcd").expect("valid"), b"x".to_vec())],
                "gedo: put",
            )
            .expect("put");
    });
    assert_eq!(*attempts.0.lock().expect("not poisoned"), vec![1, 2]);
}
