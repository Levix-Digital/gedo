//! Commit generations (ADR-0014: 1 for a root, one more than the highest parent), kept in the
//! symbol cache's `generations` namespace so each is computed once per machine (IMP-006,
//! `specs/008-bounded-detached-walk/research.md` R4).
//!
//! The namespace is local derived data: a missing record is computed from the history, a record
//! is used only when it equals one more than the generations of its commit's parents, and a cache
//! failure is only a warning (ADR-0015).

use std::collections::HashMap;

use gedo_core::ids::GitOid;
use gedo_core::ports::{ObjectStore, RefStore, WriteAheadLog};
use gedo_crdt::generation;

use super::records::{GENERATION_FORMAT, GENERATIONS_CACHE_NAMESPACE};
use super::{Context, EventError, EventLog};

/// Generation records written to the cache in one transaction: each transaction is a durable
/// commit of the cache, so a first walk of a long history writes few of them.
const WRITE_BATCH: usize = 100_000;

/// The record of `generation`: the format byte, then 8 little-endian bytes.
pub fn encode_generation(generation: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(9);
    bytes.push(GENERATION_FORMAT);
    bytes.extend_from_slice(&generation.to_le_bytes());
    bytes
}

/// The generation in `bytes`, or `None` when the record is not one this build writes.
pub fn decode_generation(bytes: &[u8]) -> Option<u64> {
    match bytes {
        [GENERATION_FORMAT, rest @ ..] => {
            let value = u64::from_le_bytes(rest.try_into().ok()?);
            (value > 0).then_some(value)
        }
        _ => None,
    }
}

impl<O: ObjectStore, R: RefStore, W: WriteAheadLog> EventLog<'_, O, R, W> {
    /// The generation of `commit`, reading the cache before the history and walking only the
    /// ancestors whose generation is not known yet; every generation computed is kept in the
    /// cache.
    pub fn generation(&mut self, commit: &GitOid) -> Result<u64, EventError> {
        if let Some(known) = self.generations.get(commit) {
            return Ok(*known);
        }
        let mut computed = Vec::new();
        let mut stack = vec![(*commit, false)];
        let mut parents_of: HashMap<GitOid, Vec<GitOid>> = HashMap::new();
        while let Some((oid, parents_done)) = stack.pop() {
            if self.generations.contains_key(&oid) {
                continue;
            }
            if parents_done {
                let parents = parents_of.remove(&oid).unwrap_or_default();
                let known: Vec<u64> = parents
                    .iter()
                    .filter_map(|parent| self.generations.get(parent).copied())
                    .collect();
                let value = generation(&known);
                self.generations.insert(oid, value);
                computed.push((oid, value));
                continue;
            }
            let parents = self
                .objects
                .read_commit(&oid)
                .context(|| format!("read commit {oid}"))?
                .parents;
            if let Some(value) = self.checked_cached_generation(&oid, &parents) {
                self.generations.insert(oid, value);
                self.checked_parents.insert(oid, parents);
                continue;
            }
            stack.push((oid, true));
            stack.extend(parents.iter().map(|parent| (*parent, false)));
            parents_of.insert(oid, parents);
        }
        self.store_generations(&computed);
        Ok(self.generations.get(commit).copied().unwrap_or(1))
    }

    /// The parents of `commit`, from the commits read to check a cached generation or from the
    /// object store.
    pub(super) fn parents(&mut self, commit: &GitOid) -> Result<Vec<GitOid>, EventError> {
        if let Some(parents) = self.checked_parents.remove(commit) {
            return Ok(parents);
        }
        Ok(self
            .objects
            .read_commit(commit)
            .context(|| format!("read commit {commit}"))?
            .parents)
    }

    /// The cached generation of `commit` when it equals one more than the generations of its
    /// `parents` known in memory or in the cache (1 for a root); a record that disagrees is
    /// refused with a warning.
    fn checked_cached_generation(&self, commit: &GitOid, parents: &[GitOid]) -> Option<u64> {
        let cached = self.cached_generation(commit)?;
        let mut known = Vec::with_capacity(parents.len());
        for parent in parents {
            known.push(
                self.generations
                    .get(parent)
                    .copied()
                    .or_else(|| self.cached_generation(parent))?,
            );
        }
        let expected = generation(&known);
        if cached == expected {
            Some(cached)
        } else {
            tracing::warn!(%commit, cached, expected, "generation record refused");
            None
        }
    }

    /// The generation record of `commit` in the cache, if any and well formed.
    fn cached_generation(&self, commit: &GitOid) -> Option<u64> {
        let cache = self.cache?;
        match cache.get(GENERATIONS_CACHE_NAMESPACE, commit.as_bytes()) {
            Ok(Some(bytes)) => {
                let decoded = decode_generation(&bytes);
                if decoded.is_none() {
                    tracing::warn!(%commit, "generation record refused");
                }
                decoded
            }
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%commit, %error, "could not read the generation cache");
                None
            }
        }
    }

    /// Keeps `generations` in the cache, in batches; a failure is only a warning.
    fn store_generations(&self, generations: &[(GitOid, u64)]) {
        let Some(cache) = self.cache else { return };
        for batch in generations.chunks(WRITE_BATCH) {
            let entries: Vec<(Vec<u8>, Vec<u8>)> = batch
                .iter()
                .map(|(commit, value)| (commit.as_bytes().to_vec(), encode_generation(*value)))
                .collect();
            if let Err(error) = cache.put(GENERATIONS_CACHE_NAMESPACE, &entries) {
                tracing::warn!(%error, generations = entries.len(), "could not write generations");
                return;
            }
        }
    }
}
