//! Port to the references under `refs/gedo/` and the remote mirrors under `refs/gedo-remote/`.

use crate::error::StoreError;
use crate::ids::{GedoRefName, GitOid};

/// The value a compare-and-swap expects to find.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expected {
    /// The reference must not exist.
    Absent,
    /// The reference must hold this value.
    Value(GitOid),
}

impl Expected {
    /// `Absent` for `None`, `Value` otherwise.
    pub fn from_current(current: Option<GitOid>) -> Self {
        current.map_or(Expected::Absent, Expected::Value)
    }
}

/// A compare-and-swap update of one reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefUpdate {
    /// The reference to update.
    pub name: GedoRefName,
    /// The value it must hold for the update to apply.
    pub expected: Expected,
    /// The new value.
    pub new: GitOid,
    /// Reference log message, `gedo: <operation>`.
    pub log_message: String,
}

/// Result of a compare-and-swap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefOutcome {
    /// The reference now holds the new value (or was deleted).
    Updated,
    /// The reference no longer held the expected value; nothing changed.
    Moved {
        /// Its current value (`None` when it does not exist).
        current: Option<GitOid>,
    },
}

/// Atomic references under `refs/gedo/`.
///
/// Only names under `refs/gedo/` can be written ([`GedoRefName`] cannot hold any other). Every
/// update is atomic and recorded in the reference log; two writers in different processes can
/// never both get `Updated` for the same expected value.
pub trait RefStore {
    /// The current value of a reference.
    fn read(&self, name: &GedoRefName) -> Result<Option<GitOid>, StoreError>;
    /// Every reference equal to `prefix` or below it, sorted by name.
    fn list(&self, prefix: &GedoRefName) -> Result<Vec<(GedoRefName, GitOid)>, StoreError>;
    /// The mirrors `git fetch` wrote under `refs/gedo-remote/<remote>/`, as names relative to
    /// that prefix (`tapes` for `refs/gedo-remote/origin/tapes`), sorted by name.
    fn list_remote(&self, remote: &str) -> Result<Vec<(String, GitOid)>, StoreError>;
    /// Sets a reference if it still holds the expected value.
    ///
    /// Creating a reference (`Absent`) that already holds exactly the new value is `Updated`.
    /// When other writers keep the reference locked past the adapter's lock wait, the result is
    /// `StoreError::Contended`: nothing was written, and the caller may read and try again.
    fn compare_and_swap(&self, update: &RefUpdate) -> Result<RefOutcome, StoreError>;
    /// Deletes a reference if it still holds `expected`.
    fn delete(&self, name: &GedoRefName, expected: GitOid) -> Result<RefOutcome, StoreError>;
}
