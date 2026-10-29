//! The stored forms of events, tapes, commit index entries and write-ahead log records
//! (`specs/004-event-model/data-model.md`).
//!
//! Everything read from `refs/gedo/*` may come from another machine: decoding checks the size
//! limit and the format, the record's invariants and that its content matches the id it is stored
//! under, and never panics.

use gedo_core::StoreError;
use gedo_core::event::{Event, EventId, Tape, TapeId};
use gedo_core::ids::{EntryId, GitOid};
use gedo_core::record::RecordEnvelope;
use gedo_crdt::{event_id, tape_id};
use serde::{Deserialize, Serialize};

/// The collection of events, by event id.
pub const EVENTS_COLLECTION: &str = "events";
/// The collection of tapes, by tape id.
pub const TAPES_COLLECTION: &str = "tapes";
/// The commit index: one entry per commit and tape.
pub const COMMIT_TAPES_COLLECTION: &str = "commit-tapes";

/// Format of event records.
pub const EVENT_FORMAT: u16 = 1;
/// Format of tape records.
pub const TAPE_FORMAT: u16 = 1;
/// Format of commit index entries.
pub const INDEX_FORMAT: u16 = 1;
/// Format of write-ahead log records.
pub const WAL_FORMAT: u16 = 1;

/// A record of the write-ahead log. Later features append their variants at the end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalRecord {
    /// A live event not published yet.
    Event(Event),
}

/// The entry of an event in its collection.
pub fn event_entry(id: &EventId) -> Result<EntryId, StoreError> {
    Ok(EntryId::new(&id.entry_name())?)
}

/// The entry of a tape in its collection.
pub fn tape_entry(id: &TapeId) -> Result<EntryId, StoreError> {
    Ok(EntryId::new(&id.entry_name())?)
}

/// The commit index entry of `tape` for `commit`.
pub fn index_entry(commit: &GitOid, tape: &TapeId) -> Result<EntryId, StoreError> {
    Ok(EntryId::new(&format!(
        "{}{}",
        commit.to_hex(),
        tape.entry_name()
    ))?)
}

/// The tape named by an index entry of `commit`.
pub fn tape_of_index_entry(commit: &GitOid, entry: &EntryId) -> Result<TapeId, StoreError> {
    entry
        .as_str()
        .strip_prefix(commit.to_hex().as_str())
        .and_then(TapeId::from_entry_name)
        .ok_or_else(|| StoreError::Corrupted {
            path: format!("refs/gedo/{COMMIT_TAPES_COLLECTION}"),
            offset: None,
            reason: format!("entry {entry} is not a tape of commit {commit}"),
        })
}

/// Encodes an event record.
pub fn encode_event(event: &Event) -> Result<Vec<u8>, StoreError> {
    RecordEnvelope::wrap(EVENT_FORMAT, event)?.encode()
}

/// Decodes the event stored under `id`.
pub fn decode_event(bytes: &[u8], id: &EventId) -> Result<Event, StoreError> {
    let source = format!("event {id}");
    let event: Event = RecordEnvelope::decode(bytes, &[EVENT_FORMAT], &source)?.unwrap(&source)?;
    event.check().map_err(|reason| corrupted(&source, reason))?;
    if event_id(&event) != *id {
        return Err(corrupted(&source, "content does not match the id".into()));
    }
    Ok(event)
}

/// Encodes a tape record.
pub fn encode_tape(tape: &Tape) -> Result<Vec<u8>, StoreError> {
    RecordEnvelope::wrap(TAPE_FORMAT, tape)?.encode()
}

/// Decodes the tape stored under `id`.
pub fn decode_tape(bytes: &[u8], id: &TapeId) -> Result<Tape, StoreError> {
    let source = format!("tape {id}");
    let tape: Tape = RecordEnvelope::decode(bytes, &[TAPE_FORMAT], &source)?.unwrap(&source)?;
    tape.check().map_err(|reason| corrupted(&source, reason))?;
    if tape_id(&tape) != *id {
        return Err(corrupted(&source, "content does not match the id".into()));
    }
    Ok(tape)
}

/// Encodes the blob of a commit index entry.
pub fn encode_index_entry() -> Result<Vec<u8>, StoreError> {
    RecordEnvelope {
        format: INDEX_FORMAT,
        payload: Vec::new(),
    }
    .encode()
}

/// Checks the blob of a commit index entry.
pub fn decode_index_entry(bytes: &[u8], source: &str) -> Result<(), StoreError> {
    let envelope = RecordEnvelope::decode(bytes, &[INDEX_FORMAT], source)?;
    if envelope.payload.is_empty() {
        Ok(())
    } else {
        Err(corrupted(source, "an index entry has a payload".into()))
    }
}

/// Encodes a write-ahead log record.
pub fn encode_wal(record: &WalRecord) -> Result<Vec<u8>, StoreError> {
    RecordEnvelope::wrap(WAL_FORMAT, record)?.encode()
}

/// Decodes a write-ahead log record.
pub fn decode_wal(bytes: &[u8], source: &str) -> Result<WalRecord, StoreError> {
    let record: WalRecord = RecordEnvelope::decode(bytes, &[WAL_FORMAT], source)?.unwrap(source)?;
    let WalRecord::Event(event) = &record;
    event.check().map_err(|reason| corrupted(source, reason))?;
    Ok(record)
}

fn corrupted(source: &str, reason: String) -> StoreError {
    StoreError::Corrupted {
        path: source.to_owned(),
        offset: None,
        reason,
    }
}
