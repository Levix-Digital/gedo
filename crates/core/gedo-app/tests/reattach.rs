//! Re-attachment of tapes to rewritten and fetched commits, the retention report and the filling
//! of the event cache (User Stories 1 to 5 of `specs/005-history-reattachment`).

mod support;

use std::collections::BTreeMap;

use gedo_app::events::records::{self, COMMIT_TAPES_COLLECTION, EVENTS_CACHE_NAMESPACE};
use gedo_app::{CommitEvents, DEFAULT_GRACE_DAYS, EventLog, IdentityIndex};
use gedo_core::Collection;
use gedo_core::event::{Labels, Origin, SemanticOp, TapeId};
use gedo_core::identity::SemanticDelta;
use gedo_core::ids::{GedoRefName, GitOid};
use gedo_core::memory::{MemoryCache, MemoryRefStore, MemoryWal};
use gedo_core::ports::SymbolCache;
use gedo_crdt::RecordingContext;
use support::{History, files};

const DAY: u64 = 86_400;

fn person() -> RecordingContext {
    RecordingContext::person("levi@example.com", Labels::new()).expect("valid")
}

/// A repository under test: its history, the identity cache, the Gedo references and the event
/// cache.
struct Repo {
    history: History,
    identities: MemoryCache,
    refs: MemoryRefStore,
    events: MemoryCache,
}

impl Repo {
    fn new() -> Self {
        Self {
            history: History::default(),
            identities: support::cache(),
            refs: MemoryRefStore::new(),
            events: support::cache(),
        }
    }

    fn commit(&self, parents: &[GitOid], pairs: &[(&str, &str)]) -> GitOid {
        self.history.commit(parents, &files(pairs))
    }

    fn index(&self, tips: &[GitOid]) {
        let mut index = IdentityIndex::open(&self.history.objects, &self.history.parser, &self.identities)
            .expect("opens");
        index.index_history(tips).expect("indexed");
    }

    fn identities(
        &self,
    ) -> IdentityIndex<'_, gedo_core::memory::MemoryObjectStore, gedo_parse::TreeSitterParser, MemoryCache>
    {
        IdentityIndex::open(&self.history.objects, &self.history.parser, &self.identities).expect("opens")
    }

    /// Records the changes of `commit` as live events on top of its first parent and publishes
    /// them as its tape, as a developer working with Gedo would.
    fn publish_live(&self, commit: GitOid) -> TapeId {
        self.index(&[commit]);
        let identities = self.identities();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
        let CommitEvents::Reconstructed(events) = log.events_of_commit(&commit, &identities).expect("events")
        else {
            panic!("{commit} is already published");
        };
        let ops: Vec<_> = events
            .iter()
            .map(|event| (event.operation.clone(), event.result_hash))
            .collect();
        let parent = self.history.objects.read_commit_parents(&commit).first().copied();
        log.record(&person(), parent, &ops).expect("recorded");
        log.publish(commit).expect("published").expect("a tape")
    }

    fn rewritten(&self, pairs: &[(GitOid, GitOid)]) -> gedo_app::ReattachReport {
        let news: Vec<GitOid> = pairs.iter().map(|(_, new)| *new).collect();
        self.index(&news);
        let identities = self.identities();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal)
            .expect("opens")
            .with_cache(&self.events);
        log.reattach_rewritten(pairs, &identities).expect("re-attached")
    }

    fn fetched(&self, commits: &[GitOid], tips: &[GitOid]) -> gedo_app::ReattachReport {
        self.index(commits);
        let identities = self.identities();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal)
            .expect("opens")
            .with_cache(&self.events);
        log.reattach_new(commits, tips, &identities).expect("re-attached")
    }

    fn events_of(&self, commit: GitOid) -> CommitEvents {
        self.index(&[commit]);
        let identities = self.identities();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
        log.events_of_commit(&commit, &identities).expect("events")
    }

    fn tapes_of(&self, commit: GitOid) -> Vec<TapeId> {
        let mut wal = MemoryWal::new();
        let log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
        log.tapes_of(&commit).expect("tapes")
    }

    fn detached(&self, tips: &[GitOid], now: u64) -> Vec<gedo_app::DetachedTape> {
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal)
            .expect("opens")
            .with_cache(&self.events);
        log.detached(tips, now, DEFAULT_GRACE_DAYS).expect("report")
    }

    /// Every commit index entry, as text.
    fn index_entries(&self) -> String {
        let name = GedoRefName::collection(COMMIT_TAPES_COLLECTION).expect("valid");
        let collection = Collection::new(name, &self.history.objects, &self.refs);
        collection
            .entries()
            .expect("entries")
            .keys()
            .map(|entry| {
                let (commit, tape) = records::parse_index_entry(entry).expect("valid");
                format!("{commit} {tape}\n")
            })
            .collect()
    }
}

