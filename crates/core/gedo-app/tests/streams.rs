//! Virtual streams (`specs/009-virtual-streams`): labeled streams found again (US1), inherited author
//! and labels (US2), switching work (US3), publishing (US4), convergence and discard (US5).

mod support;

use std::cell::Cell;

use gedo_app::{CommitEvents, EventLog, IdentityIndex, Side, StreamError, StreamPublish, Streams};
use gedo_core::StoreError;
use gedo_core::event::{AgentId, AuthorKind, Clock, Labels, Origin, SemanticOp};
use gedo_core::identity::{AstNodeId, Location, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::memory::{
    MemoryCache, MemoryObjectStore, MemoryRefStore, MemoryStreamLog, MemoryStreamStore, MemoryWal,
};
use gedo_core::ports::{ObjectStore, StreamStore};
use gedo_core::stream::StreamName;
use gedo_core::syntax::{EntityKind, StructuralHash};
use gedo_crdt::{ConflictKind, Part, RecordingContext, StreamFilter, event_id, fingerprint};
use proptest::prelude::*;
use support::{History, files};

const EMAIL: &str = "levi@example.com";

fn name(text: &str) -> StreamName {
    StreamName::new(text).expect("valid name")
}

fn agent(text: &str) -> AgentId {
    AgentId::new(text).expect("valid agent")
}

fn labels(pairs: &[(&str, &str)]) -> Labels {
    Labels::from_pairs(pairs).expect("valid labels")
}

fn created(n: u8) -> (SemanticOp, Option<StructuralHash>) {
    (
        SemanticOp::Delta(SemanticDelta::Created {
            id: AstNodeId([n; 16]),
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

struct Repo {
    history: History,
    refs: MemoryRefStore,
    store: MemoryStreamStore,
}

impl Repo {
    fn new() -> Self {
        Repo {
            history: History::default(),
            refs: MemoryRefStore::new(),
            store: MemoryStreamStore::new(),
        }
    }

    fn objects(&self) -> &MemoryObjectStore {
        &self.history.objects
    }

    fn streams(&self) -> Streams<'_, MemoryObjectStore, MemoryRefStore, MemoryStreamStore> {
        Streams::new(&self.history.objects, &self.refs, &self.store)
    }

    fn commit(&self, parents: &[GitOid], text: &str) -> GitOid {
        self.history.commit(parents, &files(&[("src/lib.rs", text)]))
    }

    fn tree(&self, text: &str) -> GitOid {
        self.history.tree(vec![("src/lib.rs", text)])
    }

    fn tree_of(&self, commit: GitOid) -> GitOid {
        self.objects().read_commit(&commit).expect("commit").tree
    }
}

// --- US1: labeled streams found again ----------------------------------------------------------

#[test]
fn create_then_list_in_a_new_process_returns_every_field() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let created = repo
        .streams()
        .create(
            &name("dc/m2"),
            Some(agent("implementer")),
            labels(&[("dc.plan", "gedo_virtual_streams"), ("dc.milestone", "2")]),
            Some(base),
        )
        .expect("created");
    // A new process: new use cases over the same stored streams.
    let reopened = Streams::new(&repo.history.objects, &repo.refs, &repo.store);
    let listed = reopened
        .list(&StreamFilter::from_texts(Some("dc.plan=gedo_virtual_streams"), None).expect("filter"))
        .expect("listed");
    assert_eq!(listed, vec![created.clone()]);
    assert_eq!(listed[0].base, Some(base));
    assert_eq!(listed[0].agent, Some(agent("implementer")));
    assert_eq!(reopened.get(&name("dc/m2")).expect("read"), created);
    assert_eq!(
        repo.store.pin_of(&name("dc/m2")),
        Some(base),
        "the base is kept alive"
    );
}

#[test]
fn list_filters_by_every_label_and_by_agent_in_name_order() {
    let repo = Repo::new();
    let streams = repo.streams();
    streams
        .create(
            &name("dc/m2"),
            Some(agent("implementer")),
            labels(&[("dc.plan", "p"), ("dc.milestone", "2")]),
            None,
        )
        .expect("created");
    streams
        .create(
            &name("dc/m1"),
            Some(agent("planner")),
            labels(&[("dc.plan", "p"), ("dc.milestone", "1")]),
            None,
        )
        .expect("created");
    streams
        .create(&name("mine"), None, labels(&[("dc.plan", "q")]), None)
        .expect("created");
    let names = |filter: StreamFilter| -> Vec<String> {
        streams
            .list(&filter)
            .expect("listed")
            .into_iter()
            .map(|stream| stream.name.to_string())
            .collect()
    };
    assert_eq!(names(StreamFilter::default()), ["dc/m1", "dc/m2", "mine"]);
    assert_eq!(
        names(StreamFilter::from_texts(Some("dc.plan=p"), None).expect("filter")),
        ["dc/m1", "dc/m2"]
    );
    assert_eq!(
        names(StreamFilter::from_texts(Some("dc.plan=p,dc.milestone=2"), None).expect("filter")),
        ["dc/m2"]
    );
    assert_eq!(
        names(StreamFilter::from_texts(None, Some("planner")).expect("filter")),
        ["dc/m1"]
    );
    assert!(names(StreamFilter::from_texts(Some("dc.plan=z"), None).expect("filter")).is_empty());
}

#[test]
fn create_refuses_and_stores_nothing() {
    let repo = Repo::new();
    let streams = repo.streams();
    let reserved = streams.create(&name("a"), None, labels(&[("gedo.kind", "x")]), None);
    assert!(matches!(reserved, Err(StreamError::Label(_))), "{reserved:?}");
    let missing_base = GitOid::from_bytes(&[7; 20]).expect("oid");
    assert!(matches!(
        streams.create(&name("a"), None, Labels::new(), Some(missing_base)),
        Err(StreamError::Store { .. })
    ));
    assert!(repo.store.names().expect("names").is_empty());

    streams
        .create(&name("a"), None, Labels::new(), None)
        .expect("created");
    assert!(matches!(
        streams.create(&name("a"), Some(agent("other")), Labels::new(), None),
        Err(StreamError::Exists(_))
    ));
    assert_eq!(
        streams.get(&name("a")).expect("read").agent,
        None,
        "the first one is kept"
    );

    assert!(StreamName::new("Dc/M2").is_err());
    let seventeen: Vec<(String, String)> = (0..17).map(|n| (format!("k{n}"), "v".to_owned())).collect();
    assert!(Labels::from_pairs(&seventeen).is_err());
    assert!(AgentId::new("Bad Agent").is_err());
    assert!(matches!(streams.get(&name("b")), Err(StreamError::NotFound(_))));
}

#[test]
fn concurrent_creations_of_one_name_have_one_winner() {
    let repo = Repo::new();
    let results: Vec<bool> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|n| {
                let repo = &repo;
                scope.spawn(move || {
                    repo.streams()
                        .create(&name("race"), Some(agent(&format!("a{n}"))), Labels::new(), None)
                        .is_ok()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("thread"))
            .collect()
    });
    assert_eq!(results.iter().filter(|won| **won).count(), 1);
}

// --- US2: inherited author and labels -----------------------------------------------------------

#[test]
fn record_inside_a_stream_inherits_its_author_and_labels() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let streams = repo.streams();
    let stream = streams
        .create(
            &name("dc/m2"),
            Some(agent("implementer")),
            labels(&[("dc.plan", "p")]),
            Some(base),
        )
        .expect("created");
    let ids = streams
        .record(&stream.name, EMAIL, &[created(1), created(2)])
        .expect("recorded");
    let pending = streams.pending(&stream.name).expect("pending");
    assert_eq!(pending.iter().map(event_id).collect::<Vec<_>>(), ids);
    for (sequence, event) in pending.iter().enumerate() {
        assert_eq!(event.author.kind, AuthorKind::Agent(agent("implementer")));
        assert_eq!(event.author.fingerprint, fingerprint(EMAIL));
        assert_eq!(event.labels, stream.labels);
        assert_eq!(event.origin, Origin::Live);
        assert_eq!(
            event.clock,
            Clock {
                generation: 2,
                sequence: sequence as u32
            },
            "live clock on the base"
        );
    }
}

#[test]
fn record_outside_streams_keeps_todays_context() {
    let repo = Repo::new();
    let streams = repo.streams();
    let outside = streams
        .recording_context(EMAIL, Some("worker-1"), Some("dc.plan=x"))
        .expect("context");
    assert_eq!(
        outside,
        RecordingContext::from_texts(EMAIL, Some("worker-1"), Some("dc.plan=x")).expect("context")
    );
    let person = streams.recording_context(EMAIL, None, None).expect("context");
    assert_eq!(person.author().kind, AuthorKind::Person);
}

#[test]
fn record_inside_an_active_stream_ignores_the_environment() {
    let repo = Repo::new();
    let streams = repo.streams();
    let stream = streams
        .create(
            &name("s"),
            Some(agent("implementer")),
            labels(&[("dc.plan", "p")]),
            None,
        )
        .expect("created");
    let empty = repo.objects().write_tree(&[]).expect("tree");
    streams.switch(Some(&stream.name), empty).expect("switched");
    let context = streams
        .recording_context(EMAIL, Some("other"), Some("dc.plan=q"))
        .expect("context");
    assert_eq!(context.author().kind, AuthorKind::Agent(agent("implementer")));
    assert_eq!(context.labels(), &stream.labels);
}

#[test]
fn record_keeps_streams_and_the_repository_log_apart() {
    let repo = Repo::new();
    let streams = repo.streams();
    streams
        .create(&name("a"), None, labels(&[("k", "a")]), None)
        .expect("created");
    streams
        .create(&name("b"), None, labels(&[("k", "b")]), None)
        .expect("created");
    let a = streams
        .record(&name("a"), EMAIL, &[created(1)])
        .expect("recorded");
    let b = streams
        .record(&name("b"), EMAIL, &[created(2), created(3)])
        .expect("recorded");
    let pending_a: Vec<_> = streams
        .pending(&name("a"))
        .expect("pending")
        .iter()
        .map(event_id)
        .collect();
    let pending_b: Vec<_> = streams
        .pending(&name("b"))
        .expect("pending")
        .iter()
        .map(event_id)
        .collect();
    assert_eq!((pending_a, pending_b), (a, b));
    // The repository's own log is a different log, untouched.
    let mut wal = MemoryWal::new();
    let log = EventLog::open(repo.objects(), &repo.refs, &mut wal).expect("opens");
    assert!(log.pending().expect("pending").is_empty());
}

// --- US3: switching work -----------------------------------------------------------------------

#[test]
fn switch_between_streams_saves_and_returns_the_work() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let streams = repo.streams();
    streams
        .create(&name("retry-policy"), None, Labels::new(), Some(base))
        .expect("created");
    streams
        .create(&name("hotfix"), None, Labels::new(), Some(base))
        .expect("created");

    let head = repo.tree_of(base);
    let outside = repo.tree("pub fn outside() {}\n");
    assert_eq!(
        streams
            .switch(Some(&name("retry-policy")), outside)
            .expect("switched"),
        Some(head)
    );
    let retry = repo.tree("pub fn retry() {}\n");
    assert_eq!(
        streams.switch(Some(&name("hotfix")), retry).expect("switched"),
        Some(head),
        "a new stream gives its base"
    );
    let fix = repo.tree("pub fn fix() {}\n");
    assert_eq!(
        streams
            .switch(Some(&name("retry-policy")), fix)
            .expect("switched"),
        Some(retry)
    );
    assert_eq!(streams.get(&name("hotfix")).expect("read").work, Some(fix));
    assert_eq!(
        streams.switch(None, retry).expect("switched"),
        Some(outside),
        "the outside work comes back"
    );
    assert_eq!(streams.active().expect("active"), None);
    assert_eq!(
        streams.switch(Some(&name("hotfix")), outside).expect("switched"),
        Some(fix)
    );
    assert_eq!(
        streams.active().expect("active").map(|stream| stream.name),
        Some(name("hotfix"))
    );
}

#[test]
fn switch_to_the_active_stream_returns_the_current_work() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let streams = repo.streams();
    streams
        .create(&name("a"), None, Labels::new(), Some(base))
        .expect("created");
    streams
        .switch(Some(&name("a")), repo.tree_of(base))
        .expect("switched");
    let work = repo.tree("pub fn g() {}\n");
    assert_eq!(
        streams.switch(Some(&name("a")), work).expect("switched"),
        Some(work)
    );
    // Work equal to the base is no saved work.
    streams
        .switch(Some(&name("a")), repo.tree_of(base))
        .expect("switched");
    assert_eq!(streams.get(&name("a")).expect("read").work, None);
    assert_eq!(repo.store.pin_of(&name("a")), Some(base));
}

