//! Recording, publishing and reading live events (User Stories 2, 3 and 6).

mod support;

use std::cell::Cell;

use gedo_app::events::records::{
    self, COMMIT_TAPES_COLLECTION, EVENTS_CACHE_NAMESPACE, EVENTS_COLLECTION, TAPES_COLLECTION,
};
use gedo_app::{EventError, EventLog};
use gedo_core::event::{AgentId, AuthorKind, Clock, Labels, Origin, SemanticOp, Tape};
use gedo_core::identity::{AstNodeId, Location, SemanticDelta};
use gedo_core::ids::{GedoRefName, GitOid};
use gedo_core::memory::{MemoryCache, MemoryRefStore, MemoryWal};
use gedo_core::ports::{
    CacheEntry, CacheRebuildSource, LogSequence, ObjectStore, OpenReport, RefOutcome, RefStore, RefUpdate,
    SymbolCache, WriteAheadLog,
};
use gedo_core::syntax::{EntityKind, StructuralHash};
use gedo_core::{Collection, StoreError};
use gedo_crdt::{AuthorKindFilter, EventFilter, RecordingContext, event_id, fingerprint, parse_labels};
use support::{History, files};

fn node(n: u8) -> AstNodeId {
    AstNodeId([n; 16])
}

fn created(n: u8) -> (SemanticOp, Option<StructuralHash>) {
    (
        SemanticOp::Delta(SemanticDelta::Created {
            id: node(n),
            kind: EntityKind::Function,
            at: Location {
                unit: "src".into(),
                path: "src/lib.rs".into(),
                qualified_path: format!("crate::f{n}"),
            },
        }),
        Some(StructuralHash([n; 32])),
    )
}

fn body(n: u8, after: u8) -> (SemanticOp, Option<StructuralHash>) {
    (
        SemanticOp::Delta(SemanticDelta::BodyModified {
            id: node(n),
            before: StructuralHash([0; 32]),
            after: StructuralHash([after; 32]),
        }),
        Some(StructuralHash([after; 32])),
    )
}

fn person() -> RecordingContext {
    RecordingContext::person("levi@example.com", Labels::new()).expect("valid")
}

fn agent() -> RecordingContext {
    RecordingContext::from_texts(
        "levi@example.com",
        Some("worker-1"),
        Some("dc.plan=gedo_event_model,dc.milestone=M2"),
    )
    .expect("valid")
}

/// A history of two commits: `base`, then `next` on top of it.
fn two_commits() -> (History, GitOid, GitOid) {
    let history = History::default();
    let base = history.commit(&[], &files(&[("src/lib.rs", "pub fn f() {}\n")]));
    let next = history.commit(&[base], &files(&[("src/lib.rs", "pub fn f() { g() }\n")]));
    (history, base, next)
}

fn entries<O: ObjectStore, R: RefStore>(objects: &O, refs: &R, name: &str) -> Vec<(String, GitOid)> {
    Collection::new(GedoRefName::collection(name).expect("valid"), objects, refs)
        .entries()
        .expect("entries")
        .into_iter()
        .map(|(id, blob)| (id.to_string(), blob))
        .collect()
}

fn stored<O: ObjectStore, R: RefStore>(objects: &O, refs: &R) -> Vec<Vec<(String, GitOid)>> {
    [EVENTS_COLLECTION, TAPES_COLLECTION, COMMIT_TAPES_COLLECTION]
        .iter()
        .map(|name| entries(objects, refs, name))
        .collect()
}

