//! Gedo's core: the domain types, the ports through which the core reaches storage, and the pure
//! logic built on them.
//!
//! This crate performs no I/O and depends on no infrastructure library (ADR-0001); adapters such
//! as `gedo-store` implement its ports.
#![forbid(unsafe_code)]