#[test]
fn switch_out_of_streams_without_saved_outside_work_keeps_head() {
    let repo = Repo::new();
    let streams = repo.streams();
    streams
        .create(&name("a"), None, Labels::new(), None)
        .expect("created");
    let empty = repo.objects().write_tree(&[]).expect("tree");
    // A stream without a base gives the empty tree.
    assert_eq!(
        streams.switch(Some(&name("a")), empty).expect("switched"),
        Some(empty)
    );
    assert!(repo.store.outside_pin().is_some(), "the outside work is pinned");
}

#[test]
fn switch_to_an_unknown_stream_changes_nothing() {
    let repo = Repo::new();
    let streams = repo.streams();
    streams
        .create(&name("a"), None, Labels::new(), None)
        .expect("created");
    let work = repo.tree("pub fn g() {}\n");
    streams.switch(Some(&name("a")), work).expect("switched");
    let other = repo.tree("pub fn h() {}\n");
    assert!(matches!(
        streams.switch(Some(&name("nope")), other),
        Err(StreamError::NotFound(_))
    ));
    assert_eq!(
        streams.active().expect("active").map(|stream| stream.name),
        Some(name("a"))
    );
    assert_eq!(
        streams.get(&name("a")).expect("read").work,
        None,
        "the current work was not saved"
    );
}