#[test]
fn recorded_events_are_published_as_one_tape_of_the_commit() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");

    let first = log
        .record(&person(), Some(base), &[created(1), body(1, 2)])
        .expect("recorded");
    let second = log.record(&agent(), Some(base), &[body(1, 3)]).expect("recorded");
    assert_eq!(log.pending().expect("pending").len(), 3);

    let tape = log.publish(next).expect("published").expect("a tape");
    assert!(
        log.pending().expect("pending").is_empty(),
        "published events leave the log"
    );
    assert_eq!(log.tapes_of(&next).expect("tapes"), vec![tape]);
    assert!(log.tapes_of(&base).expect("tapes").is_empty());

    let ids: Vec<_> = first.iter().chain(&second).copied().collect();
    assert_eq!(log.tape(&tape).expect("tape").events, ids, "recording order");

    let events: Vec<_> = ids.iter().map(|id| log.event(id).expect("event")).collect();
    let clocks: Vec<Clock> = events.iter().map(|event| event.clock).collect();
    assert_eq!(
        clocks,
        [0, 1, 2].map(|sequence| Clock {
            generation: 2,
            sequence
        }),
        "one generation above the base, sequences in recording order"
    );
    assert!(events.iter().all(|event| event.origin == Origin::Live));
    assert_eq!(events[0].author.kind, AuthorKind::Person);
    assert_eq!(events[0].author.fingerprint, fingerprint("levi@example.com"));
    assert_eq!(
        events[2].author.kind,
        AuthorKind::Agent(AgentId::new("worker-1").expect("valid"))
    );
    assert_eq!(events[2].author.fingerprint, events[0].author.fingerprint);
    assert_eq!(
        events[2].labels.get("dc.milestone").map(|value| value.as_str()),
        Some("M2")
    );
    for (id, event) in ids.iter().zip(&events) {
        assert_eq!(event_id(event), *id);
    }
}

#[test]
fn an_empty_log_publishes_nothing() {
    let (history, _, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    assert_eq!(log.publish(next).expect("publish"), None);
    assert!(stored(&history.objects, &refs).iter().all(Vec::is_empty));
}

#[test]
fn sequences_continue_across_reopening_and_restart_after_a_publish() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    {
        let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
        log.record(&person(), Some(base), &[created(1)])
            .expect("recorded");
    }
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("reopens");
    let id = log
        .record(&person(), Some(base), &[body(1, 2)])
        .expect("recorded")[0];
    assert_eq!(log.pending().expect("pending")[1].1.clock.sequence, 1);
    log.publish(next).expect("published");
    assert_eq!(log.event(&id).expect("event").clock.sequence, 1);

    let after = log
        .record(&person(), Some(next), &[body(1, 3)])
        .expect("recorded")[0];
    let pending = log.pending().expect("pending");
    assert_eq!(
        pending[0].1.clock,
        Clock {
            generation: 3,
            sequence: 0
        }
    );
    assert_eq!(event_id(&pending[0].1), after);
}

#[test]
fn reserved_labels_and_bad_agents_are_refused_before_recording() {
    assert!(RecordingContext::from_texts("e@x", None, Some("gedo.stream=x")).is_err());
    assert!(RecordingContext::from_texts("e@x", Some("Agent One"), None).is_err());
    let many: Vec<String> = (0..17).map(|n| format!("k{n}=v")).collect();
    assert!(parse_labels(&many.join(",")).is_err());
}

#[test]
fn filters_find_events_by_label_kind_and_agent() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    log.record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    let tagged = log
        .record(&agent(), Some(base), &[created(2), body(2, 5)])
        .expect("recorded");
    log.publish(next).expect("published");

    let labels = parse_labels("dc.plan=gedo_event_model").expect("valid");
    let filter = EventFilter {
        labels: labels
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        ..EventFilter::default()
    };
    let found: Vec<_> = log
        .filter(&filter)
        .expect("filter")
        .iter()
        .map(event_id)
        .collect();
    assert_eq!(found, tagged);

    let people = EventFilter {
        author_kind: Some(AuthorKindFilter::Person),
        ..EventFilter::default()
    };
    assert_eq!(log.filter(&people).expect("filter").len(), 1);
    assert_eq!(log.filter(&EventFilter::default()).expect("filter").len(), 3);
}

#[test]
fn the_same_events_published_twice_are_stored_once() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    log.record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    let tape = log.publish(next).expect("published");
    let before = stored(&history.objects, &refs);

    // The same event again (same base, same sequence after the truncation) is the same id.
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("reopens");
    log.record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    assert_eq!(log.publish(next).expect("published"), tape);
    assert_eq!(stored(&history.objects, &refs), before);
}

/// A reference store that fails every compare-and-swap after `allowed` successful ones.
struct FailingRefs<'a> {
    inner: &'a MemoryRefStore,
    allowed: Cell<usize>,
}

