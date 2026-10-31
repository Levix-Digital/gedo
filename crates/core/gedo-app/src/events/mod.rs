//! The event log: live events recorded into the write-ahead log, published as tapes indexed by
//! commit, and the events of commits made without Gedo reconstructed on demand (FEAT-063, FEAT-064,
//! `specs/004-event-model/research.md` R9 to R12). Tapes are re-attached to rewritten commits
//! by [`reattach`] (FEAT-065).
//!
//! Events, tapes and index entries are content-addressed entries of three collections, so a
//! publish is idempotent and data written on several machines joins without conflict.
//! Reconstructed events are never stored: they are a pure function of the commit.
//!
//! With a symbol cache ([`EventLog::with_cache`]), stored event records are also kept in its
//! `events` namespace, so reads and filters do not open one object per event (BUG-010). The
//! cache is local derived data: a missing, unreadable or refused entry is read from the
//! collection again, and a cache failure never fails an operation. Commit generations are kept
//! in its `generations` namespace the same way ([`generations`], IMP-006).

pub mod converge;
pub mod generations;
pub mod reattach;
pub mod records;
pub mod split;

use std::collections::{BTreeSet, HashMap};

use gedo_core::event::{Event, EventId, MAX_TAPE_EVENTS, SemanticOp, Tape, TapeId};
use gedo_core::ids::{GedoRefName, GitOid};
use gedo_core::ports::{LogSequence, ObjectStore, RefStore, SymbolCache, WriteAheadLog};
use gedo_core::syntax::StructuralHash;
use gedo_core::{Collection, LabelError, StoreError};
use gedo_crdt::clock::order_key;
use gedo_crdt::{
    CandidateTape, EventFilter, ReconstructError, RecordingContext, event_id, link_merges, live_clock,
    match_commit, reconstructed_events, sort_events, tape_id,
};

use crate::index::{IdentityIndex, IndexError};
use records::{
    COMMIT_TAPES_COLLECTION, EVENTS_CACHE_NAMESPACE, EVENTS_COLLECTION, TAPES_COLLECTION, WalRecord,
};

/// Why an event log operation failed.
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    /// Reading or writing storage failed.
    #[error("{context}: {source}")]
    Store {
        /// What was being done.
        context: String,
        /// The storage error.
        #[source]
        source: StoreError,
    },

    /// The commit's identities are not available.
    #[error(transparent)]
    Index(#[from] IndexError),

    /// A recording context or a filter was refused.
    #[error(transparent)]
    Label(#[from] LabelError),

    /// An event of a reconstructed commit could not be built.
    #[error("reconstruct the events of commit {commit}: {source}")]
    Reconstruct {
        /// The commit.
        commit: GitOid,
        /// What was missing.
        #[source]
        source: ReconstructError,
    },

    /// The log holds more events than one tape may list; nothing was published.
    #[error("{events} events waiting; a tape lists at most {limit}, publish more often")]
    TapeTooLarge {
        /// Events in the log.
        events: usize,
        /// The limit.
        limit: usize,
    },

    /// The events of a side cannot be folded over the base (an event about an unknown entity).
    #[error("converge side {side}: {source}")]
    Tree {
        /// The side's position in the input.
        side: usize,
        /// What went wrong.
        #[source]
        source: gedo_crdt::TreeError,
    },

    /// A stored record named another record that is not in the local data (a partial fetch).
    #[error("{what} {id} is missing from the local Gedo data; fetch the remote's refs/gedo again")]
    Missing {
        /// `tape` or `event`.
        what: &'static str,
        /// The missing id.
        id: String,
    },
}

trait Context<T> {
    fn context(self, context: impl FnOnce() -> String) -> Result<T, EventError>;
}

impl<T> Context<T> for Result<T, StoreError> {
    fn context(self, context: impl FnOnce() -> String) -> Result<T, EventError> {
        self.map_err(|source| EventError::Store {
            context: context(),
            source,
        })
    }
}

/// The events of one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitEvents {
    /// The commit has tapes (published live or re-attached).
    Published {
        /// Each tape with its events in recording order.
        tapes: Vec<(TapeId, Vec<Event>)>,
        /// The reconstructed events of the commit's changes that no tape event accounts for, then
        /// the identity links of entities created on a rewritten branch (empty for a commit its
        /// tapes cover).
        remainder: Vec<Event>,
    },
    /// No tape: the events reconstructed from the commit's changes.
    Reconstructed(Vec<Event>),
}