#[test]
fn switch_keeps_one_active_stream_per_working_tree() {
    let repo = Repo::new();
    let second = repo.store.worktree("feature");
    let streams = repo.streams();
    let other = Streams::new(&repo.history.objects, &repo.refs, &second);
    streams
        .create(&name("a"), None, Labels::new(), None)
        .expect("created");
    streams
        .create(&name("b"), None, Labels::new(), None)
        .expect("created");
    let empty = repo.objects().write_tree(&[]).expect("tree");
    streams.switch(Some(&name("a")), empty).expect("switched");
    other.switch(Some(&name("b")), empty).expect("switched");
    assert_eq!(
        streams.active().expect("active").map(|stream| stream.name),
        Some(name("a"))
    );
    assert_eq!(
        other.active().expect("active").map(|stream| stream.name),
        Some(name("b"))
    );
    assert_eq!(
        other.list(&StreamFilter::default()).expect("listed").len(),
        2,
        "streams are shared"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn every_switch_returns_the_tree_last_saved_for_its_target(
        steps in proptest::collection::vec((0usize..4, 0u8..6), 1..24),
    ) {
        let repo = Repo::new();
        let base = repo.commit(&[], "pub fn base() {}\n");
        let base_tree = repo.tree_of(base);
        let streams = repo.streams();
        let names = [name("s0"), name("s1"), name("s2")];
        for stream in &names {
            streams.create(stream, None, Labels::new(), Some(base)).expect("created");
        }
        // The model: what each place last saved (index 3 is outside any stream).
        let mut saved: [Option<GitOid>; 4] = [None; 4];
        let mut active = 3usize;
        for (target, content) in steps {
            let current = if content == 0 { base_tree } else { repo.tree(&format!("pub fn w{content}() {{}}\n")) };
            saved[active] = Some(current);
            let returned = streams
                .switch(names.get(target), current)
                .expect("switched");
            let expected = match target {
                3 => saved[3],
                _ => Some(saved[target].unwrap_or(base_tree)),
            };
            prop_assert_eq!(returned, expected);
            active = target;
        }
    }
}

// --- US4: publishing a stream ------------------------------------------------------------------

#[test]
fn publish_writes_the_commit_with_the_events_as_its_tape() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let streams = repo.streams();
    streams
        .create(
            &name("dc/m2"),
            Some(agent("implementer")),
            labels(&[("dc.plan", "p")]),
            Some(base),
        )
        .expect("created");
    let ids = streams
        .record(&name("dc/m2"), EMAIL, &[created(1), created(2)])
        .expect("recorded");
    let work = repo.tree("pub fn f() {}\npub fn f1() {}\n");
    streams
        .switch(Some(&name("dc/m2")), repo.tree_of(base))
        .expect("entered");
    streams.switch(None, work).expect("left with the work saved");

    let outcome = streams
        .publish(&name("dc/m2"), None, "feat: m2\n")
        .expect("published");
    let StreamPublish::Published { commit, tape, events } = outcome else {
        panic!("published expected, got {outcome:?}");
    };
    assert_eq!(events, 2);
    let info = repo.objects().read_commit(&commit).expect("commit");
    assert_eq!(
        (info.tree, info.parents.clone(), info.message.as_str()),
        (work, vec![base], "feat: m2\n")
    );

    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(repo.objects(), &repo.refs, &mut wal).expect("opens");
    assert_eq!(log.tapes_of(&commit).expect("tapes"), vec![tape.expect("a tape")]);
    let identities = support::cache();
    let mut index = IdentityIndex::open(repo.objects(), &repo.history.parser, &identities).expect("opens");
    index.index_history(&[commit]).expect("indexed");
    let CommitEvents::Published { tapes, .. } = log.events_of_commit(&commit, &index).expect("events") else {
        panic!("published events expected");
    };
    assert_eq!(tapes[0].1.iter().map(event_id).collect::<Vec<_>>(), ids);

    let stream = streams.get(&name("dc/m2")).expect("read");
    assert_eq!(
        (stream.base, stream.work, stream.publishing),
        (Some(commit), None, None)
    );
    assert!(streams.pending(&name("dc/m2")).expect("pending").is_empty());
    assert_eq!(repo.store.pin_of(&name("dc/m2")), Some(commit));
    assert_eq!(
        streams
            .publish(&name("dc/m2"), None, "again\n")
            .expect("published"),
        StreamPublish::NothingToPublish
    );
}

