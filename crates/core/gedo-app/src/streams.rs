//! Virtual streams (FEAT-068, `specs/009-virtual-streams/research.md` R5 to R11): named units of
//! work kept on the developer's machine, each with its base commit, its author kind and labels, its
//! saved work tree and its own log of pending events.
//!
//! Events recorded inside a stream inherit its author and labels and go to its own log, through
//! [`EventLog`]. Switching saves the caller's current work tree into the stream left and returns
//! the target's; publishing writes the stream's commit on its base with its events as the commit's
//! tape. Nothing here writes the working tree, the index, `HEAD` or a branch: the caller builds the
//! work tree from the working files and writes the returned tree back.
//!
//! Every change of a stream's record is made while holding the stream's log, so the log, which
//! has one writer at a time, serializes the changes of one stream (R10).

use gedo_core::StoreError;
use gedo_core::event::{AgentId, Event, EventId, LabelError, Labels, SemanticOp, TapeId};
use gedo_core::ids::{GitOid, ParseIdError};
use gedo_core::ports::{NewCommit, ObjectStore, RefStore, StreamStore, SymbolCache};
use gedo_core::stream::{
    Stream, StreamName, WorktreeState, decode_state, decode_stream, encode_state, encode_stream,
};
use gedo_core::syntax::StructuralHash;
use gedo_crdt::{RecordingContext, StreamFilter, recording_context, stream_context};

use crate::events::converge::Side;
use crate::events::{EventError, EventLog};

/// Message of the commits that keep a stream's saved work alive (research R7).
pub const WORK_COMMIT_MESSAGE: &str = "gedo stream work\n";

/// Why a stream operation failed.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    /// A stream name was refused.
    #[error(transparent)]
    Name(#[from] ParseIdError),

    /// An agent or labels were refused.
    #[error(transparent)]
    Label(#[from] LabelError),

    /// A stream with this name already exists.
    #[error("stream {0} already exists")]
    Exists(StreamName),

    /// No stream has this name.
    #[error("stream {0} does not exist")]
    NotFound(StreamName),

    /// The stream has no base commit, so it cannot be a convergence side.
    #[error("stream {0} has no base commit")]
    NoBase(StreamName),

    /// Discarding would lose pending events or saved work, and the caller did not force it.
    #[error("stream {name} has {events} pending events{}; discard it with force to drop them", if *work { " and saved work" } else { "" })]
    HasWork {
        /// The stream.
        name: StreamName,
        /// Its pending events.
        events: usize,
        /// Whether it has saved work.
        work: bool,
    },

    /// A stored stream record or working tree state was refused.
    #[error("stream {name}: {source}")]
    Record {
        /// The stream, or `state` for the working tree state.
        name: String,
        /// Why.
        #[source]
        source: StoreError,
    },

    /// Reading or writing storage failed.
    #[error("{context}: {source}")]
    Store {
        /// What was being done.
        context: String,
        /// The storage error.
        #[source]
        source: StoreError,
    },

    /// Recording or publishing events failed.
    #[error(transparent)]
    Event(#[from] EventError),
}

trait Context<T> {
    fn context(self, context: impl FnOnce() -> String) -> Result<T, StreamError>;
}

impl<T> Context<T> for Result<T, StoreError> {
    fn context(self, context: impl FnOnce() -> String) -> Result<T, StreamError> {
        self.map_err(|source| StreamError::Store {
            context: context(),
            source,
        })
    }
}

/// What publishing a stream did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamPublish {
    /// No pending event and no work besides the base: nothing was written.
    NothingToPublish,
    /// The stream's commit was written; the stream is now on it.
    Published {
        /// The commit (the stream's work on its base).
        commit: GitOid,
        /// The tape of the stream's events; `None` when it had none.
        tape: Option<TapeId>,
        /// The number of events published.
        events: usize,
    },
}

/// What discarding a stream dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Discarded {
    /// Pending events dropped.
    pub events: usize,
    /// Whether saved work was dropped.
    pub work: bool,
}

/// The virtual streams of a repository, seen from one working tree.
pub struct Streams<'a, O, R, S> {
    objects: &'a O,
    refs: &'a R,
    store: &'a S,
    cache: Option<&'a dyn SymbolCache>,
}

