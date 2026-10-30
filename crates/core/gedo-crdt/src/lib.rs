//! Gedo's event layer: the pure rules that turn semantic changes into events
//! (`specs/004-event-model/research.md`).
//!
//! Event and tape ids derived from content ([`id`]), clocks built on commit generations
//! ([`clock`]), author fingerprints ([`author`]), the text form of labels and the recording
//! context ([`labels`]), the events of a commit made without Gedo ([`reconstruct`]) and event
//! filters ([`filter`]), re-attachment after rewrites ([`reattach`]) and the tree CRDT that
//! converges concurrent changes ([`tree`]), and the semantic lanes of a publish ([`lanes`]) with
//! the refactor version they split off ([`split`]). Streams and the sequencer join this crate in
//! later features.
//!
//! Everything here is pure: no I/O, deterministic on every platform and input order (ADR-0001).
#![forbid(unsafe_code)]

pub mod author;
pub mod clock;
pub mod filter;
pub mod id;
pub mod labels;
pub mod lanes;
pub mod reattach;
pub mod reconstruct;
pub mod split;
pub mod tree;

pub use author::fingerprint;
pub use clock::{generation, live_clock, sort_events};
pub use filter::{AuthorKindFilter, EventFilter};
pub use id::{event_id, tape_id};
pub use labels::{RecordingContext, format_labels, parse_labels};
pub use lanes::{
    FileVersion, FileVersions, FollowUps, Lane, LanePlan, LanedChange, classify, event_lane, part_of,
};
pub use reattach::{
    CandidateTape, Reattachment, SemanticSignature, link_merges, match_commit, semantic_signature,
};
pub use reconstruct::{ReconstructError, reconstructed_events};
pub use split::{BuildError, RefactorFile, build, refactor_message};
pub use tree::{
    Conflict, ConflictKind, ConflictValue, Convergence, EntityState, Part, PartValue, Place, SideError,
    SideState, TreeError, TreeState, converge, converge_with, fold,
};