#[test]
fn publish_of_nothing_writes_nothing() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let streams = repo.streams();
    streams
        .create(&name("a"), None, Labels::new(), Some(base))
        .expect("created");
    assert_eq!(
        streams.publish(&name("a"), None, "x\n").expect("published"),
        StreamPublish::NothingToPublish
    );
    assert_eq!(streams.get(&name("a")).expect("read").base, Some(base));
}

#[test]
fn publish_of_a_stream_without_a_base_is_a_root_commit() {
    let repo = Repo::new();
    let streams = repo.streams();
    streams
        .create(&name("a"), None, Labels::new(), None)
        .expect("created");
    streams
        .record(&name("a"), EMAIL, &[created(1)])
        .expect("recorded");
    let work = repo.tree("pub fn f1() {}\n");
    let StreamPublish::Published { commit, .. } = streams
        .publish(&name("a"), Some(work), "root\n")
        .expect("published")
    else {
        panic!("published expected");
    };
    assert!(
        repo.objects()
            .read_commit(&commit)
            .expect("commit")
            .parents
            .is_empty()
    );
    assert!(matches!(streams.side(&name("b")), Err(StreamError::NotFound(_))));
    assert!(streams.side(&name("a")).is_ok(), "the stream now has a base");
}

/// A stream store that fails the `n`-th write (record replace or pin), counting from 1, once.
struct Failing {
    inner: MemoryStreamStore,
    writes: Cell<usize>,
    fail_at: usize,
}

