//! Convergence of commits over their common base (User Story 5 of `specs/006-tree-crdt`): scripted
//! branches, published and reconstructed events, a graft, live events on top of a side, errors.

mod support;

use gedo_app::{CommitEvents, ConvergeReport, EventError, EventLog, IdentityIndex, IndexError, Side};
use gedo_core::event::{EventId, Labels};
use gedo_core::ids::GitOid;
use gedo_core::memory::{MemoryCache, MemoryRefStore, MemoryWal};
use gedo_core::ports::ObjectStore;
use gedo_crdt::{ConflictKind, EntityState, Part, RecordingContext, event_id};
use support::{History, files};

const ROOT: &str = "pub struct Invoice;

impl Invoice {
    pub fn total(&self) -> u32 {
        1
    }
}

pub fn f() -> u32 {
    1
}

pub fn g() -> u32 {
    2
}
";

/// `ROOT` with `edit` applied to the text.
fn edited(edits: &[(&str, &str)]) -> String {
    edits.iter().fold(ROOT.to_owned(), |text, (from, to)| {
        assert!(text.contains(from), "{from:?} not in the text");
        text.replacen(from, to, 1)
    })
}

const TOTAL: &str = "    pub fn total(&self) -> u32 {\n        1\n    }\n";

fn with_method(name: &str, value: u32) -> (String, String) {
    (
        TOTAL.to_owned(),
        format!("{TOTAL}\n    pub fn {name}(&self) -> u32 {{\n        {value}\n    }}\n"),
    )
}

struct Repo {
    history: History,
    identities: MemoryCache,
    refs: MemoryRefStore,
}

impl Repo {
    fn new() -> Self {
        Self {
            history: History::default(),
            identities: support::cache(),
            refs: MemoryRefStore::new(),
        }
    }

    fn commit(&self, parents: &[GitOid], text: &str) -> GitOid {
        self.history.commit(parents, &files(&[("src/lib.rs", text)]))
    }

    fn identities(
        &self,
    ) -> IdentityIndex<'_, gedo_core::memory::MemoryObjectStore, gedo_parse::TreeSitterParser, MemoryCache>
    {
        IdentityIndex::open(&self.history.objects, &self.history.parser, &self.identities).expect("opens")
    }

    fn index(&self, tips: &[GitOid]) {
        self.identities().index_history(tips).expect("indexed");
    }

    fn try_converge(&self, base: GitOid, sides: &[Side]) -> Result<ConvergeReport, EventError> {
        let identities = self.identities();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
        log.converge(&base, sides, &identities)
    }

    fn converge(&self, base: GitOid, tips: &[GitOid]) -> ConvergeReport {
        self.index(tips);
        let sides: Vec<Side> = tips.iter().map(|tip| Side::commit(*tip)).collect();
        self.try_converge(base, &sides).expect("converges")
    }

    fn events_of(&self, commit: GitOid) -> CommitEvents {
        let identities = self.identities();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
        log.events_of_commit(&commit, &identities).expect("events")
    }

    /// Publishes the changes of `commit` as live events of an agent, as a developer working with
    /// Gedo would.
    fn publish_live(&self, commit: GitOid) -> Vec<EventId> {
        self.index(&[commit]);
        let CommitEvents::Reconstructed(events) = self.events_of(commit) else {
            panic!("{commit} is already published");
        };
        let ops: Vec<_> = events
            .iter()
            .map(|event| (event.operation.clone(), event.result_hash))
            .collect();
        let identities = self.identities();
        drop(identities);
        let parent = self
            .history
            .objects
            .read_commit(&commit)
            .expect("commit")
            .parents
            .first()
            .copied();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
        let context = RecordingContext::agent(
            "levi@example.com",
            gedo_core::event::AgentId::new("worker-1").expect("valid"),
            Labels::from_pairs(&[("dc.plan", "gedo_tree_crdt")]).expect("valid"),
        )
        .expect("valid");
        log.record(&context, parent, &ops).expect("recorded");
        log.publish(commit).expect("published").expect("a tape");
        match self.events_of(commit) {
            CommitEvents::Published { tapes, .. } => tapes
                .iter()
                .flat_map(|(_, events)| events.iter().map(event_id))
                .collect(),
            CommitEvents::Reconstructed(_) => panic!("not published"),
        }
    }
}

/// The present entity with this qualified path.
fn find<'a>(report: &'a ConvergeReport, qualified_path: &str) -> Option<&'a EntityState> {
    report
        .convergence
        .state
        .entities()
        .values()
        .find(|entity| entity.present && entity.qualified_path() == qualified_path)
}

#[test]
fn methods_added_to_the_same_impl_on_two_branches_both_converge() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let (from, to) = with_method("tax", 2);
    let ours = repo.commit(&[root], &edited(&[(&from, &to)]));
    let (from, to) = with_method("due", 3);
    let theirs = repo.commit(&[root], &edited(&[(&from, &to)]));
    let report = repo.converge(root, &[ours, theirs]);
    assert!(
        report.convergence.is_clean(),
        "{:?}",
        report.convergence.conflicts
    );
    assert!(find(&report, "crate::Invoice::tax").is_some());
    assert!(find(&report, "crate::Invoice::due").is_some());
    assert!(find(&report, "crate::Invoice::total").is_some());
    assert_eq!(report.commits, vec![vec![ours], vec![theirs]]);
}

