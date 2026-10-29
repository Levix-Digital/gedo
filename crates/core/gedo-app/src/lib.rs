//! Gedo's use cases: the core logic that combines the ports of `gedo-core` with the algorithms of
//! `gedo-ast` and `gedo-crdt`.
//!
//! Like the rest of the core, this crate performs no I/O of its own and depends on no
//! infrastructure library (ADR-0001); callers pass in the adapters.
#![forbid(unsafe_code)]

pub mod events;
pub mod index;

pub use events::{CommitEvents, EventError, EventLog};
pub use index::{CommitChanges, IdentityIndex, IndexError, IndexReport};