impl Failing {
    fn write(&self) -> Result<(), StoreError> {
        let count = self.writes.get() + 1;
        self.writes.set(count);
        if count == self.fail_at {
            Err(StoreError::invalid_input("inject", "an interruption"))
        } else {
            Ok(())
        }
    }
}

impl StreamStore for Failing {
    type Log = MemoryStreamLog;
    fn names(&self) -> Result<Vec<StreamName>, StoreError> {
        self.inner.names()
    }
    fn read(&self, name: &StreamName) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.read(name)
    }
    fn create(&self, name: &StreamName, record: &[u8]) -> Result<bool, StoreError> {
        self.inner.create(name, record)
    }
    fn replace(&self, name: &StreamName, record: &[u8]) -> Result<(), StoreError> {
        self.write()?;
        self.inner.replace(name, record)
    }
    fn remove(&self, name: &StreamName) -> Result<(), StoreError> {
        self.inner.remove(name)
    }
    fn open_log(&self, name: &StreamName) -> Result<MemoryStreamLog, StoreError> {
        self.inner.open_log(name)
    }
    fn pin(&self, name: &StreamName, commit: Option<GitOid>) -> Result<(), StoreError> {
        self.write()?;
        self.inner.pin(name, commit)
    }
    fn read_state(&self) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.read_state()
    }
    fn write_state(&self, state: &[u8], outside: Option<GitOid>) -> Result<(), StoreError> {
        self.inner.write_state(state, outside)
    }
}