impl CommitEvents {
    /// Every event, tape after tape, then the remainder.
    pub fn events(&self) -> Vec<&Event> {
        match self {
            CommitEvents::Published { tapes, remainder } => tapes
                .iter()
                .flat_map(|(_, events)| events)
                .chain(remainder)
                .collect(),
            CommitEvents::Reconstructed(events) => events.iter().collect(),
        }
    }
}

/// The events of the log by id, with the sequence of the last record read.
type PendingEvents = (LogSequence, Vec<(EventId, Event)>);

/// The event log of one repository.
pub struct EventLog<'a, O, R, W> {
    objects: &'a O,
    refs: &'a R,
    wal: &'a mut W,
    cache: Option<&'a dyn SymbolCache>,
    /// The clock of the last event in the log, to continue its sequence.
    last: Option<gedo_core::event::Clock>,
    generations: HashMap<GitOid, u64>,
}

impl<'a, O: ObjectStore, R: RefStore, W: WriteAheadLog> EventLog<'a, O, R, W> {
    /// Opens the log over the repository's stores and write-ahead log.
    pub fn open(objects: &'a O, refs: &'a R, wal: &'a mut W) -> Result<Self, EventError> {
        let mut log = EventLog {
            objects,
            refs,
            wal,
            cache: None,
            last: None,
            generations: HashMap::new(),
        };
        log.last = log.pending()?.last().map(|(_, event)| event.clock);
        Ok(log)
    }

