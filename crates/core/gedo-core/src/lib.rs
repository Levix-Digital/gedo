//! Gedo's core: the domain types, the ports through which the core reaches storage and parsers,
//! and the pure logic built on them.
//!
//! This crate performs no I/O and depends on no infrastructure library (ADR-0001); adapters such
//! as `gedo-store` implement its ports.
#![forbid(unsafe_code)]

pub mod collection;
#[cfg(feature = "contract-tests")]
pub mod contract_tests;
pub mod error;
pub mod event;
pub mod identity;
pub mod ids;
#[cfg(feature = "memory")]
pub mod memory;
pub mod ports;
pub mod record;
pub mod syntax;

pub use collection::{Collection, MergeOutcome};
pub use error::StoreError;
pub use event::{
    AgentId, Author, AuthorFingerprint, AuthorKind, Clock, Event, EventId, LabelError, LabelKey, LabelValue,
    Labels, Origin, SemanticOp, Tape, TapeId,
};
pub use identity::{
    AnchoredNode, AstNodeId, BlobAstMap, IdentityMerged, Location, MatchFormat, SemanticDelta,
};
pub use ids::{EntryId, GedoRefName, GitOid, ObjectHash};