impl RefStore for FailingRefs<'_> {
    fn read(&self, name: &GedoRefName) -> Result<Option<GitOid>, StoreError> {
        self.inner.read(name)
    }
    fn list(&self, prefix: &GedoRefName) -> Result<Vec<(GedoRefName, GitOid)>, StoreError> {
        self.inner.list(prefix)
    }
    fn list_remote(&self, remote: &str) -> Result<Vec<(String, GitOid)>, StoreError> {
        self.inner.list_remote(remote)
    }
    fn compare_and_swap(&self, update: &RefUpdate) -> Result<RefOutcome, StoreError> {
        match self.allowed.get() {
            0 => Err(StoreError::invalid_input(
                "simulated crash",
                "the process stopped",
            )),
            left => {
                self.allowed.set(left - 1);
                self.inner.compare_and_swap(update)
            }
        }
    }
    fn delete(&self, name: &GedoRefName, expected: GitOid) -> Result<RefOutcome, StoreError> {
        self.inner.delete(name, expected)
    }
}

/// A log whose truncation fails, as if the process stopped just before it.
struct NoTruncate<'a>(&'a mut MemoryWal);

impl WriteAheadLog for NoTruncate<'_> {
    fn append(&mut self, payload: &[u8]) -> Result<LogSequence, StoreError> {
        self.0.append(payload)
    }
    fn sync(&mut self) -> Result<(), StoreError> {
        self.0.sync()
    }
    fn read_all(&self) -> Result<Vec<(LogSequence, Vec<u8>)>, StoreError> {
        self.0.read_all()
    }
    fn truncate_through(&mut self, _sequence: LogSequence) -> Result<(), StoreError> {
        Err(StoreError::invalid_input(
            "simulated crash",
            "the process stopped",
        ))
    }
    fn open_report(&self) -> &OpenReport {
        self.0.open_report()
    }
}

fn record_three(history: &History, refs: &MemoryRefStore, wal: &mut MemoryWal, base: GitOid) {
    let mut log = EventLog::open(&history.objects, refs, wal).expect("opens");
    log.record(&agent(), Some(base), &[created(1), body(1, 2), created(4)])
        .expect("recorded");
    wal.elapse_durability_window();
}

#[test]
fn a_publish_interrupted_at_any_step_completes_with_the_same_result() {
    let (reference_history, base, next) = two_commits();
    let reference_refs = MemoryRefStore::new();
    let mut reference_wal = MemoryWal::new();
    record_three(&reference_history, &reference_refs, &mut reference_wal, base);
    EventLog::open(&reference_history.objects, &reference_refs, &mut reference_wal)
        .expect("opens")
        .publish(next)
        .expect("published");
    let expected = stored(&reference_history.objects, &reference_refs);

    for allowed in 0..=3 {
        let (history, base, next) = two_commits();
        let refs = MemoryRefStore::new();
        let mut wal = MemoryWal::new();
        record_three(&history, &refs, &mut wal, base);
        let failing = FailingRefs {
            inner: &refs,
            allowed: Cell::new(allowed),
        };
        let mut stopping = NoTruncate(&mut wal);
        let outcome = EventLog::open(&history.objects, &failing, &mut stopping)
            .expect("opens")
            .publish(next);
        assert!(outcome.is_err(), "step {allowed} stops the publish");

        let mut recovered = wal.crash();
        assert_eq!(recovered.read_all().expect("log").len(), 3, "no event lost");
        let mut log = EventLog::open(&history.objects, &refs, &mut recovered).expect("reopens");
        log.publish(next).expect("published");
        assert!(log.pending().expect("pending").is_empty());
        assert_eq!(
            stored(&history.objects, &refs),
            expected,
            "after a stop at step {allowed}"
        );
    }
}

#[test]
fn a_crash_while_appending_keeps_every_durable_event() {
    let (history, base, _) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    log.record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    wal.elapse_durability_window();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    log.record(&person(), Some(base), &[created(2)])
        .expect("recorded");
    let mut recovered = wal.crash();
    let log = EventLog::open(&history.objects, &refs, &mut recovered).expect("reopens");
    assert_eq!(log.pending().expect("pending").len(), 1);
}

#[test]
fn events_appended_during_a_publish_stay_for_the_next_one() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    log.record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    let pending = log.pending().expect("pending");
    drop(log);
    // Another writer appends after the publish read the log: truncating through the last record
    // read keeps it.
    let extra = records::encode_wal(&records::WalRecord::Event(
        EventLog::open(&history.objects, &refs, &mut MemoryWal::new())
            .and_then(|mut other| {
                other.record(&person(), Some(base), &[created(9)])?;
                Ok(other.pending()?.remove(0).1)
            })
            .expect("built"),
    ))
    .expect("encoded");
    let last = pending.last().expect("one").0;
    wal.append(&extra).expect("appended");
    wal.truncate_through(last).expect("truncated");
    assert_eq!(wal.read_all().expect("log").len(), 1, "the later record is kept");
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    assert_eq!(log.publish(next).expect("published").map(|_| ()), Some(()));
}