#[test]
fn publish_interrupted_at_any_step_completes_without_losing_or_repeating_events() {
    // Writes of a publish: 1 the record with the commit, 2 the pin, 3 the final record.
    for fail_at in 1..=3 {
        let repo = Repo::new();
        let base = repo.commit(&[], "pub fn f() {}\n");
        let failing = Failing {
            inner: repo.store.clone(),
            writes: Cell::new(0),
            fail_at: usize::MAX,
        };
        let streams = Streams::new(&repo.history.objects, &repo.refs, &failing);
        streams
            .create(&name("a"), None, Labels::new(), Some(base))
            .expect("created");
        let ids = streams
            .record(&name("a"), EMAIL, &[created(1), created(2)])
            .expect("recorded");
        let work = repo.tree("pub fn f() {}\npub fn f1() {}\n");

        failing.writes.set(0);
        let failing = Failing { fail_at, ..failing };
        let streams = Streams::new(&repo.history.objects, &repo.refs, &failing);
        assert!(
            streams.publish(&name("a"), Some(work), "m\n").is_err(),
            "step {fail_at} interrupts"
        );

        // The next operation completes the publish (or, before the commit was recorded, the
        // events are still pending and a new publish carries them).
        let streams = repo.streams();
        let pending = streams.pending(&name("a")).expect("pending");
        let stream = streams.get(&name("a")).expect("read");
        let commit = match stream.base {
            Some(commit) if commit != base => {
                assert!(pending.is_empty(), "step {fail_at}: published once");
                assert_eq!(stream.publishing, None);
                commit
            }
            _ => {
                assert_eq!(
                    pending.iter().map(event_id).collect::<Vec<_>>(),
                    ids,
                    "step {fail_at}: nothing lost"
                );
                let StreamPublish::Published { commit, .. } =
                    streams.publish(&name("a"), Some(work), "m\n").expect("published")
                else {
                    panic!("published expected");
                };
                commit
            }
        };
        let mut wal = MemoryWal::new();
        let log = EventLog::open(repo.objects(), &repo.refs, &mut wal).expect("opens");
        let tapes = log.tapes_of(&commit).expect("tapes");
        assert_eq!(tapes.len(), 1, "step {fail_at}: one tape");
        assert_eq!(
            log.tape(&tapes[0]).expect("tape").events,
            ids,
            "step {fail_at}: every event once"
        );
        assert_eq!(repo.objects().read_commit(&commit).expect("commit").tree, work);
    }
}

// --- US5: convergence and discard --------------------------------------------------------------

const ROOT: &str = "pub fn f() -> u32 {\n    1\n}\n\npub fn g() -> u32 {\n    2\n}\n";

