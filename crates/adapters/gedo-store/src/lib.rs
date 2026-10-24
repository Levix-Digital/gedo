//! Gedo's persistence adapter: implements the `gedo-core` ports over the repository's Git object
//! database, `refs/gedo/*` and the local runtime directory `.git/gedo/`.
#![forbid(unsafe_code)]
