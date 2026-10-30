//! Gedo's event layer: the pure rules that turn semantic changes into events
//! (`specs/004-event-model/research.md`).
//!
//! Event and tape ids derived from content ([`id`]), clocks built on commit generations
//! ([`clock`]), author fingerprints ([`author`]), the text form of labels and the recording
//! context ([`labels`]), the events of a commit made without Gedo ([`reconstruct`]) and event
//! filters ([`filter`]). The tree CRDT, lanes, streams and the sequencer join this crate in later
//! features.
//!
//! Everything here is pure: no I/O, deterministic on every platform and input order (ADR-0001).
#![forbid(unsafe_code)]

pub mod author;
pub mod clock;
pub mod filter;
pub mod id;
pub mod labels;
pub mod reattach;
pub mod reconstruct;

pub use author::fingerprint;
pub use clock::{generation, live_clock, sort_events};
pub use filter::{AuthorKindFilter, EventFilter};
pub use id::{event_id, tape_id};
pub use labels::{RecordingContext, format_labels, parse_labels};
pub use reattach::{
    CandidateTape, Reattachment, SemanticSignature, link_merges, match_commit, semantic_signature,
};
pub use reconstruct::{ReconstructError, reconstructed_events};