trait Parents {
    fn read_commit_parents(&self, commit: &GitOid) -> Vec<GitOid>;
}

impl Parents for gedo_core::memory::MemoryObjectStore {
    fn read_commit_parents(&self, commit: &GitOid) -> Vec<GitOid> {
        use gedo_core::ports::ObjectStore;
        self.read_commit(commit).expect("commit").parents
    }
}

const BASE: &str = "pub fn base() -> u32 {\n    1\n}\n";
const OTHER: &str = "pub fn other() -> u32 {\n    2\n}\n";
const HELPER_V1: &str = "pub fn base() -> u32 {\n    1\n}\n\npub fn helper() -> u32 {\n    10\n}\n";
const HELPER_V2: &str = "pub fn base() -> u32 {\n    1\n}\n\npub fn helper() -> u32 {\n    20\n}\n";
const BASE_EDITED: &str = "pub fn base() -> u32 {\n    5\n}\n";

/// main: root; branch: one commit editing `base`; main moves on with another file.
fn rebase_setup(repo: &Repo) -> (GitOid, GitOid, GitOid) {
    let root = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let work = repo.commit(&[root], &[("src/lib.rs", BASE_EDITED)]);
    let main = repo.commit(&[root], &[("src/lib.rs", BASE), ("src/other.rs", OTHER)]);
    repo.index(&[work, main]);
    (root, work, main)
}