#[test]
fn two_bodies_of_one_function_conflict() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let ours = repo.commit(
        &[root],
        &edited(&[("    1\n}\n\npub fn g", "    5\n}\n\npub fn g")]),
    );
    let theirs = repo.commit(
        &[root],
        &edited(&[("    1\n}\n\npub fn g", "    6\n}\n\npub fn g")]),
    );
    let report = repo.converge(root, &[ours, theirs]);
    let conflicts = &report.convergence.conflicts;
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].kind, ConflictKind::Part(Part::Body));
    assert_eq!(conflicts[0].values.len(), 2);
}

#[test]
fn a_removal_against_an_edit_conflicts() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let ours = repo.commit(&[root], &edited(&[("\npub fn g() -> u32 {\n    2\n}\n", "")]));
    let theirs = repo.commit(&[root], &edited(&[("    2\n}", "    9\n}")]));
    let report = repo.converge(root, &[ours, theirs]);
    let conflicts = &report.convergence.conflicts;
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].kind, ConflictKind::Part(Part::Presence));
    assert!(find(&report, "crate::g").is_some(), "kept with its base content");
}

#[test]
fn the_same_change_on_both_branches_applies_once() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let fix = edited(&[("    1\n}\n\npub fn g", "    5\n}\n\npub fn g")]);
    let ours = repo.commit(&[root], &fix);
    let (from, to) = with_method("tax", 2);
    let theirs_first = repo.commit(&[root], &edited(&[(&from, &to)]));
    let theirs = repo.commit(&[theirs_first], &fix.replacen(&from, &to, 1));
    let report = repo.converge(root, &[ours, theirs]);
    assert!(
        report.convergence.is_clean(),
        "{:?}",
        report.convergence.conflicts
    );
    assert_eq!(report.commits[1], vec![theirs_first, theirs]);
}

#[test]
fn published_events_name_the_conflict_with_their_live_ids() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let ours = repo.commit(
        &[root],
        &edited(&[("    1\n}\n\npub fn g", "    5\n}\n\npub fn g")]),
    );
    let theirs = repo.commit(
        &[root],
        &edited(&[("    1\n}\n\npub fn g", "    6\n}\n\npub fn g")]),
    );
    let live = repo.publish_live(ours);
    let published = repo.converge(root, &[ours, theirs]);

    let fresh = Repo::new();
    let root2 = fresh.commit(&[], ROOT);
    let ours2 = fresh.commit(
        &[root2],
        &edited(&[("    1\n}\n\npub fn g", "    5\n}\n\npub fn g")]),
    );
    let theirs2 = fresh.commit(
        &[root2],
        &edited(&[("    1\n}\n\npub fn g", "    6\n}\n\npub fn g")]),
    );
    let reconstructed = fresh.converge(root2, &[ours2, theirs2]);

    assert_eq!(published.convergence.state, reconstructed.convergence.state);
    let ours_events = &published.convergence.conflicts[0]
        .values
        .iter()
        .find(|value| value.sides == [0])
        .expect("ours")
        .events;
    assert!(
        live.contains(&ours_events[0]),
        "the conflict names the live event"
    );
}

#[test]
fn live_events_on_top_of_a_side_take_part() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let ours = repo.commit(
        &[root],
        &edited(&[("    1\n}\n\npub fn g", "    5\n}\n\npub fn g")]),
    );
    let ours_next = repo.commit(
        &[ours],
        &edited(&[
            ("    1\n}\n\npub fn g", "    5\n}\n\npub fn g"),
            ("    2\n}", "    7\n}"),
        ]),
    );
    let theirs = repo.commit(&[root], &edited(&[("    2\n}", "    8\n}")]));
    repo.index(&[ours_next, theirs]);
    let live = repo.events_of(ours_next).events().into_iter().cloned().collect();
    let with_live = repo
        .try_converge(root, &[Side { tip: ours, live }, Side::commit(theirs)])
        .expect("converges");
    let committed = repo.converge(root, &[ours_next, theirs]);
    assert_eq!(with_live.convergence, committed.convergence);
    assert_eq!(
        with_live.convergence.conflicts.len(),
        1,
        "g changed on both sides"
    );
}

#[test]
fn a_base_not_indexed_is_an_error() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let ours = repo.commit(&[root], &edited(&[("    2\n}", "    8\n}")]));
    let error = repo
        .try_converge(root, &[Side::commit(ours)])
        .expect_err("not indexed");
    assert!(
        matches!(error, EventError::Index(IndexError::NotIndexed { .. })),
        "{error}"
    );
}

#[test]
fn a_convergence_is_pinned() {
    let repo = Repo::new();
    let root = repo.commit(&[], ROOT);
    let (from, to) = with_method("tax", 2);
    let ours = repo.commit(
        &[root],
        &edited(&[(&from, &to), ("    1\n}\n\npub fn g", "    5\n}\n\npub fn g")]),
    );
    let theirs = repo.commit(
        &[root],
        &edited(&[
            ("    1\n}\n\npub fn g", "    6\n}\n\npub fn g"),
            ("\npub fn g() -> u32 {\n    2\n}\n", ""),
        ]),
    );
    let report = repo.converge(root, &[ours, theirs]);
    insta::assert_debug_snapshot!(report.convergence);
}
