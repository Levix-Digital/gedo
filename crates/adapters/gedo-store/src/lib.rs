//! Gedo's persistence adapter: implements the `gedo-core` ports over the repository's Git object
//! database, `refs/gedo/*` and the local runtime directory `.git/gedo/`.
#![forbid(unsafe_code)]

pub mod cache;
mod convert;
pub mod fs;
pub mod objects;
pub mod refs;
pub mod remote;
pub mod repository;
pub mod wal;

pub use cache::RedbCache;
pub use objects::GitObjectStore;
pub use refs::GitRefStore;
pub use repository::{GitRepository, Identity};
pub use wal::FileWal;
