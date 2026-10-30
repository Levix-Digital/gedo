//! Re-attachment of tapes to rewritten commits, the retention report of detached tapes and the
//! filling of the event cache (FEAT-065, IMP-005, `specs/005-history-reattachment/research.md`).
//!
//! Re-attachment only adds commit index entries: events, tapes and existing entries never change,
//! and every entry is content-addressed, so a run interrupted at any point and run again writes
//! the same data, and machines join their entries by union (ADR-0014). The matching itself is
//! `gedo_crdt::reattach`; this module reads the candidates and writes the result.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use gedo_core::event::{Event, EventId, SemanticOp, TapeId};
use gedo_core::identity::AstNodeId;
use gedo_core::ids::GitOid;
use gedo_core::ports::{ObjectStore, Parser, RefStore, SymbolCache, WriteAheadLog};
use gedo_core::syntax::StructuralHash;
use gedo_crdt::{CandidateTape, Reattachment, match_commit, semantic_signature};

use super::records::{
    self, COMMIT_TAPES_COLLECTION, EVENTS_CACHE_NAMESPACE, EVENTS_COLLECTION, RETENTION_CACHE_NAMESPACE,
    RETENTION_KEY, RetentionRecord,
};
use super::{Context, EventError, EventLog};
use crate::index::IdentityIndex;

/// Days a detached tape is kept before `gedo gc` may remove it (technical specification).
pub const DEFAULT_GRACE_DAYS: u32 = 90;

/// Event records read and cached together by [`EventLog::fill_cache`].
const FILL_BATCH: usize = 1_000;

/// What re-attachment did for one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitReattachment {
    /// The commit.
    pub commit: GitOid,
    /// The tapes attached to it, sorted by id.
    pub attached: Vec<TapeId>,
    /// Whether the semantic signatures of the old and new commit were equal (no matching).
    pub fast_path: bool,
    /// Changes of the commit no attached event accounts for.
    pub unmatched: usize,
    /// Identities of entities created on the rewritten branch linked to the commit's.
    pub links: usize,
}

/// What a call to re-attachment did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReattachReport {
    /// Every commit processed, in processing order (generation, then id).
    pub commits: Vec<CommitReattachment>,
    /// Commits left as they were because they already had an index entry.
    pub skipped: Vec<GitOid>,
}

impl ReattachReport {
    /// Tapes attached by the call, over every commit.
    pub fn attached(&self) -> usize {
        self.commits.iter().map(|commit| commit.attached.len()).sum()
    }
}

/// A tape attached to no commit of the history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetachedTape {
    /// The tape.
    pub tape: TapeId,
    /// The commits it is indexed for, none of them reachable.
    pub commits: Vec<GitOid>,
    /// The number of its events.
    pub events: usize,
    /// When this machine first saw it detached, in Unix seconds.
    pub detached_since: u64,
    /// Whether it has been detached for the grace period and may be removed by `gedo gc`.
    pub eligible: bool,
}

/// What a call to [`EventLog::fill_cache`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheFill {
    /// Events in the collection.
    pub listed: usize,
    /// Events the cache already held.
    pub already_cached: usize,
    /// Events read and written to the cache.
    pub cached: usize,
    /// Batches the cache refused (logged; filters read those events from the collection).
    pub cache_failures: usize,
}

/// The commit index: the commits of every tape.
type IndexMap = BTreeMap<TapeId, Vec<GitOid>>;