/// The live operations of `text` on top of `root`, as a developer working in a stream would record.
fn ops_of(
    repo: &Repo,
    identities: &MemoryCache,
    root: GitOid,
    text: &str,
) -> Vec<(SemanticOp, Option<StructuralHash>)> {
    let commit = repo.commit(&[root], text);
    let mut index = IdentityIndex::open(repo.objects(), &repo.history.parser, identities).expect("opens");
    index.index_history(&[commit]).expect("indexed");
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(repo.objects(), &repo.refs, &mut wal).expect("opens");
    let CommitEvents::Reconstructed(events) = log.events_of_commit(&commit, &index).expect("events") else {
        panic!("reconstructed expected");
    };
    events
        .iter()
        .map(|event| (event.operation.clone(), event.result_hash))
        .collect()
}

fn converge(repo: &Repo, identities: &MemoryCache, root: GitOid, sides: &[Side]) -> gedo_app::ConvergeReport {
    let index = IdentityIndex::open(repo.objects(), &repo.history.parser, identities).expect("opens");
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(repo.objects(), &repo.refs, &mut wal).expect("opens");
    log.converge(&root, sides, &index).expect("converges")
}

#[test]
fn converge_two_streams_without_and_with_conflicts() {
    let repo = Repo::new();
    let identities = support::cache();
    let root = repo.commit(&[], ROOT);
    IdentityIndex::open(repo.objects(), &repo.history.parser, &identities)
        .expect("opens")
        .index_history(&[root])
        .expect("indexed");
    let streams = repo.streams();
    for stream in ["a", "b", "c"] {
        streams
            .create(&name(stream), None, Labels::new(), Some(root))
            .expect("created");
    }
    let f_body = ops_of(&repo, &identities, root, &ROOT.replace("    1\n", "    5\n"));
    let g_body = ops_of(&repo, &identities, root, &ROOT.replace("    2\n", "    7\n"));
    let f_other = ops_of(&repo, &identities, root, &ROOT.replace("    1\n", "    6\n"));
    streams.record(&name("a"), EMAIL, &f_body).expect("recorded");
    streams.record(&name("b"), EMAIL, &g_body).expect("recorded");
    streams.record(&name("c"), EMAIL, &f_other).expect("recorded");

    let sides = [
        streams.side(&name("a")).expect("side"),
        streams.side(&name("b")).expect("side"),
    ];
    let clean = converge(&repo, &identities, root, &sides);
    assert!(clean.convergence.is_clean(), "{:?}", clean.convergence.conflicts);
    assert_eq!(clean.events, vec![f_body.len(), g_body.len()]);

    let sides = [
        streams.side(&name("a")).expect("side"),
        streams.side(&name("c")).expect("side"),
    ];
    let conflicting = converge(&repo, &identities, root, &sides);
    let conflicts = &conflicting.convergence.conflicts;
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].kind, ConflictKind::Part(Part::Body));
    assert_eq!(conflicts[0].values.len(), 2, "both streams are named");
}

#[test]
fn converge_needs_a_base() {
    let repo = Repo::new();
    let streams = repo.streams();
    streams
        .create(&name("a"), None, Labels::new(), None)
        .expect("created");
    assert!(matches!(streams.side(&name("a")), Err(StreamError::NoBase(_))));
}

#[test]
fn discard_refuses_to_drop_work_unless_forced() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let streams = repo.streams();
    streams
        .create(&name("a"), None, Labels::new(), Some(base))
        .expect("created");
    streams
        .record(&name("a"), EMAIL, &[created(1)])
        .expect("recorded");
    streams
        .switch(Some(&name("a")), repo.tree_of(base))
        .expect("entered");
    streams.switch(None, repo.tree("pub fn w() {}\n")).expect("left");
    let refused = streams.discard(&name("a"), false);
    assert!(
        matches!(
            refused,
            Err(StreamError::HasWork {
                events: 1,
                work: true,
                ..
            })
        ),
        "{refused:?}"
    );
    assert!(streams.get(&name("a")).is_ok());

    let dropped = streams.discard(&name("a"), true).expect("discarded");
    assert_eq!((dropped.events, dropped.work), (1, true));
    assert!(matches!(streams.get(&name("a")), Err(StreamError::NotFound(_))));
    assert_eq!(repo.store.pin_of(&name("a")), None);

    streams
        .create(&name("empty"), None, Labels::new(), Some(base))
        .expect("created");
    streams.discard(&name("empty"), false).expect("nothing to lose");
}

