//! The event log: live events recorded into the write-ahead log, published as tapes indexed by
//! commit, and the events of commits made without Gedo reconstructed on demand (FEAT-063, FEAT-064,
//! `specs/004-event-model/research.md` R9 to R12).
//!
//! Events, tapes and index entries are content-addressed entries of three collections, so a
//! publish is idempotent and data written on several machines joins without conflict.
//! Reconstructed events are never stored: they are a pure function of the commit.

pub mod records;

use std::collections::{BTreeSet, HashMap};

use gedo_core::event::{Event, EventId, MAX_TAPE_EVENTS, SemanticOp, Tape, TapeId};
use gedo_core::ids::{GedoRefName, GitOid};
use gedo_core::ports::{LogSequence, ObjectStore, RefStore, WriteAheadLog};
use gedo_core::syntax::StructuralHash;
use gedo_core::{Collection, LabelError, StoreError};
use gedo_crdt::clock::order_key;
use gedo_crdt::{
    EventFilter, ReconstructError, RecordingContext, event_id, generation, live_clock, reconstructed_events,
    sort_events, tape_id,
};

use crate::index::{IdentityIndex, IndexError};
use records::{COMMIT_TAPES_COLLECTION, EVENTS_COLLECTION, TAPES_COLLECTION, WalRecord};

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
    /// Tapes published live for the commit, each with its events in recording order.
    Published(Vec<(TapeId, Vec<Event>)>),
    /// No tape: the events reconstructed from the commit's changes.
    Reconstructed(Vec<Event>),
}

impl CommitEvents {
    /// Every event, tape after tape.
    pub fn events(&self) -> Vec<&Event> {
        match self {
            CommitEvents::Published(tapes) => tapes.iter().flat_map(|(_, events)| events).collect(),
            CommitEvents::Reconstructed(events) => events.iter().collect(),
        }
    }
}

/// The event log of one repository.
pub struct EventLog<'a, O, R, W> {
    objects: &'a O,
    refs: &'a R,
    wal: &'a mut W,
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
            last: None,
            generations: HashMap::new(),
        };
        log.last = log.pending()?.last().map(|(_, event)| event.clock);
        Ok(log)
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
        let pending = self.pending()?;
        let Some((last_sequence, _)) = pending.last() else {
            return Ok(None);
        };
        let last_sequence = *last_sequence;
        let mut seen = BTreeSet::new();
        let events: Vec<(EventId, Event)> = pending
            .into_iter()
            .map(|(_, event)| (event_id(&event), event))
            .filter(|(id, _)| seen.insert(*id))
            .collect();
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
        let tape = Tape {
            events: events.iter().map(|(id, _)| *id).collect(),
        };
        let tape = (tape_id(&tape), tape);
        let message = format!("gedo: publish {} for {commit}", tape.0);

        let mut entries = Vec::with_capacity(events.len());
        for (id, event) in &events {
            entries.push((
                records::event_entry(id).context(|| "name an event".into())?,
                records::encode_event(event).context(|| format!("encode event {id}"))?,
            ));
        }
        self.collection(EVENTS_COLLECTION)?
            .put(&entries, &message)
            .context(|| "store the events".into())?;
        self.collection(TAPES_COLLECTION)?
            .put(
                &[(
                    records::tape_entry(&tape.0).context(|| "name the tape".into())?,
                    records::encode_tape(&tape.1).context(|| "encode the tape".into())?,
                )],
                &message,
            )
            .context(|| "store the tape".into())?;
        self.collection(COMMIT_TAPES_COLLECTION)?
            .put(
                &[(
                    records::index_entry(&commit, &tape.0).context(|| "name the index entry".into())?,
                    records::encode_index_entry().context(|| "encode the index entry".into())?,
                )],
                &message,
            )
            .context(|| "store the index entry".into())?;
        self.wal
            .truncate_through(last_sequence)
            .context(|| "remove the published events from the log".into())?;
        self.last = self.pending()?.last().map(|(_, event)| event.clock);
        tracing::info!(%commit, tape = %tape.0, events = events.len(), "events published");
        Ok(Some(tape.0))
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
        let entry = records::event_entry(id).context(|| "name the event".into())?;
        let bytes = self
            .collection(EVENTS_COLLECTION)?
            .get(&entry)
            .context(|| format!("read event {id}"))?
            .ok_or_else(|| EventError::Missing {
                what: "event",
                id: id.to_string(),
            })?;
        records::decode_event(&bytes, id).context(|| format!("decode event {id}"))
    }

    /// The events of `commit`: its published tapes, or, when it has none, the events
    /// reconstructed from its changes in `identities`.
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
        if !tapes.is_empty() {
            let mut published = Vec::with_capacity(tapes.len());
            for id in tapes {
                let tape = self.tape(&id)?;
                let events = tape
                    .events
                    .iter()
                    .map(|event| self.event(event))
                    .collect::<Result<_, _>>()?;
                published.push((id, events));
            }
            return Ok(CommitEvents::Published(published));
        }
        let changes = identities.changes_of(commit)?;
        let hashes = identities.result_hashes(commit)?;
        let author = self
            .objects
            .read_commit(commit)
            .context(|| format!("read commit {commit}"))?
            .author_email;
        let generation = self.generation(commit)?;
        let events = reconstructed_events(generation, &author, &changes.deltas, &changes.merges, |id| {
            hashes.get(&id).copied()
        })
        .map_err(|source| EventError::Reconstruct {
            commit: *commit,
            source,
        })?;
        tracing::debug!(%commit, events = events.len(), "events reconstructed");
        Ok(CommitEvents::Reconstructed(events))
    }

    /// Every stored event that matches `filter`, in the total order of events.
    pub fn filter(&self, filter: &EventFilter) -> Result<Vec<Event>, EventError> {
        let collection = self.collection(EVENTS_COLLECTION)?;
        let mut found = Vec::new();
        for (entry, blob) in collection.entries().context(|| "list the events".into())? {
            let id = EventId::from_entry_name(entry.as_str()).ok_or_else(|| EventError::Store {
                context: "list the events".into(),
                source: StoreError::Corrupted {
                    path: format!("refs/gedo/{EVENTS_COLLECTION}"),
                    offset: None,
                    reason: format!("entry {entry} is not an event id"),
                },
            })?;
            let bytes = self
                .objects
                .read_blob(&blob)
                .context(|| format!("read event {id}"))?;
            let event = records::decode_event(&bytes, &id).context(|| format!("decode event {id}"))?;
            if filter.matches(&event) {
                found.push(event);
            }
        }
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

    /// The generation of `commit` (research R3), walking only ancestors not known yet.
    pub fn generation(&mut self, commit: &GitOid) -> Result<u64, EventError> {
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
                self.generations.insert(oid, generation(&known));
                continue;
            }
            let parents = self
                .objects
                .read_commit(&oid)
                .context(|| format!("read commit {oid}"))?
                .parents;
            stack.push((oid, true));
            stack.extend(parents.iter().map(|parent| (*parent, false)));
            parents_of.insert(oid, parents);
        }
        Ok(self.generations.get(commit).copied().unwrap_or(1))
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