impl<O: ObjectStore, R: RefStore, W: WriteAheadLog> EventLog<'_, O, R, W> {
    /// Re-attaches the commits that Git's `post-rewrite` hook reports as `(old, new)` pairs.
    ///
    /// For each new commit without an index entry, the tapes of its old commits are attached:
    /// all of them when the old and new commits make the same semantic changes (the fast path),
    /// otherwise those with at least one event the new commit's changes match. Every new commit
    /// must be indexed in `identities`; an old commit that is not only skips the fast path.
    pub fn reattach_rewritten<P, C>(
        &mut self,
        pairs: &[(GitOid, GitOid)],
        identities: &IdentityIndex<'_, O, P, C>,
    ) -> Result<ReattachReport, EventError>
    where
        P: Parser + Sync,
        C: SymbolCache,
    {
        let mut olds: BTreeMap<GitOid, BTreeSet<GitOid>> = BTreeMap::new();
        for (old, new) in pairs {
            olds.entry(*new).or_default().insert(*old);
        }
        let mut report = ReattachReport::default();
        let mut writes = Vec::new();
        for new in self.in_generation_order(olds.keys().copied())? {
            if !self.tapes_of(&new)?.is_empty() {
                report.skipped.push(new);
                continue;
            }
            let olds = &olds[&new];
            let mut tapes = BTreeSet::new();
            for old in olds {
                tapes.extend(self.tapes_of(old)?);
            }
            let changes = identities.changes_of(&new)?;
            let hashes = identities.result_hashes(&new)?;
            let fast_path = match olds.iter().next() {
                Some(old) if olds.len() == 1 && !tapes.is_empty() => {
                    match (identities.changes_of(old), identities.result_hashes(old)) {
                        (Ok(old_changes), Ok(old_hashes)) => {
                            semantic_signature(&old_changes.deltas, &old_changes.merges, |id| {
                                old_hashes.get(&id).copied()
                            }) == semantic_signature(&changes.deltas, &changes.merges, |id| {
                                hashes.get(&id).copied()
                            })
                        }
                        _ => false,
                    }
                }
                _ => false,
            };
            let outcome = if fast_path {
                Reattachment {
                    attached: tapes.iter().copied().collect(),
                    ..Reattachment::default()
                }
            } else {
                let candidates = self.candidates(&tapes, true)?;
                let ops = operations(&changes.deltas, &changes.merges);
                match_commit(
                    &ops,
                    |id| hashes.get(&id).copied(),
                    &candidates,
                    |id| identities.resolve(id),
                )
            };
            writes.extend(outcome.attached.iter().map(|tape| (new, *tape)));
            report.commits.push(self.log_commit(new, &outcome, fast_path));
        }
        self.write_index(&writes)?;
        tracing::info!(
            commits = report.commits.len(),
            attached = report.attached(),
            skipped = report.skipped.len(),
            "re-attachment done"
        );
        Ok(report)
    }

    /// Re-attaches `commits` that arrived without pairs (a fetch, a server-side squash or rebase
    /// merge): their candidates are the detached tapes, those indexed for no commit reachable
    /// from `tips`.
    ///
    /// A detached tape is attached when every one of its events is matched, superseded by a later
    /// matched event of the same entity, or has no net effect, and at least one is matched or
    /// superseded. Commits are processed in generation order; a tape attached to an ancestor in
    /// the same call is no candidate for its descendants, as if the calls had been separate.
    pub fn reattach_new<P, C>(
        &mut self,
        commits: &[GitOid],
        tips: &[GitOid],
        identities: &IdentityIndex<'_, O, P, C>,
    ) -> Result<ReattachReport, EventError>
    where
        P: Parser + Sync,
        C: SymbolCache,
    {
        let index = self.index_map()?;
        let detached: BTreeSet<TapeId> = self.detached_set(&index, tips)?.into_keys().collect();
        let mut report = ReattachReport::default();
        let mut writes = Vec::new();
        let mut attached_to: Vec<(GitOid, TapeId)> = Vec::new();
        let order = self.in_generation_order(commits.iter().copied())?;
        let all = self.candidates(&detached, false)?;
        for commit in order {
            if !self.tapes_of(&commit)?.is_empty() {
                report.skipped.push(commit);
                continue;
            }
            let ancestors = self.ancestors(&commit, attached_to.iter().map(|(commit, _)| *commit))?;
            let taken: BTreeSet<TapeId> = attached_to
                .iter()
                .filter(|(at, _)| ancestors.contains(at))
                .map(|(_, tape)| *tape)
                .collect();
            let candidates: Vec<CandidateTape> = all
                .iter()
                .filter(|candidate| !taken.contains(&candidate.tape))
                .cloned()
                .collect();
            let changes = identities.changes_of(&commit)?;
            let hashes = identities.result_hashes(&commit)?;
            let ops = operations(&changes.deltas, &changes.merges);
            let outcome = match_commit(
                &ops,
                |id| hashes.get(&id).copied(),
                &candidates,
                |id| identities.resolve(id),
            );
            for tape in &outcome.attached {
                writes.push((commit, *tape));
                attached_to.push((commit, *tape));
            }
            report.commits.push(self.log_commit(commit, &outcome, false));
        }
        self.write_index(&writes)?;
        tracing::info!(
            commits = report.commits.len(),
            attached = report.attached(),
            skipped = report.skipped.len(),
            "re-attachment done"
        );
        Ok(report)
    }

    /// The tapes indexed for no commit reachable from `tips`, with the time this machine first
    /// saw each detached (kept in the cache given with [`EventLog::with_cache`]) and whether the
    /// grace period has passed at `now` (Unix seconds). Nothing is removed.
    ///
    /// Without a cache, or with a record the cache refuses, every tape is dated `now`, so none is
    /// eligible: losing the record only restarts the period.
    pub fn detached(
        &mut self,
        tips: &[GitOid],
        now: u64,
        grace_days: u32,
    ) -> Result<Vec<DetachedTape>, EventError> {
        let index = self.index_map()?;
        let detached = self.detached_set(&index, tips)?;
        let previous = self.retention();
        let mut record = RetentionRecord::default();
        let mut report = Vec::with_capacity(detached.len());
        let grace = u64::from(grace_days).saturating_mul(86_400);
        for (tape, commits) in detached {
            let since = previous.detached.get(&tape).copied().unwrap_or(now).min(now);
            record.detached.insert(tape, since);
            let events = self.tape(&tape)?.events.len();
            report.push(DetachedTape {
                tape,
                commits,
                events,
                detached_since: since,
                eligible: now.saturating_sub(since) >= grace,
            });
        }
        if record != previous {
            self.store_retention(&record);
        }
        tracing::info!(
            detached = report.len(),
            eligible = report.iter().filter(|tape| tape.eligible).count(),
            grace_days,
            "detached tapes"
        );
        Ok(report)
    }

    /// Reads every stored event that the cache does not hold yet and keeps it there, so the first
    /// filter after new Gedo data arrives (a fetch, a merge of `refs/gedo/*`) reads no event
    /// object (IMP-005). Cache failures are logged and counted, never returned (ADR-0015).
    pub fn fill_cache(&self) -> Result<CacheFill, EventError> {
        let mut fill = CacheFill::default();
        let Some(cache) = self.cache else {
            return Ok(fill);
        };
        let entries = self
            .collection(EVENTS_COLLECTION)?
            .entries()
            .context(|| "list the events".into())?;
        fill.listed = entries.len();
        let mut batch: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for (entry, blob) in entries {
            let id = EventId::from_entry_name(entry.as_str()).ok_or_else(|| EventError::Store {
                context: "list the events".into(),
                source: gedo_core::StoreError::Corrupted {
                    path: format!("refs/gedo/{EVENTS_COLLECTION}"),
                    offset: None,
                    reason: format!("entry {entry} is not an event id"),
                },
            })?;
            if self.cached_event(&id).is_some() {
                fill.already_cached += 1;
                continue;
            }
            let bytes = self
                .objects
                .read_blob(&blob)
                .context(|| format!("read event {id}"))?;
            records::decode_event(&bytes, &id).context(|| format!("decode event {id}"))?;
            batch.push((id.0.to_vec(), bytes));
            if batch.len() == FILL_BATCH {
                put_batch(cache, &mut batch, &mut fill);
            }
        }
        put_batch(cache, &mut batch, &mut fill);
        tracing::info!(
            listed = fill.listed,
            cached = fill.cached,
            already_cached = fill.already_cached,
            "event cache filled"
        );
        Ok(fill)
    }

    /// The candidates of `tapes`, with their events.
    fn candidates(&self, tapes: &BTreeSet<TapeId>, paired: bool) -> Result<Vec<CandidateTape>, EventError> {
        tapes
            .iter()
            .map(|tape| {
                let ids = self.tape(tape)?.events;
                Ok(CandidateTape {
                    tape: *tape,
                    events: self.events(&ids)?,
                    paired,
                })
            })
            .collect()
    }

    /// Every entry of the commit index, by tape.
    fn index_map(&self) -> Result<IndexMap, EventError> {
        let entries = self
            .collection(COMMIT_TAPES_COLLECTION)?
            .entries()
            .context(|| "list the commit index".into())?;
        let mut index: IndexMap = BTreeMap::new();
        for entry in entries.keys() {
            let (commit, tape) =
                records::parse_index_entry(entry).context(|| "read the commit index".into())?;
            index.entry(tape).or_default().push(commit);
        }
        Ok(index)
    }

    /// The tapes of `index` whose commits are all unreachable from `tips`, with those commits.
    fn detached_set(&self, index: &IndexMap, tips: &[GitOid]) -> Result<IndexMap, EventError> {
        let mut unseen: HashSet<GitOid> = index.values().flatten().copied().collect();
        let mut seen: HashSet<GitOid> = HashSet::new();
        let mut reachable: HashSet<GitOid> = HashSet::new();
        let mut stack: Vec<GitOid> = tips.to_vec();
        while let Some(commit) = stack.pop() {
            if unseen.is_empty() {
                break;
            }
            if !seen.insert(commit) {
                continue;
            }
            if unseen.remove(&commit) {
                reachable.insert(commit);
            }
            let parents = self
                .objects
                .read_commit(&commit)
                .context(|| format!("read commit {commit}"))?
                .parents;
            stack.extend(parents);
        }
        Ok(index
            .iter()
            .filter(|(_, commits)| !commits.iter().any(|commit| reachable.contains(commit)))
            .map(|(tape, commits)| (*tape, commits.clone()))
            .collect())
    }

    /// Which of `candidates` are ancestors of `commit` (or the commit itself).
    fn ancestors(
        &mut self,
        commit: &GitOid,
        candidates: impl Iterator<Item = GitOid>,
    ) -> Result<HashSet<GitOid>, EventError> {
        let wanted: HashSet<GitOid> = candidates.collect();
        let mut found = HashSet::new();
        if wanted.is_empty() {
            return Ok(found);
        }
        let mut lowest = u64::MAX;
        for oid in &wanted {
            lowest = lowest.min(self.generation(oid)?);
        }
        let mut seen = HashSet::new();
        let mut stack = vec![*commit];
        while let Some(oid) = stack.pop() {
            if !seen.insert(oid) || self.generation(&oid)? < lowest {
                continue;
            }
            if wanted.contains(&oid) {
                found.insert(oid);
            }
            let parents = self
                .objects
                .read_commit(&oid)
                .context(|| format!("read commit {oid}"))?
                .parents;
            stack.extend(parents);
        }
        Ok(found)
    }

    /// `commits` sorted by generation, then id, without duplicates.
    fn in_generation_order(
        &mut self,
        commits: impl Iterator<Item = GitOid>,
    ) -> Result<Vec<GitOid>, EventError> {
        let mut keyed = Vec::new();
        for commit in commits.collect::<BTreeSet<_>>() {
            keyed.push((self.generation(&commit)?, commit));
        }
        keyed.sort();
        Ok(keyed.into_iter().map(|(_, commit)| commit).collect())
    }

    /// Writes the index entries of `(commit, tape)` pairs in one collection write, and removes
    /// the attached tapes from the retention record.
    fn write_index(&self, pairs: &[(GitOid, TapeId)]) -> Result<(), EventError> {
        if pairs.is_empty() {
            return Ok(());
        }
        let mut entries = Vec::with_capacity(pairs.len());
        for (commit, tape) in pairs {
            entries.push((
                records::index_entry(commit, tape).context(|| "name the index entry".into())?,
                records::encode_index_entry().context(|| "encode the index entry".into())?,
            ));
        }
        self.collection(COMMIT_TAPES_COLLECTION)?
            .put(&entries, &format!("gedo: re-attach {} tapes", pairs.len()))
            .context(|| "store the re-attached index entries".into())?;
        let mut record = self.retention();
        let before = record.detached.len();
        for (_, tape) in pairs {
            record.detached.remove(tape);
        }
        if record.detached.len() != before {
            self.store_retention(&record);
        }
        Ok(())
    }

    fn log_commit(&self, commit: GitOid, outcome: &Reattachment, fast_path: bool) -> CommitReattachment {
        if !outcome.attached.is_empty() {
            tracing::info!(%commit, tapes = outcome.attached.len(), fast_path, "tapes re-attached");
        }
        if !outcome.unmatched.is_empty() {
            tracing::debug!(%commit, changes = outcome.unmatched.len(), "changes not matched");
        }
        CommitReattachment {
            commit,
            attached: outcome.attached.clone(),
            fast_path,
            unmatched: outcome.unmatched.len(),
            links: outcome.links.len(),
        }
    }

    /// The retention record, empty when there is no cache or the record is refused.
    fn retention(&self) -> RetentionRecord {
        let Some(cache) = self.cache else {
            return RetentionRecord::default();
        };
        match cache.get(RETENTION_CACHE_NAMESPACE, RETENTION_KEY) {
            Ok(Some(bytes)) => records::decode_retention(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, "retention record refused");
                RetentionRecord::default()
            }),
            Ok(None) => RetentionRecord::default(),
            Err(error) => {
                tracing::warn!(%error, "retention record refused");
                RetentionRecord::default()
            }
        }
    }

    fn store_retention(&self, record: &RetentionRecord) {
        let Some(cache) = self.cache else { return };
        let written = records::encode_retention(record)
            .and_then(|bytes| cache.put(RETENTION_CACHE_NAMESPACE, &[(RETENTION_KEY.to_vec(), bytes)]));
        if let Err(error) = written {
            tracing::warn!(%error, "could not write the retention record");
        }
    }
}

