use std::collections::BTreeMap;
use std::sync::Mutex;

use super::lock;
use crate::error::StoreError;
use crate::ids::{GedoRefName, GitOid, validate_remote_name};
use crate::ports::{Expected, RefOutcome, RefStore, RefUpdate};

#[derive(Debug, Default)]
struct State {
    refs: BTreeMap<GedoRefName, GitOid>,
    reflog: BTreeMap<GedoRefName, Vec<(GitOid, String)>>,
    remotes: BTreeMap<String, BTreeMap<String, GitOid>>,
}

/// References in memory with atomic compare-and-swap and a reference log.
#[derive(Debug, Default)]
pub struct MemoryRefStore {
    state: Mutex<State>,
}

impl MemoryRefStore {
    /// An empty reference store.
    pub fn new() -> Self {
        MemoryRefStore::default()
    }

    /// The values a reference took, oldest first, with their log messages.
    pub fn reflog(&self, name: &GedoRefName) -> Vec<(GitOid, String)> {
        lock(&self.state).reflog.get(name).cloned().unwrap_or_default()
    }

    /// Sets a remote mirror, as `git fetch` would under `refs/gedo-remote/<remote>/<name>`.
    pub fn set_remote(&self, remote: &str, name: &str, oid: GitOid) {
        lock(&self.state)
            .remotes
            .entry(remote.to_owned())
            .or_default()
            .insert(name.to_owned(), oid);
    }
}

/// Git stores references as files, so `a` and `a/b` cannot both exist.
fn conflicting_name(refs: &BTreeMap<GedoRefName, GitOid>, name: &GedoRefName) -> Option<GedoRefName> {
    refs.keys()
        .find(|existing| *existing != name && (existing.starts_with(name) || name.starts_with(existing)))
        .cloned()
}

impl RefStore for MemoryRefStore {
    fn read(&self, name: &GedoRefName) -> Result<Option<GitOid>, StoreError> {
        Ok(lock(&self.state).refs.get(name).copied())
    }

    fn list(&self, prefix: &GedoRefName) -> Result<Vec<(GedoRefName, GitOid)>, StoreError> {
        let state = lock(&self.state);
        Ok(state
            .refs
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, oid)| (name.clone(), *oid))
            .collect())
    }

    fn list_remote(&self, remote: &str) -> Result<Vec<(String, GitOid)>, StoreError> {
        validate_remote_name(remote)?;
        let state = lock(&self.state);
        Ok(state
            .remotes
            .get(remote)
            .map(|mirrors| mirrors.iter().map(|(name, oid)| (name.clone(), *oid)).collect())
            .unwrap_or_default())
    }

    fn compare_and_swap(&self, update: &RefUpdate) -> Result<RefOutcome, StoreError> {
        let mut state = lock(&self.state);
        let current = state.refs.get(&update.name).copied();
        let matches = match update.expected {
            Expected::Absent => current.is_none() || current == Some(update.new),
            Expected::Value(expected) => current == Some(expected),
        };
        if !matches {
            return Ok(RefOutcome::Moved { current });
        }
        if current.is_none()
            && let Some(existing) = conflicting_name(&state.refs, &update.name)
        {
            return Err(StoreError::backend(
                "update reference",
                format!("{} conflicts with existing reference {existing}", update.name),
            ));
        }
        if current != Some(update.new) {
            state.refs.insert(update.name.clone(), update.new);
            state
                .reflog
                .entry(update.name.clone())
                .or_default()
                .push((update.new, update.log_message.clone()));
        }
        Ok(RefOutcome::Updated)
    }

    fn delete(&self, name: &GedoRefName, expected: GitOid) -> Result<RefOutcome, StoreError> {
        let mut state = lock(&self.state);
        let current = state.refs.get(name).copied();
        if current != Some(expected) {
            return Ok(RefOutcome::Moved { current });
        }
        state.refs.remove(name);
        state.reflog.remove(name);
        Ok(RefOutcome::Updated)
    }
}
