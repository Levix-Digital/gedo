//! Ids derived from content (research R2, R8).
//!
//! An id is the BLAKE3 key derivation of the postcard encoding of the value, truncated to 16 bytes
//! with the version and variant bits of a UUID version 8, as `AstNodeId` is built (ADR-0007).

use gedo_core::event::{Event, EventId, Tape, TapeId};
use serde::Serialize;

/// Key-derivation context of event ids, format 1. Never change it: every id would change.
const EVENT_CONTEXT_V1: &str = "gedo event id v1";
/// Key-derivation context of tape ids, format 1. Never change it: every id would change.
const TAPE_CONTEXT_V1: &str = "gedo tape id v1";

/// The id of an event: equal content gives equal ids on every machine.
pub fn event_id(event: &Event) -> EventId {
    EventId(derive(EVENT_CONTEXT_V1, event))
}

/// The id of a tape.
pub fn tape_id(tape: &Tape) -> TapeId {
    TapeId(derive(TAPE_CONTEXT_V1, tape))
}

fn derive<T: Serialize>(context: &str, value: &T) -> [u8; 16] {
    let encoded = postcard::to_allocvec(value)
        .unwrap_or_else(|_| unreachable!("events and tapes hold only encodable fields"));
    let digest = blake3::derive_key(context, &encoded);
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // RFC 9562: version 8 in the high nibble of byte 6, variant `10` in the top bits of byte 8.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes
}
