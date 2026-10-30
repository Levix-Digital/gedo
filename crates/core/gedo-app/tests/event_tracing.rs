//! Publishing, refusing a publish, re-attaching, reporting detached tapes, filling the event cache
//! and converging sides are visible as `tracing` events.
//!
//! One test in its own binary: tracing caches whether a call site is enabled process-wide, so
//! concurrent tests without a subscriber would disable the events for this one.

mod support;

use std::sync::{Arc, Mutex};

use gedo_app::{DEFAULT_GRACE_DAYS, EventLog, IdentityIndex, Side};
use gedo_core::event::{Labels, MAX_TAPE_EVENTS, SemanticOp};
use gedo_core::identity::{AstNodeId, SemanticDelta};
use gedo_core::memory::{MemoryRefStore, MemoryWal};
use gedo_core::syntax::StructuralHash;
use gedo_crdt::RecordingContext;
use support::{History, files};
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

fn body(after: u8) -> (SemanticOp, Option<StructuralHash>) {
    (
        SemanticOp::Delta(SemanticDelta::BodyModified {
            id: AstNodeId([1; 16]),
            before: StructuralHash([0; 32]),
            after: StructuralHash([after; 32]),
        }),
        Some(StructuralHash([after; 32])),
    )
}

#[test]
fn publishing_and_refusals_are_traced() {
    let messages = Messages::default();
    let history = History::default();
    let base = history.commit(&[], &files(&[("src/lib.rs", "pub fn f() {}\n")]));
    let refs = MemoryRefStore::new();
    let person = RecordingContext::person("levi@example.com", Labels::new()).expect("valid");
    tracing::subscriber::with_default(messages.clone(), || {
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
        log.record(&person, Some(base), &[body(1)]).expect("recorded");
        log.publish(base).expect("published");

        let ops: Vec<_> = (0..=MAX_TAPE_EVENTS).map(|n| body((n % 251) as u8)).collect();
        log.record(&person, Some(base), &ops).expect("recorded");
        assert!(log.publish(base).is_err());
    });
    // An amend of the message: the new commit makes the same changes as the published one.
    let amended = history.commit_with_message(&[], &files(&[("src/lib.rs", "pub fn f() {}\n")]), "amended");
    let identities = support::cache();
    let mut index = IdentityIndex::open(&history.objects, &history.parser, &identities).expect("opens");
    index.index_history(&[base, amended]).expect("indexed");
    let events = support::cache();
    tracing::subscriber::with_default(messages.clone(), || {
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&history.objects, &refs, &mut wal)
            .expect("opens")
            .with_cache(&events);
        let report = log
            .reattach_rewritten(&[(base, amended)], &index)
            .expect("re-attached");
        assert_eq!(report.attached(), 1);
        log.detached(&[amended], 0, DEFAULT_GRACE_DAYS).expect("report");
        log.fill_cache().expect("filled");
        log.converge(&base, &[Side::commit(base)], &index)
            .expect("converged");
    });
    let messages = messages.0.lock().expect("not poisoned").clone();
    assert!(
        messages.iter().any(|message| message == "events published"),
        "{messages:?}"
    );
    assert!(
        messages.iter().any(|message| message.contains("tape too large")),
        "{messages:?}"
    );
    for expected in [
        "tapes re-attached",
        "re-attachment done",
        "detached tapes",
        "event cache filled",
        "sides converged",
    ] {
        assert!(
            messages.iter().any(|message| message == expected),
            "{expected}: {messages:?}"
        );
    }
}