#[test]
fn two_machines_publishing_join_without_conflict() {
    let (history, base, next) = two_commits();
    let mine = MemoryRefStore::new();
    let theirs = MemoryRefStore::new();
    let mut my_wal = MemoryWal::new();
    let mut their_wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &mine, &mut my_wal).expect("opens");
    log.record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    let my_tape = log.publish(next).expect("published").expect("tape");
    let mut log = EventLog::open(&history.objects, &theirs, &mut their_wal).expect("opens");
    log.record(&agent(), Some(base), &[created(2)]).expect("recorded");
    let their_tape = log.publish(next).expect("published").expect("tape");

    for name in [EVENTS_COLLECTION, TAPES_COLLECTION, COMMIT_TAPES_COLLECTION] {
        let name = GedoRefName::collection(name).expect("valid");
        let head = theirs.read(&name).expect("read").expect("head");
        Collection::new(name, &history.objects, &mine)
            .merge(head)
            .expect("merged");
    }
    let log = EventLog::open(&history.objects, &mine, &mut my_wal).expect("opens");
    let mut tapes = log.tapes_of(&next).expect("tapes");
    tapes.sort();
    let mut expected = vec![my_tape, their_tape];
    expected.sort();
    assert_eq!(tapes, expected);
    assert_eq!(log.filter(&EventFilter::default()).expect("filter").len(), 2);
}

#[test]
fn a_tape_missing_from_the_local_data_is_reported() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    log.record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    log.publish(next).expect("published");

    // A second machine has only the index (a partial fetch).
    let partial = MemoryRefStore::new();
    let name = GedoRefName::collection(COMMIT_TAPES_COLLECTION).expect("valid");
    let head = refs.read(&name).expect("read").expect("head");
    Collection::new(name, &history.objects, &partial)
        .merge(head)
        .expect("merged");
    let mut other_wal = MemoryWal::new();
    let other = EventLog::open(&history.objects, &partial, &mut other_wal).expect("opens");
    let tape = other.tapes_of(&next).expect("tapes")[0];
    assert!(matches!(
        other.tape(&tape),
        Err(EventError::Missing { what: "tape", .. })
    ));
}

#[test]
fn oversized_tapes_are_refused_and_the_log_is_kept() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
    let ops: Vec<_> = (0..=gedo_core::event::MAX_TAPE_EVENTS)
        .map(|n| body(1, (n % 251) as u8))
        .collect();
    log.record(&person(), Some(base), &ops).expect("recorded");
    assert!(matches!(log.publish(next), Err(EventError::TapeTooLarge { .. })));
    assert_eq!(log.pending().expect("pending").len(), ops.len());
    assert!(stored(&history.objects, &refs).iter().all(Vec::is_empty));
}

#[test]
fn corrupted_records_are_errors_with_context() {
    let event = {
        let (history, base, _) = two_commits();
        let refs = MemoryRefStore::new();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
        log.record(&person(), Some(base), &[created(1)])
            .expect("recorded");
        log.pending().expect("pending").remove(0).1
    };
    let id = event_id(&event);
    let bytes = records::encode_event(&event).expect("encoded");
    assert_eq!(records::decode_event(&bytes, &id).expect("decoded"), event);

    let other = gedo_core::event::EventId([0; 16]);
    assert!(matches!(
        records::decode_event(&bytes, &other),
        Err(StoreError::Corrupted { .. })
    ));
    assert!(records::decode_event(&bytes[..bytes.len() - 1], &id).is_err());
    let mut newer = bytes.clone();
    newer[0] = 9;
    assert!(matches!(
        records::decode_event(&newer, &id),
        Err(StoreError::UnsupportedFormat { .. }) | Err(StoreError::Corrupted { .. })
    ));
    let mut broken = event.clone();
    broken.result_hash = None;
    let broken_bytes = records::encode_event(&broken).expect("encoded");
    assert!(records::decode_event(&broken_bytes, &event_id(&broken)).is_err());

    let tape = Tape { events: vec![id] };
    let tape_bytes = records::encode_tape(&tape).expect("encoded");
    let tape_id = gedo_crdt::tape_id(&tape);
    assert_eq!(
        records::decode_tape(&tape_bytes, &tape_id).expect("decoded"),
        tape
    );
    assert!(records::decode_tape(&tape_bytes, &gedo_core::event::TapeId([0; 16])).is_err());
    let empty = records::encode_tape(&Tape { events: vec![] }).expect("encoded");
    assert!(records::decode_tape(&empty, &gedo_crdt::tape_id(&Tape { events: vec![] })).is_err());
}