impl<'a, O: ObjectStore, R: RefStore, S: StreamStore> Streams<'a, O, R, S> {
    /// The streams kept in `store`, over the repository's object and reference stores.
    pub fn new(objects: &'a O, refs: &'a R, store: &'a S) -> Self {
        Streams {
            objects,
            refs,
            store,
            cache: None,
        }
    }

    /// Uses `cache` for the event logs of the streams (generations and stored events).
    pub fn with_cache(mut self, cache: &'a dyn SymbolCache) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Creates the stream `name` on `base` with `agent` and `labels`, which never change
    /// afterwards. Labels with the reserved prefix `gedo.` and a name already used are refused.
    pub fn create(
        &self,
        name: &StreamName,
        agent: Option<AgentId>,
        labels: Labels,
        base: Option<GitOid>,
    ) -> Result<Stream, StreamError> {
        let labels = labels.for_recording()?;
        if let Some(base) = base {
            self.objects
                .read_commit(&base)
                .context(|| format!("read the base {base} of stream {name}"))?;
        }
        let stream = Stream::new(name.clone(), agent, labels, base);
        let record = encode_stream(&stream).context(|| format!("encode stream {name}"))?;
        if !self
            .store
            .create(name, &record)
            .context(|| format!("create stream {name}"))?
        {
            return Err(StreamError::Exists(name.clone()));
        }
        if let Some(base) = base {
            self.store
                .pin(name, Some(base))
                .context(|| format!("pin stream {name}"))?;
        }
        tracing::info!(stream = %name, agent = ?stream.agent.as_ref().map(AgentId::as_str), labels = stream.labels.len(), "stream created");
        Ok(stream)
    }

    /// The stream `name`.
    pub fn get(&self, name: &StreamName) -> Result<Stream, StreamError> {
        let bytes = self
            .store
            .read(name)
            .context(|| format!("read stream {name}"))?
            .ok_or_else(|| StreamError::NotFound(name.clone()))?;
        decode_stream(name, &bytes).map_err(|source| {
            tracing::warn!(stream = %name, error = %source, "stream record refused");
            StreamError::Record {
                name: name.to_string(),
                source,
            }
        })
    }

    /// The streams `filter` keeps, in name order.
    pub fn list(&self, filter: &StreamFilter) -> Result<Vec<Stream>, StreamError> {
        let names = self.store.names().context(|| "list the streams".into())?;
        let mut streams = Vec::with_capacity(names.len());
        for name in names {
            match self.get(&name) {
                Ok(stream) if filter.matches(&stream) => streams.push(stream),
                Ok(_) => {}
                // Removed by another process between the listing and the read.
                Err(StreamError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(streams)
    }

    /// The working tree's active stream; a stream discarded meanwhile (from another working
    /// tree) counts as none, with a warning.
    pub fn active(&self) -> Result<Option<Stream>, StreamError> {
        let state = self.state()?;
        let Some(name) = state.active else {
            return Ok(None);
        };
        match self.get(&name) {
            Ok(stream) => Ok(Some(stream)),
            Err(StreamError::NotFound(_)) => {
                tracing::warn!(stream = %name, "active stream missing; it was discarded");
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// The recording context of this working tree: its active stream's, or outside any stream the
    /// one named by the texts of `GEDO_AGENT` and `GEDO_LABELS`.
    pub fn recording_context(
        &self,
        email: &str,
        agent: Option<&str>,
        labels: Option<&str>,
    ) -> Result<RecordingContext, StreamError> {
        let active = self.active()?;
        if active.is_some()
            && (agent.is_some_and(|text| !text.is_empty()) || labels.is_some_and(|text| !text.is_empty()))
        {
            tracing::debug!("GEDO_AGENT and GEDO_LABELS ignored inside a stream");
        }
        Ok(recording_context(active.as_ref(), email, agent, labels)?)
    }

    /// Records live events of the Git identity with `email` inside the stream `name`: they carry
    /// the stream's author kind and labels, a live clock on its base, and go to its own log.
    pub fn record(
        &self,
        name: &StreamName,
        email: &str,
        ops: &[(SemanticOp, Option<StructuralHash>)],
    ) -> Result<Vec<EventId>, StreamError> {
        let (mut wal, stream) = self.open(name)?;
        let context = stream_context(&stream, email)?;
        let mut events = self.event_log(&mut wal)?;
        let ids = events.record(&context, stream.base, ops)?;
        tracing::debug!(stream = %name, events = ids.len(), "events recorded in a stream");
        Ok(ids)
    }

    /// The pending events of the stream `name`, in recording order.
    pub fn pending(&self, name: &StreamName) -> Result<Vec<Event>, StreamError> {
        let (mut wal, _) = self.open(name)?;
        let events = self.event_log(&mut wal)?;
        Ok(events.pending()?.into_iter().map(|(_, event)| event).collect())
    }

    /// Saves `current`, the tree of the caller's working files, where the working tree is (its
    /// active stream, or its outside work), makes `target` (a stream, or none) active and returns
    /// the target's work: a stream's saved work, its base's tree or the empty tree; out of streams,
    /// the saved outside work, or `None` when nothing was saved (keep the tree of `HEAD`).
    pub fn switch(
        &self,
        target: Option<&StreamName>,
        current: GitOid,
    ) -> Result<Option<GitOid>, StreamError> {
        if let Some(target) = target {
            self.get(target)?;
        }
        let mut state = self.state()?;
        let active = match state.active.take() {
            Some(name)
                if self
                    .store
                    .read(&name)
                    .context(|| format!("read stream {name}"))?
                    .is_some() =>
            {
                Some(name)
            }
            Some(name) => {
                tracing::warn!(stream = %name, "active stream missing; it was discarded");
                None
            }
            None => None,
        };
        match &active {
            Some(name) => self.save_work(name, current)?,
            None => state.outside = Some(current),
        }
        let work = match target {
            Some(target) => {
                let stream = self.get(target)?;
                Some(match stream.work {
                    Some(work) => work,
                    None => self.base_tree(&stream)?,
                })
            }
            None => state.outside,
        };
        state.active = target.cloned();
        self.write_state(&state)?;
        tracing::info!(
            from = active.as_ref().map_or("", StreamName::as_str),
            to = target.map_or("", StreamName::as_str),
            "stream switched"
        );
        Ok(work)
    }

    /// Publishes the stream `name`: writes the commit of `tree` (or the stream's saved work, or its
    /// base's tree) on its base with `message`, publishes the pending events as its tape and moves
    /// the stream onto it. No branch moves. An interrupted publish is completed by the next
    /// operation on the stream, with the same commit (research R9).
    pub fn publish(
        &self,
        name: &StreamName,
        tree: Option<GitOid>,
        message: &str,
    ) -> Result<StreamPublish, StreamError> {
        let (mut wal, mut stream) = self.open(name)?;
        let mut events = self.event_log(&mut wal)?;
        let pending = events.pending()?.len();
        let base_tree = self.base_tree(&stream)?;
        let tree = tree.or(stream.work).unwrap_or(base_tree);
        if pending == 0 && tree == base_tree {
            return Ok(StreamPublish::NothingToPublish);
        }
        let commit = self
            .objects
            .write_commit(&NewCommit {
                tree,
                parents: stream.base.into_iter().collect(),
                message: message.to_owned(),
            })
            .context(|| format!("write the commit of stream {name}"))?;
        stream.publishing = Some(commit);
        self.replace(&stream)?;
        self.store
            .pin(name, Some(commit))
            .context(|| format!("pin stream {name}"))?;
        let tape = events.publish(commit)?;
        stream.base = Some(commit);
        stream.work = None;
        stream.publishing = None;
        self.replace(&stream)?;
        tracing::info!(stream = %name, %commit, tape = ?tape.map(|tape| tape.to_string()), events = pending, "stream published");
        Ok(StreamPublish::Published {
            commit,
            tape,
            events: pending,
        })
    }

    /// The stream `name` as a side of a convergence (feature 006): its base and pending events.
    pub fn side(&self, name: &StreamName) -> Result<Side, StreamError> {
        let (mut wal, stream) = self.open(name)?;
        let tip = stream.base.ok_or_else(|| StreamError::NoBase(name.clone()))?;
        let events = self.event_log(&mut wal)?;
        let live = events.pending()?.into_iter().map(|(_, event)| event).collect();
        Ok(Side { tip, live })
    }

    /// Discards the stream `name` with its record, log and pin. A stream with pending events or
    /// saved work is discarded only with `force`.
    pub fn discard(&self, name: &StreamName, force: bool) -> Result<Discarded, StreamError> {
        let (mut wal, stream) = self.open(name)?;
        let events = self.event_log(&mut wal)?.pending()?.len();
        let work = stream.work.is_some();
        if !force && (events > 0 || work) {
            return Err(StreamError::HasWork {
                name: name.clone(),
                events,
                work,
            });
        }
        // The log is closed before its directory is removed (Windows keeps open files).
        drop(wal);
        self.store
            .remove(name)
            .context(|| format!("remove stream {name}"))?;
        let mut state = self.state()?;
        if state.active.as_ref() == Some(name) {
            state.active = None;
            self.write_state(&state)?;
        }
        tracing::info!(stream = %name, events, work, "stream discarded");
        Ok(Discarded { events, work })
    }

    /// Opens the log of `name` (its lock for the caller's operation) and reads the stream,
    /// completing a publish in progress first.
    fn open(&self, name: &StreamName) -> Result<(S::Log, Stream), StreamError> {
        self.get(name)?;
        let mut wal = self
            .store
            .open_log(name)
            .context(|| format!("open the log of stream {name}"))?;
        let mut stream = self.get(name)?;
        if let Some(commit) = stream.publishing {
            let mut events = self.event_log(&mut wal)?;
            let count = events.pending()?.len();
            events.publish(commit)?;
            stream.base = Some(commit);
            stream.work = None;
            stream.publishing = None;
            self.replace(&stream)?;
            self.store
                .pin(name, Some(commit))
                .context(|| format!("pin stream {name}"))?;
            tracing::warn!(stream = %name, %commit, events = count, "stream publish completed");
        }
        Ok((wal, stream))
    }

    fn event_log<'w>(&self, wal: &'w mut S::Log) -> Result<EventLog<'w, O, R, S::Log>, StreamError>
    where
        'a: 'w,
    {
        let log = EventLog::open(self.objects, self.refs, wal)?;
        Ok(match self.cache {
            Some(cache) => log.with_cache(cache),
            None => log,
        })
    }

    /// Saves `work` as the work of the active stream `name` and pins it.
    fn save_work(&self, name: &StreamName, work: GitOid) -> Result<(), StreamError> {
        let (_wal, mut stream) = self.open(name)?;
        let base_tree = self.base_tree(&stream)?;
        stream.work = (work != base_tree).then_some(work);
        let pin = match stream.work {
            Some(work) => Some(self.work_commit(work, stream.base)?),
            None => stream.base,
        };
        self.replace(&stream)?;
        self.store.pin(name, pin).context(|| format!("pin stream {name}"))
    }

    /// A commit keeping `tree` (and `base`) reachable through a pin (research R7).
    fn work_commit(&self, tree: GitOid, base: Option<GitOid>) -> Result<GitOid, StreamError> {
        self.objects
            .write_commit(&NewCommit {
                tree,
                parents: base.into_iter().collect(),
                message: WORK_COMMIT_MESSAGE.into(),
            })
            .context(|| "write a stream work commit".into())
    }

    fn base_tree(&self, stream: &Stream) -> Result<GitOid, StreamError> {
        match stream.base {
            Some(base) => Ok(self
                .objects
                .read_commit(&base)
                .context(|| format!("read the base {base} of stream {}", stream.name))?
                .tree),
            None => self
                .objects
                .write_tree(&[])
                .context(|| "write the empty tree".into()),
        }
    }

    fn replace(&self, stream: &Stream) -> Result<(), StreamError> {
        let record = encode_stream(stream).context(|| format!("encode stream {}", stream.name))?;
        self.store
            .replace(&stream.name, &record)
            .context(|| format!("write stream {}", stream.name))
    }

    fn state(&self) -> Result<WorktreeState, StreamError> {
        match self
            .store
            .read_state()
            .context(|| "read the stream state".into())?
        {
            Some(bytes) => decode_state(&bytes).map_err(|source| StreamError::Record {
                name: "state".into(),
                source,
            }),
            None => Ok(WorktreeState::default()),
        }
    }

    fn write_state(&self, state: &WorktreeState) -> Result<(), StreamError> {
        let pin = state
            .outside
            .map(|outside| self.work_commit(outside, None))
            .transpose()?;
        let bytes = encode_state(state).context(|| "encode the stream state".into())?;
        self.store
            .write_state(&bytes, pin)
            .context(|| "write the stream state".into())
    }
}