#[test]
fn a_rebase_that_keeps_the_changes_takes_the_fast_path() {
    let repo = Repo::new();
    let (_, work, main) = rebase_setup(&repo);
    let tape = repo.publish_live(work);
    let rebased = repo.commit(&[main], &[("src/lib.rs", BASE_EDITED), ("src/other.rs", OTHER)]);
    let report = repo.rewritten(&[(work, rebased)]);
    assert_eq!(report.commits.len(), 1);
    assert!(report.commits[0].fast_path);
    assert_eq!(report.commits[0].attached, vec![tape]);
    match repo.events_of(rebased) {
        CommitEvents::Published { tapes, remainder } => {
            assert_eq!(tapes[0].0, tape);
            assert!(tapes[0].1.iter().all(|event| event.origin == Origin::Live));
            assert!(remainder.is_empty(), "{remainder:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_amend_of_the_message_keeps_the_tape() {
    let repo = Repo::new();
    let (root, work, _) = rebase_setup(&repo);
    let tape = repo.publish_live(work);
    let amended = repo.history.commit_with_message(
        &[root],
        &files(&[("src/lib.rs", BASE_EDITED)]),
        "a better message",
    );
    let report = repo.rewritten(&[(work, amended)]);
    assert!(report.commits[0].fast_path);
    assert_eq!(repo.tapes_of(amended), vec![tape]);
}

#[test]
fn an_amend_with_code_keeps_the_tape_and_reconstructs_the_extra_change() {
    let repo = Repo::new();
    let (root, work, _) = rebase_setup(&repo);
    let tape = repo.publish_live(work);
    let amended = repo.commit(&[root], &[("src/lib.rs", BASE_EDITED), ("src/other.rs", OTHER)]);
    let report = repo.rewritten(&[(work, amended)]);
    assert!(!report.commits[0].fast_path);
    assert_eq!(report.commits[0].attached, vec![tape]);
    assert_eq!(report.commits[0].unmatched, 1);
    match repo.events_of(amended) {
        CommitEvents::Published { remainder, .. } => {
            assert_eq!(remainder.len(), 1, "{remainder:?}");
            assert_eq!(remainder[0].origin, Origin::Reconstructed);
            assert!(matches!(
                remainder[0].operation,
                SemanticOp::Delta(SemanticDelta::Created { .. })
            ));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_rebase_with_a_conflict_resolved_by_hand_keeps_the_matching_changes() {
    let repo = Repo::new();
    let root = repo.commit(&[], &[("src/lib.rs", BASE), ("src/other.rs", OTHER)]);
    let work = repo.commit(
        &[root],
        &[
            ("src/lib.rs", BASE_EDITED),
            ("src/other.rs", "pub fn other() -> u32 {\n    3\n}\n"),
        ],
    );
    let main = repo.commit(
        &[root],
        &[
            ("src/lib.rs", BASE),
            ("src/other.rs", "pub fn other() -> u32 {\n    4\n}\n"),
        ],
    );
    repo.index(&[work, main]);
    let tape = repo.publish_live(work);
    // The conflict on `other` is resolved with a third body; `base` is replayed unchanged.
    let rebased = repo.commit(
        &[main],
        &[
            ("src/lib.rs", BASE_EDITED),
            ("src/other.rs", "pub fn other() -> u32 {\n    7\n}\n"),
        ],
    );
    let report = repo.rewritten(&[(work, rebased)]);
    assert!(!report.commits[0].fast_path);
    assert_eq!(report.commits[0].attached, vec![tape]);
    assert_eq!(report.commits[0].unmatched, 1);
}

#[test]
fn an_interactive_squash_attaches_the_tapes_of_every_squashed_commit() {
    let repo = Repo::new();
    let root = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let first = repo.commit(&[root], &[("src/lib.rs", HELPER_V1)]);
    let second = repo.commit(&[first], &[("src/lib.rs", HELPER_V2)]);
    let one = repo.publish_live(first);
    let two = repo.publish_live(second);
    let squashed = repo.commit(&[root], &[("src/lib.rs", HELPER_V2)]);
    let report = repo.rewritten(&[(first, squashed), (second, squashed)]);
    let mut expected = vec![one, two];
    expected.sort();
    assert_eq!(report.commits[0].attached, expected);
    assert_eq!(report.commits[0].links, 1);
}

#[test]
fn a_split_gives_each_new_commit_the_tape_of_its_changes() {
    let repo = Repo::new();
    let (root, _, _) = rebase_setup(&repo);
    let both = repo.commit(&[root], &[("src/lib.rs", BASE_EDITED), ("src/other.rs", OTHER)]);
    let tape = repo.publish_live(both);
    let left = repo.commit(&[root], &[("src/lib.rs", BASE_EDITED)]);
    let right = repo.commit(&[left], &[("src/lib.rs", BASE_EDITED), ("src/other.rs", OTHER)]);
    let report = repo.rewritten(&[(both, left), (both, right)]);
    assert_eq!(report.commits.len(), 2);
    assert!(report.commits.iter().all(|commit| commit.attached == vec![tape]));
}

/// main: root, then an unrelated commit; branch: creates `helper`, then edits it. The branch is
/// squashed onto main on the server and deleted.
fn squash_setup(repo: &Repo) -> (GitOid, Vec<TapeId>, GitOid) {
    let root = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let first = repo.commit(&[root], &[("src/lib.rs", HELPER_V1)]);
    let second = repo.commit(&[first], &[("src/lib.rs", HELPER_V2)]);
    let main = repo.commit(&[root], &[("src/lib.rs", BASE), ("src/other.rs", OTHER)]);
    let mut tapes = vec![repo.publish_live(first), repo.publish_live(second)];
    tapes.sort();
    let squash = repo.commit(&[main], &[("src/lib.rs", HELPER_V2), ("src/other.rs", OTHER)]);
    (main, tapes, squash)
}

#[test]
fn a_server_squash_attaches_the_branch_and_links_the_created_entity() {
    let repo = Repo::new();
    let (_, tapes, squash) = squash_setup(&repo);
    let report = repo.fetched(&[squash], &[squash]);
    assert_eq!(report.commits[0].attached, tapes);
    assert_eq!(report.commits[0].links, 1);
    match repo.events_of(squash) {
        CommitEvents::Published {
            tapes: found,
            remainder,
        } => {
            assert_eq!(found.len(), 2);
            assert_eq!(remainder.len(), 1, "{remainder:?}");
            let SemanticOp::IdentityMerged { kept, merged } = remainder[0].operation else {
                panic!("{remainder:?}");
            };
            assert!(kept < merged);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_detached_unrelated_branch_stays_detached() {
    let repo = Repo::new();
    let (main, tapes, squash) = squash_setup(&repo);
    let unrelated = repo.commit(
        &[main],
        &[
            ("src/lib.rs", BASE),
            ("src/other.rs", "pub fn other() -> u32 {\n    9\n}\n"),
        ],
    );
    let theirs = repo.publish_live(unrelated);
    let report = repo.fetched(&[squash], &[squash]);
    assert_eq!(report.commits[0].attached, tapes);
    assert!(!report.commits[0].attached.contains(&theirs));
}

#[test]
fn a_server_rebase_merge_gives_each_replayed_commit_its_tape() {
    let repo = Repo::new();
    let root = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let first = repo.commit(&[root], &[("src/lib.rs", HELPER_V1)]);
    let second = repo.commit(&[first], &[("src/lib.rs", HELPER_V2)]);
    let main = repo.commit(&[root], &[("src/lib.rs", BASE), ("src/other.rs", OTHER)]);
    let one = repo.publish_live(first);
    let two = repo.publish_live(second);
    let replay_one = repo.commit(&[main], &[("src/lib.rs", HELPER_V1), ("src/other.rs", OTHER)]);
    let replay_two = repo.commit(
        &[replay_one],
        &[("src/lib.rs", HELPER_V2), ("src/other.rs", OTHER)],
    );
    let report = repo.fetched(&[replay_two, replay_one], &[replay_two]);
    let by_commit: BTreeMap<GitOid, Vec<TapeId>> = report
        .commits
        .iter()
        .map(|commit| (commit.commit, commit.attached.clone()))
        .collect();
    assert_eq!(by_commit[&replay_one], vec![one]);
    assert_eq!(by_commit[&replay_two], vec![two]);
}

#[test]
fn a_commit_never_published_is_reconstructed_as_before() {
    let repo = Repo::new();
    let (_, work, main) = rebase_setup(&repo);
    let rebased = repo.commit(&[main], &[("src/lib.rs", BASE_EDITED), ("src/other.rs", OTHER)]);
    let report = repo.rewritten(&[(work, rebased)]);
    assert!(report.commits[0].attached.is_empty());
    assert!(matches!(repo.events_of(rebased), CommitEvents::Reconstructed(_)));
}

#[test]
fn re_attachment_is_idempotent_and_skips_commits_with_tapes() {
    let repo = Repo::new();
    let (_, _, squash) = squash_setup(&repo);
    repo.fetched(&[squash], &[squash]);
    let entries = repo.index_entries();
    let again = repo.fetched(&[squash], &[squash]);
    assert_eq!(again.skipped, vec![squash]);
    assert_eq!(repo.index_entries(), entries);
}

#[test]
fn re_attachment_writes_only_index_entries() {
    let repo = Repo::new();
    let (_, _, squash) = squash_setup(&repo);
    let heads = |repo: &Repo| -> Vec<Option<GitOid>> {
        ["events", "tapes"]
            .iter()
            .map(|name| {
                let name = GedoRefName::collection(name).expect("valid");
                Collection::new(name, &repo.history.objects, &repo.refs)
                    .head()
                    .expect("head")
            })
            .collect()
    };
    let before = heads(&repo);
    repo.fetched(&[squash], &[squash]);
    assert_eq!(heads(&repo), before);
}

#[test]
fn the_result_does_not_depend_on_the_processing_order() {
    let render = |reverse: bool| {
        let repo = Repo::new();
        let root = repo.commit(&[], &[("src/lib.rs", BASE)]);
        let first = repo.commit(&[root], &[("src/lib.rs", HELPER_V1)]);
        let second = repo.commit(&[first], &[("src/lib.rs", HELPER_V2)]);
        let main = repo.commit(&[root], &[("src/lib.rs", BASE), ("src/other.rs", OTHER)]);
        repo.publish_live(first);
        repo.publish_live(second);
        let replay_one = repo.commit(&[main], &[("src/lib.rs", HELPER_V1), ("src/other.rs", OTHER)]);
        let replay_two = repo.commit(
            &[replay_one],
            &[("src/lib.rs", HELPER_V2), ("src/other.rs", OTHER)],
        );
        let squash = repo.commit(&[main], &[("src/lib.rs", HELPER_V2), ("src/other.rs", OTHER)]);
        let mut commits = vec![replay_one, replay_two, squash];
        if reverse {
            commits.reverse();
        }
        repo.fetched(&commits, &[replay_two, squash]);
        repo.index_entries()
    };
    let forward = render(false);
    assert_eq!(render(true), forward);
    insta::assert_snapshot!(forward);
}

#[test]
fn detached_tapes_are_reported_and_eligible_after_the_grace_period() {
    let repo = Repo::new();
    let (main, tapes, squash) = squash_setup(&repo);
    let start = 1_800_000_000;
    let report = repo.detached(&[main], start);
    assert_eq!(report.iter().map(|tape| tape.tape).collect::<Vec<_>>(), tapes);
    assert!(
        report
            .iter()
            .all(|tape| tape.detached_since == start && !tape.eligible)
    );
    let later = repo.detached(&[main], start + 89 * DAY);
    assert!(
        later
            .iter()
            .all(|tape| tape.detached_since == start && !tape.eligible)
    );
    let past = repo.detached(&[main], start + 90 * DAY);
    assert!(past.iter().all(|tape| tape.eligible));
    repo.fetched(&[squash], &[squash]);
    assert!(repo.detached(&[squash], start + 91 * DAY).is_empty());
}

#[test]
fn a_custom_grace_period_is_used() {
    let repo = Repo::new();
    let (main, _, _) = squash_setup(&repo);
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&repo.history.objects, &repo.refs, &mut wal)
        .expect("opens")
        .with_cache(&repo.events);
    log.detached(&[main], 1_000, 7).expect("report");
    let report = log.detached(&[main], 1_000 + 7 * DAY, 7).expect("report");
    assert!(report.iter().all(|tape| tape.eligible));
}

#[test]
fn without_a_cache_no_detached_tape_is_eligible() {
    let repo = Repo::new();
    let (main, _, _) = squash_setup(&repo);
    let mut wal = MemoryWal::new();
    let mut log = EventLog::open(&repo.history.objects, &repo.refs, &mut wal).expect("opens");
    log.detached(&[main], 1_000, DEFAULT_GRACE_DAYS).expect("report");
    let report = log
        .detached(&[main], 1_000 + 365 * DAY, DEFAULT_GRACE_DAYS)
        .expect("report");
    assert!(!report.is_empty());
    assert!(report.iter().all(|tape| !tape.eligible));
}

#[test]
fn a_reachable_branch_has_no_detached_tape() {
    let repo = Repo::new();
    let (_, work, main) = rebase_setup(&repo);
    repo.publish_live(work);
    assert!(repo.detached(&[work, main], 1_000).is_empty());
}

#[test]
fn filling_the_cache_reads_each_missing_event_once() {
    let repo = Repo::new();
    let (_, tapes, _) = squash_setup(&repo);
    let mut wal = MemoryWal::new();
    let log = EventLog::open(&repo.history.objects, &repo.refs, &mut wal)
        .expect("opens")
        .with_cache(&repo.events);
    let events: usize = tapes
        .iter()
        .map(|tape| log.tape(tape).expect("tape").events.len())
        .sum();
    let fill = log.fill_cache().expect("filled");
    assert_eq!(fill.listed, events);
    assert_eq!(fill.cached, events);
    let cached = tapes
        .iter()
        .flat_map(|tape| log.tape(tape).expect("tape").events)
        .filter(|id| {
            repo.events
                .get(EVENTS_CACHE_NAMESPACE, &id.0)
                .expect("read")
                .is_some()
        })
        .count();
    assert_eq!(cached, events);
    let again = log.fill_cache().expect("filled");
    assert_eq!(again.already_cached, events);
    assert_eq!(again.cached, 0);
}

#[test]
fn filling_without_a_cache_does_nothing() {
    let repo = Repo::new();
    squash_setup(&repo);
    let mut wal = MemoryWal::new();
    let log = EventLog::open(&repo.history.objects, &repo.refs, &mut wal).expect("opens");
    assert_eq!(log.fill_cache().expect("filled"), gedo_app::CacheFill::default());
}