fn cache() -> MemoryCache {
    MemoryCache::open(&NoSource).expect("opens")
}

struct NoSource;

impl CacheRebuildSource for NoSource {
    fn entries(&self) -> Result<Vec<CacheEntry>, StoreError> {
        Ok(Vec::new())
    }
}

fn cached(cache: &MemoryCache, id: &gedo_core::event::EventId) -> Option<Vec<u8>> {
    cache.get(EVENTS_CACHE_NAMESPACE, &id.0).expect("read")
}

#[test]
fn reads_through_the_event_cache_give_the_same_events() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    let filled = cache();
    let mut log = EventLog::open(&history.objects, &refs, &mut wal)
        .expect("opens")
        .with_cache(&filled);
    let mut ids = log
        .record(&person(), Some(base), &[created(1)])
        .expect("recorded");
    ids.extend(
        log.record(&agent(), Some(base), &[created(2), body(2, 5)])
            .expect("recorded"),
    );
    log.publish(next).expect("published");
    for id in &ids {
        assert!(cached(&filled, id).is_some(), "publish fills the cache");
    }
    let all = EventFilter::default();
    let with_cache = (
        log.filter(&all).expect("filter"),
        log.events(&ids).expect("events"),
    );
    drop(log);

    let without_cache = {
        let log = EventLog::open(&history.objects, &refs, &mut wal).expect("opens");
        (
            log.filter(&all).expect("filter"),
            log.events(&ids).expect("events"),
        )
    };
    assert_eq!(with_cache, without_cache);

    // An empty cache is filled by the first filter.
    let empty = cache();
    let log = EventLog::open(&history.objects, &refs, &mut wal)
        .expect("opens")
        .with_cache(&empty);
    assert_eq!(log.filter(&all).expect("filter"), without_cache.0);
    assert!(ids.iter().all(|id| cached(&empty, id).is_some()));
    drop(log);

    // A cached record that does not match its id is refused, read again and replaced.
    let good = cached(&filled, &ids[0]).expect("cached");
    filled
        .put(
            EVENTS_CACHE_NAMESPACE,
            &[(ids[0].0.to_vec(), cached(&filled, &ids[1]).expect("cached"))],
        )
        .expect("written");
    filled
        .put(
            EVENTS_CACHE_NAMESPACE,
            &[(ids[1].0.to_vec(), b"not a record".to_vec())],
        )
        .expect("written");
    let log = EventLog::open(&history.objects, &refs, &mut wal)
        .expect("opens")
        .with_cache(&filled);
    assert_eq!(log.events(&ids).expect("events"), without_cache.1);
    assert_eq!(log.filter(&all).expect("filter"), without_cache.0);
    assert_eq!(
        cached(&filled, &ids[0]),
        Some(good),
        "the refused entry is replaced"
    );
}

#[test]
fn a_publish_stopped_after_the_cache_completes_with_the_same_result() {
    let (history, base, next) = two_commits();
    let refs = MemoryRefStore::new();
    let mut wal = MemoryWal::new();
    record_three(&history, &refs, &mut wal, base);
    let events_cache = cache();
    let mut stopping = NoTruncate(&mut wal);
    let outcome = EventLog::open(&history.objects, &refs, &mut stopping)
        .expect("opens")
        .with_cache(&events_cache)
        .publish(next);
    assert!(outcome.is_err(), "the truncation stops the publish");

    let mut recovered = wal.crash();
    let mut log = EventLog::open(&history.objects, &refs, &mut recovered)
        .expect("reopens")
        .with_cache(&events_cache);
    let published = log.pending().expect("pending");
    assert_eq!(published.len(), 3, "no event lost");
    log.publish(next).expect("published");
    assert!(log.pending().expect("pending").is_empty());
    let found = log.filter(&EventFilter::default()).expect("filter");
    assert_eq!(found.len(), 3);
    for (_, event) in published {
        assert!(cached(&events_cache, &event_id(&event)).is_some());
    }
}