#[test]
fn discard_of_an_active_stream_from_another_working_tree() {
    let repo = Repo::new();
    let second = repo.store.worktree("feature");
    let streams = repo.streams();
    let other = Streams::new(&repo.history.objects, &repo.refs, &second);
    streams
        .create(&name("a"), None, Labels::new(), None)
        .expect("created");
    let empty = repo.objects().write_tree(&[]).expect("tree");
    streams.switch(Some(&name("a")), empty).expect("entered");
    other.discard(&name("a"), true).expect("discarded");
    assert_eq!(streams.active().expect("no active stream, with a warning"), None);
    let work = repo.tree("pub fn w() {}\n");
    assert_eq!(
        streams.switch(None, work).expect("switched"),
        Some(work),
        "the work is kept as outside work"
    );
}

// --- Snapshot of a scripted session ------------------------------------------------------------

#[test]
fn a_scripted_session_matches_the_snapshot() {
    let repo = Repo::new();
    let base = repo.commit(&[], "pub fn f() {}\n");
    let streams = repo.streams();
    let mut lines = Vec::new();
    let describe = |stream: &gedo_core::stream::Stream| {
        format!(
            "{} agent={} labels=[{}] base={} work={} pending={}",
            stream.name,
            stream.agent.as_ref().map_or("-", AgentId::as_str),
            gedo_crdt::format_labels(&stream.labels),
            if stream.base == Some(base) {
                "base"
            } else if stream.base.is_some() {
                "published"
            } else {
                "-"
            },
            stream.work.is_some(),
            streams.pending(&stream.name).expect("pending").len(),
        )
    };
    streams
        .create(
            &name("dc/m1"),
            Some(agent("planner")),
            labels(&[("dc.plan", "p"), ("dc.milestone", "1")]),
            Some(base),
        )
        .expect("created");
    streams
        .create(
            &name("dc/m2"),
            Some(agent("implementer")),
            labels(&[("dc.plan", "p"), ("dc.milestone", "2")]),
            Some(base),
        )
        .expect("created");
    streams
        .create(&name("mine"), None, Labels::new(), Some(base))
        .expect("created");
    streams
        .record(&name("dc/m2"), EMAIL, &[created(1), created(2)])
        .expect("recorded");
    streams
        .switch(Some(&name("dc/m2")), repo.tree_of(base))
        .expect("switched");
    streams
        .switch(Some(&name("mine")), repo.tree("pub fn f() {}\npub fn f1() {}\n"))
        .expect("switched");
    lines.push("after switching:".to_owned());
    for stream in streams.list(&StreamFilter::default()).expect("listed") {
        lines.push(format!("  {}", describe(&stream)));
    }
    let outcome = match streams
        .publish(&name("dc/m2"), None, "feat: m2\n")
        .expect("published")
    {
        StreamPublish::Published { tape, events, .. } => {
            format!("published {events} events, tape {}", tape.is_some())
        }
        StreamPublish::NothingToPublish => "nothing to publish".into(),
    };
    lines.push(format!("publish dc/m2: {outcome}"));
    lines.push(format!(
        "discard dc/m1: {:?}",
        streams
            .discard(&name("dc/m1"), false)
            .map(|dropped| (dropped.events, dropped.work))
            .map_err(|error| error.to_string())
    ));
    lines.push("filter dc.plan=p:".to_owned());
    for stream in streams
        .list(&StreamFilter::from_texts(Some("dc.plan=p"), None).expect("filter"))
        .expect("listed")
    {
        lines.push(format!("  {}", describe(&stream)));
    }
    lines.push(format!(
        "active: {}",
        streams
            .active()
            .expect("active")
            .map_or("-".to_owned(), |stream| stream.name.to_string())
    ));
    insta::assert_snapshot!("stream_session", lines.join("\n"));
}
