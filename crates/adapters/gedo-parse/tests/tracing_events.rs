//! Refused files and files with syntax errors are visible as `tracing` events.
//!
//! One test in its own binary: tracing caches whether a call site is enabled process-wide, so
//! concurrent tests without a subscriber would disable the events for this one.

use std::sync::{Arc, Mutex};

use gedo_core::ports::Parser;
use gedo_parse::{MAX_SOURCE_BYTES, TreeSitterParser};
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
fn refusals_and_syntax_errors_are_logged() {
    let messages = Messages::default();
    tracing::subscriber::with_default(messages.clone(), || {
        let parser = TreeSitterParser::new();
        let too_large = vec![b' '; MAX_SOURCE_BYTES as usize + 1];
        assert!(parser.parse("src/lib.rs", &too_large).is_err());
        assert!(parser.parse("README.md", b"# Gedo").is_err());
        assert!(parser.parse("src/lib.rs", b"fn broken( {}").is_ok());
    });
    let logged = messages.0.lock().expect("not poisoned").join("\n");
    assert!(logged.contains("file refused"), "{logged}");
    assert!(logged.contains("no supported language"), "{logged}");
    assert!(logged.contains("file parsed with syntax errors"), "{logged}");
}