/// A commit's changes as reconstruction lists them: deltas, then identity merges.
pub(super) fn operations(
    deltas: &[gedo_core::identity::SemanticDelta],
    merges: &[gedo_core::identity::IdentityMerged],
) -> Vec<SemanticOp> {
    deltas
        .iter()
        .cloned()
        .map(SemanticOp::Delta)
        .chain(merges.iter().map(|merge| SemanticOp::IdentityMerged {
            kept: merge.kept,
            merged: merge.merged,
        }))
        .collect()
}

/// The remainder of a commit with tapes: its reconstructed events that `outcome` does not
/// account for, then its identity links (research R8 of feature 005).
pub(super) fn remainder(reconstructed: Vec<Event>, changes: usize, outcome: &Reattachment) -> Vec<Event> {
    reconstructed
        .into_iter()
        .enumerate()
        .filter(|(position, _)| *position >= changes || !outcome.accounted.contains(position))
        .map(|(_, event)| event)
        .collect()
}

/// The result hashes of `ids` from `hashes`, for callers that look up by id.
pub(super) fn lookup(
    hashes: &BTreeMap<AstNodeId, StructuralHash>,
) -> impl Fn(AstNodeId) -> Option<StructuralHash> + '_ {
    move |id| hashes.get(&id).copied()
}

fn put_batch(cache: &dyn SymbolCache, batch: &mut Vec<(Vec<u8>, Vec<u8>)>, fill: &mut CacheFill) {
    if batch.is_empty() {
        return;
    }
    match cache.put(EVENTS_CACHE_NAMESPACE, batch) {
        Ok(()) => fill.cached += batch.len(),
        Err(error) => {
            fill.cache_failures += 1;
            tracing::warn!(%error, events = batch.len(), "could not write the event cache");
        }
    }
    batch.clear();
}
