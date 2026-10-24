//! Gedo's core: the domain types, the ports through which the core reaches storage, and the pure
//! logic built on them.
//!
//! This crate performs no I/O and depends on no infrastructure library (ADR-0001); adapters such
//! as `gedo-store` implement its ports.
#![forbid(unsafe_code)]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
pub mod error;
pub mod ids;
#[cfg(feature = "memory")]
pub mod memory;
pub mod ports;
pub mod record;

pub use error::StoreError;
pub use ids::{EntryId, GedoRefName, GitOid, ObjectHash};
