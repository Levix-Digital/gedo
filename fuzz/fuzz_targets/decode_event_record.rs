//! Any bytes, decoded as each record of the event log: no panic, hang or out-of-memory, and every
//! accepted record satisfies its invariants and its id.
#![no_main]

use gedo_app::events::records::{decode_event, decode_index_entry, decode_tape, decode_wal};
use gedo_core::event::{EventId, TapeId};
use gedo_crdt::{event_id, tape_id};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(gedo_app::events::records::WalRecord::Event(event)) = decode_wal(bytes, "fuzz") {
        event.check().expect("a decoded event is valid");
        let id = event_id(&event);
        let stored = gedo_app::events::records::encode_event(&event).expect("encodes");
        assert_eq!(decode_event(&stored, &id).expect("decodes"), event);
    }
    let _ = decode_event(bytes, &EventId([0; 16]));
    let _ = decode_tape(bytes, &TapeId([0; 16]));
    let _ = decode_index_entry(bytes, "fuzz");
    // Accept the payload under its own id, so the checks after the id comparison are reached.
    if let Ok(envelope) = gedo_core::record::RecordEnvelope::decode(bytes, &[1], "fuzz") {
        if let Ok(event) = envelope.unwrap::<gedo_core::event::Event>("fuzz") {
            if let Ok(decoded) = decode_event(bytes, &event_id(&event)) {
                decoded.check().expect("a decoded event is valid");
            }
        }
        if let Ok(tape) = envelope.unwrap::<gedo_core::event::Tape>("fuzz") {
            if let Ok(decoded) = decode_tape(bytes, &tape_id(&tape)) {
                decoded.check().expect("a decoded tape is valid");
            }
        }
    }
});