    /// Keeps stored event records in `cache` too, and reads them from it first.
    pub fn with_cache(mut self, cache: &'a dyn SymbolCache) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Records live events of `context` on top of `base` (none: an empty history) into the
    /// write-ahead log, in order, and returns their ids. `ops` pairs each operation with the
    /// entity's structural hash after it (`None` for a tombstone and an identity merge).
    pub fn record(
        &mut self,
        context: &RecordingContext,
        base: Option<GitOid>,
        ops: &[(SemanticOp, Option<StructuralHash>)],
    ) -> Result<Vec<EventId>, EventError> {
        let base_generation = base.map(|base| self.generation(&base)).transpose()?;
        let author = context.author();
        let mut events = Vec::with_capacity(ops.len());
        let mut last = self.last;
        for (operation, result_hash) in ops {
            let generation = live_clock(base_generation, 0).generation;
            let sequence = match last {
                Some(clock) if clock.generation == generation => {
                    clock.sequence.checked_add(1).ok_or(EventError::TapeTooLarge {
                        events: u32::MAX as usize,
                        limit: MAX_TAPE_EVENTS,
                    })?
                }
                _ => 0,
            };
            let event = Event {
                clock: live_clock(base_generation, sequence),
                target: operation.target(),
                operation: operation.clone(),
                result_hash: *result_hash,
                author: author.clone(),
                labels: context.labels().clone(),
                origin: gedo_core::event::Origin::Live,
            };
            event.check().map_err(|reason| EventError::Store {
                context: "record an event".into(),
                source: StoreError::invalid_input("record an event", reason),
            })?;
            last = Some(event.clock);
            events.push(event);
        }
        let mut ids = Vec::with_capacity(events.len());
        for event in events {
            let bytes =
                records::encode_wal(&WalRecord::Event(event.clone())).context(|| "encode an event".into())?;
            self.wal
                .append(&bytes)
                .context(|| "append an event to the log".into())?;
            self.last = Some(event.clock);
            ids.push(event_id(&event));
        }
        tracing::debug!(events = ids.len(), "events recorded");
        Ok(ids)
    }

    /// Publishes every event of the log as one tape of `commit`: stores the events, the tape and
    /// the index entry, then removes them from the log. Returns `None` when the log is empty.
    ///
    /// Idempotent and crash-safe: every entry is content-addressed, and the log is truncated only
    /// after the index entry is written. Events appended after the log was read stay in it.
    pub fn publish(&mut self, commit: GitOid) -> Result<Option<TapeId>, EventError> {
        let Some((last_sequence, events)) = self.pending_unique()? else {
            return Ok(None);
        };
        let tapes = self.store_tapes(&[(commit, events)], last_sequence)?;
        Ok(tapes.into_iter().next().flatten())
    }

    /// The events of the log deduplicated by id, in recording order, with the sequence of the last
    /// record read; `None` when the log is empty.
    fn pending_unique(&self) -> Result<Option<PendingEvents>, EventError> {
        let pending = self.pending()?;
        let Some((last_sequence, _)) = pending.last() else {
            return Ok(None);
        };
        let last_sequence = *last_sequence;
        let mut seen = BTreeSet::new();
        let events = pending
            .into_iter()
            .map(|(_, event)| (event_id(&event), event))
            .filter(|(id, _)| seen.insert(*id))
            .collect();
        Ok(Some((last_sequence, events)))
    }

    /// Stores one tape per commit (none for a commit without events): every event, then every
    /// tape, then every index entry, then truncates the log through `last_sequence` (ADR-0014).
    /// Returns the tape of each commit, in order.
    fn store_tapes(
        &mut self,
        groups: &[(GitOid, Vec<(EventId, Event)>)],
        last_sequence: LogSequence,
    ) -> Result<Vec<Option<TapeId>>, EventError> {
        for (_, events) in groups {
            if events.len() > MAX_TAPE_EVENTS {
                tracing::warn!(
                    events = events.len(),
                    limit = MAX_TAPE_EVENTS,
                    "publish refused: tape too large"
                );
                return Err(EventError::TapeTooLarge {
                    events: events.len(),
                    limit: MAX_TAPE_EVENTS,
                });
            }
        }
        let tapes: Vec<Option<(TapeId, Tape)>> = groups
            .iter()
            .map(|(_, events)| {
                (!events.is_empty()).then(|| {
                    let tape = Tape {
                        events: events.iter().map(|(id, _)| *id).collect(),
                    };
                    (tape_id(&tape), tape)
                })
            })
            .collect();
        let message = match groups
            .iter()
            .zip(&tapes)
            .find_map(|((commit, _), tape)| tape.as_ref().map(|(id, _)| (commit, id)))
        {
            Some((commit, tape)) if groups.len() == 1 => format!("gedo: publish {tape} for {commit}"),
            Some((commit, tape)) => format!("gedo: publish {tape} and more for {commit} and more"),
            None => String::new(),
        };

        let mut entries = Vec::new();
        let mut cached = Vec::new();
        for (id, event) in groups.iter().flat_map(|(_, events)| events) {
            let bytes = records::encode_event(event).context(|| format!("encode event {id}"))?;
            entries.push((
                records::event_entry(id).context(|| "name an event".into())?,
                bytes.clone(),
            ));
            cached.push((*id, bytes));
        }
        if !entries.is_empty() {
            self.collection(EVENTS_COLLECTION)?
                .put(&entries, &message)
                .context(|| "store the events".into())?;
        }
        let mut tape_entries = Vec::new();
        let mut index_entries = Vec::new();
        for ((commit, _), tape) in groups.iter().zip(&tapes) {
            let Some((id, tape)) = tape else { continue };
            tape_entries.push((
                records::tape_entry(id).context(|| "name the tape".into())?,
                records::encode_tape(tape).context(|| "encode the tape".into())?,
            ));
            index_entries.push((
                records::index_entry(commit, id).context(|| "name the index entry".into())?,
                records::encode_index_entry().context(|| "encode the index entry".into())?,
            ));
        }
        if !tape_entries.is_empty() {
            self.collection(TAPES_COLLECTION)?
                .put(&tape_entries, &message)
                .context(|| "store the tape".into())?;
            self.collection(COMMIT_TAPES_COLLECTION)?
                .put(&index_entries, &message)
                .context(|| "store the index entry".into())?;
        }
        self.cache_records(cached);
        self.wal
            .truncate_through(last_sequence)
            .context(|| "remove the published events from the log".into())?;
        self.last = self.pending()?.last().map(|(_, event)| event.clock);
        for ((commit, events), tape) in groups.iter().zip(&tapes) {
            if let Some((tape, _)) = tape {
                tracing::info!(%commit, %tape, events = events.len(), "events published");
            }
        }
        Ok(tapes.into_iter().map(|tape| tape.map(|(id, _)| id)).collect())
    }

    /// The tapes published for `commit`, by id.
    pub fn tapes_of(&self, commit: &GitOid) -> Result<Vec<TapeId>, EventError> {
        let collection = self.collection(COMMIT_TAPES_COLLECTION)?;
        let entries = collection
            .entries_with_prefix(&commit.to_hex())
            .context(|| format!("list the tapes of {commit}"))?;
        let mut tapes = Vec::with_capacity(entries.len());
        for (entry, blob) in entries {
            let bytes = self
                .objects
                .read_blob(&blob)
                .context(|| format!("read index entry {entry}"))?;
            records::decode_index_entry(&bytes, &format!("index entry {entry}"))
                .context(|| "decode an index entry".into())?;
            tapes
                .push(records::tape_of_index_entry(commit, &entry).context(|| "read an index entry".into())?);
        }
        Ok(tapes)
    }

    /// The tape `id`.
    pub fn tape(&self, id: &TapeId) -> Result<Tape, EventError> {
        let entry = records::tape_entry(id).context(|| "name the tape".into())?;
        let bytes = self
            .collection(TAPES_COLLECTION)?
            .get(&entry)
            .context(|| format!("read tape {id}"))?
            .ok_or_else(|| EventError::Missing {
                what: "tape",
                id: id.to_string(),
            })?;
        records::decode_tape(&bytes, id).context(|| format!("decode tape {id}"))
    }

    /// The event `id`.
    pub fn event(&self, id: &EventId) -> Result<Event, EventError> {
        Ok(self.events(std::slice::from_ref(id))?.remove(0))
    }

    /// The events `ids`, in the same order.
    pub fn events(&self, ids: &[EventId]) -> Result<Vec<Event>, EventError> {
        let collection = self.collection(EVENTS_COLLECTION)?;
        let mut found = Vec::with_capacity(ids.len());
        let mut read = Vec::new();
        for id in ids {
            if let Some(event) = self.cached_event(id) {
                found.push(event);
                continue;
            }
            let entry = records::event_entry(id).context(|| "name the event".into())?;
            let bytes = collection
                .get(&entry)
                .context(|| format!("read event {id}"))?
                .ok_or_else(|| EventError::Missing {
                    what: "event",
                    id: id.to_string(),
                })?;
            found.push(records::decode_event(&bytes, id).context(|| format!("decode event {id}"))?);
            read.push((*id, bytes));
        }
        self.cache_records(read);
        Ok(found)
    }

    /// The events of `commit`: its tapes with the remainder of its changes they do not account
    /// for, or, when it has none, the events reconstructed from its changes in `identities`. The
    /// commit must be indexed in `identities` either way.
    pub fn events_of_commit<P, C>(
        &mut self,
        commit: &GitOid,
        identities: &IdentityIndex<'_, O, P, C>,
    ) -> Result<CommitEvents, EventError>
    where
        P: gedo_core::ports::Parser + Sync,
        C: gedo_core::ports::SymbolCache,
    {
        let tapes = self.tapes_of(commit)?;
        let changes = identities.changes_of(commit)?;
        let hashes = identities.result_hashes(commit)?;
        if tapes.is_empty() {
            let events = self.reconstruct(commit, &changes.deltas, &changes.merges, &hashes)?;
            tracing::debug!(%commit, events = events.len(), "events reconstructed");
            return Ok(CommitEvents::Reconstructed(events));
        }
        let mut published = Vec::with_capacity(tapes.len());
        for id in tapes {
            let tape = self.tape(&id)?;
            published.push((id, self.events(&tape.events)?));
        }
        let candidates: Vec<CandidateTape> = published
            .iter()
            .map(|(tape, events)| CandidateTape {
                tape: *tape,
                events: events.clone(),
                paired: true,
            })
            .collect();
        let ops = reattach::operations(&changes.deltas, &changes.merges);
        let outcome = match_commit(&ops, reattach::lookup(&hashes), &candidates, |id| {
            identities.resolve(id)
        });
        let remainder = if outcome.unmatched.is_empty() && outcome.links.is_empty() {
            Vec::new()
        } else {
            let mut merges = changes.merges.clone();
            merges.extend(link_merges(&outcome.links, *commit));
            let reconstructed = self.reconstruct(commit, &changes.deltas, &merges, &hashes)?;
            reattach::remainder(reconstructed, ops.len(), &outcome)
        };
        Ok(CommitEvents::Published {
            tapes: published,
            remainder,
        })
    }

    /// The events reconstructed for `commit` from its changes.
    fn reconstruct(
        &mut self,
        commit: &GitOid,
        deltas: &[gedo_core::identity::SemanticDelta],
        merges: &[gedo_core::identity::IdentityMerged],
        hashes: &std::collections::BTreeMap<gedo_core::identity::AstNodeId, StructuralHash>,
    ) -> Result<Vec<Event>, EventError> {
        let author = self
            .objects
            .read_commit(commit)
            .context(|| format!("read commit {commit}"))?
            .author_email;
        let generation = self.generation(commit)?;
        reconstructed_events(generation, &author, deltas, merges, |id| hashes.get(&id).copied()).map_err(
            |source| EventError::Reconstruct {
                commit: *commit,
                source,
            },
        )
    }

    /// Every stored event that matches `filter`, in the total order of events.
    pub fn filter(&self, filter: &EventFilter) -> Result<Vec<Event>, EventError> {
        let collection = self.collection(EVENTS_COLLECTION)?;
        let mut found = Vec::new();
        let mut read = Vec::new();
        for (entry, blob) in collection.entries().context(|| "list the events".into())? {
            let id = EventId::from_entry_name(entry.as_str()).ok_or_else(|| EventError::Store {
                context: "list the events".into(),
                source: StoreError::Corrupted {
                    path: format!("refs/gedo/{EVENTS_COLLECTION}"),
                    offset: None,
                    reason: format!("entry {entry} is not an event id"),
                },
            })?;
            let event = match self.cached_event(&id) {
                Some(event) => event,
                None => {
                    let bytes = self
                        .objects
                        .read_blob(&blob)
                        .context(|| format!("read event {id}"))?;
                    let event =
                        records::decode_event(&bytes, &id).context(|| format!("decode event {id}"))?;
                    read.push((id, bytes));
                    event
                }
            };
            if filter.matches(&event) {
                found.push(event);
            }
        }
        self.cache_records(read);
        sort_events(&mut found);
        Ok(found)
    }

    /// The events waiting in the log, in recording order.
    pub fn pending(&self) -> Result<Vec<(LogSequence, Event)>, EventError> {
        let records = self.wal.read_all().context(|| "read the log".into())?;
        records
            .into_iter()
            .map(|(sequence, bytes)| {
                let WalRecord::Event(event) = records::decode_wal(&bytes, &format!("log record {sequence}"))
                    .context(|| "decode the log".into())?;
                Ok((sequence, event))
            })
            .collect()
    }

    /// The event `id` from the cache, when it holds a valid record of it.
    fn cached_event(&self, id: &EventId) -> Option<Event> {
        let cache = self.cache?;
        let bytes = match cache.get(EVENTS_CACHE_NAMESPACE, &id.0) {
            Ok(bytes) => bytes?,
            Err(error) => {
                tracing::warn!(%id, %error, "could not read the event cache");
                return None;
            }
        };
        match records::decode_event(&bytes, id) {
            Ok(event) => Some(event),
            Err(error) => {
                tracing::warn!(%id, %error, "cached event refused; reading it from the collection");
                None
            }
        }
    }

    /// Keeps stored event records in the cache, in one transaction.
    fn cache_records(&self, records: Vec<(EventId, Vec<u8>)>) {
        let Some(cache) = self.cache else { return };
        if records.is_empty() {
            return;
        }
        let entries: Vec<(Vec<u8>, Vec<u8>)> = records
            .into_iter()
            .map(|(id, bytes)| (id.0.to_vec(), bytes))
            .collect();
        if let Err(error) = cache.put(EVENTS_CACHE_NAMESPACE, &entries) {
            tracing::warn!(%error, events = entries.len(), "could not write the event cache");
        }
    }

    fn collection(&self, name: &str) -> Result<Collection<'a, O, R>, EventError> {
        let name = GedoRefName::collection(name)
            .map_err(StoreError::from)
            .context(|| format!("name the {name} collection"))?;
        Ok(Collection::new(name, self.objects, self.refs))
    }
}

/// The key of the total order of events, for callers that merge event lists.
pub fn event_order(event: &Event) -> (gedo_core::event::Clock, EventId) {
    order_key(event)
}
