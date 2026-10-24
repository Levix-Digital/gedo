//! Gedo's persistence adapter: implements the `gedo-core` ports over the repository's Git object
//! database, `refs/gedo/*` and the local runtime directory `.git/gedo/`.
#![forbid(unsafe_code)]

mod convert;
pub mod objects;
pub mod refs;
pub mod repository;

pub use objects::GitObjectStore;
pub use refs::GitRefStore;
pub use repository::{GitRepository, Identity};
