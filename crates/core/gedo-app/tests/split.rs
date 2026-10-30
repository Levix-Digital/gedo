//! Split publish (User Stories 1 to 3 of `specs/007-semantic-lanes`): the lanes of scripted work,
//! the refactor and feature commits, their tapes, and the fallback to one commit.

mod support;

use std::collections::BTreeMap;

use gedo_app::{CommitEvents, EventLog, IdentityIndex, NotSplit, SplitOutcome, SplitRequest};
use gedo_core::event::Labels;
use gedo_core::ids::GitOid;
use gedo_core::memory::{MemoryCache, MemoryObjectStore, MemoryRefStore, MemoryWal};
use gedo_core::ports::{EntryMode, ObjectStore, WriteAheadLog};
use gedo_crdt::{Lane, RecordingContext};
use gedo_parse::TreeSitterParser;
use support::{History, files};

struct Repo {
    history: History,
    identities: MemoryCache,
    refs: MemoryRefStore,
    wal: MemoryWal,
}

impl Repo {
    fn new() -> Self {
        Self {
            history: History::default(),
            identities: support::cache(),
            refs: MemoryRefStore::new(),
            wal: MemoryWal::new(),
        }
    }

    fn commit(&self, parents: &[GitOid], pairs: &[(&str, &str)]) -> GitOid {
        self.history.commit(parents, &files(pairs))
    }

    fn identities(&self) -> IdentityIndex<'_, MemoryObjectStore, TreeSitterParser, MemoryCache> {
        IdentityIndex::open(&self.history.objects, &self.history.parser, &self.identities).expect("opens")
    }

    fn tree_of(&self, commit: GitOid) -> GitOid {
        self.history.objects.read_commit(&commit).expect("commit").tree
    }

    /// Records the changes from `base` to `work` as live events of an agent, as a developer
    /// working with Gedo would.
    fn record(&mut self, base: GitOid, work: GitOid) {
        self.identities().index_history(&[work]).expect("indexed");
        let events = {
            let identities = self.identities();
            let mut wal = MemoryWal::new();
            let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
            match log.events_of_commit(&work, &identities).expect("events") {
                CommitEvents::Reconstructed(events) => events,
                CommitEvents::Published { .. } => panic!("already published"),
            }
        };
        let ops: Vec<_> = events
            .iter()
            .map(|event| (event.operation.clone(), event.result_hash))
            .collect();
        let context = RecordingContext::agent(
            "levi@example.com",
            gedo_core::event::AgentId::new("worker-1").expect("valid"),
            Labels::from_pairs(&[("dc.plan", "gedo_semantic_lanes")]).expect("valid"),
        )
        .expect("valid");
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut self.wal).expect("opens");
        log.record(&context, Some(base), &ops).expect("recorded");
    }

    fn split(&mut self, base: GitOid, work: GitOid) -> SplitOutcome {
        let request = SplitRequest {
            base,
            tree: self.tree_of(work),
            message: "feat: discounts".into(),
            refactor_message: None,
        };
        let mut identities =
            IdentityIndex::open(&self.history.objects, &self.history.parser, &self.identities)
                .expect("opens");
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut self.wal).expect("opens");
        log.publish_split(&request, &mut identities, &self.history.parser)
            .expect("published")
    }

    /// Every file of a commit's tree with its text.
    fn files_of(&self, commit: GitOid) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        self.collect(self.tree_of(commit), "", &mut out);
        out
    }

    fn collect(&self, tree: GitOid, prefix: &str, out: &mut BTreeMap<String, String>) {
        for entry in self.history.objects.read_tree(&tree).expect("tree") {
            let path = format!("{prefix}{}", entry.name);
            match entry.mode {
                EntryMode::Tree => self.collect(entry.oid, &format!("{path}/"), out),
                _ => {
                    let bytes = self.history.objects.read_blob(&entry.oid).expect("blob");
                    out.insert(path, String::from_utf8(bytes).expect("utf-8"));
                }
            }
        }
    }

    fn message(&self, commit: GitOid) -> String {
        self.history.objects.read_commit(&commit).expect("commit").message
    }

    fn parent(&self, commit: GitOid) -> GitOid {
        self.history.objects.read_commit(&commit).expect("commit").parents[0]
    }

    fn tape_events(&self, commit: GitOid) -> usize {
        let identities = self.identities();
        let mut wal = MemoryWal::new();
        let mut log = EventLog::open(&self.history.objects, &self.refs, &mut wal).expect("opens");
        match log.events_of_commit(&commit, &identities).expect("events") {
            CommitEvents::Published { tapes, .. } => tapes.iter().map(|(_, events)| events.len()).sum(),
            CommitEvents::Reconstructed(_) => 0,
        }
    }
}

const BASE: &str = "pub struct Invoice {
    pub amount: u32,
}

impl Invoice {
    pub fn sum(&self) -> u32 {
        self.amount
    }
}

pub fn report(invoice: &Invoice) -> u32 {
    invoice.sum() + 1
}

pub fn audit(invoice: &Invoice) -> u32 {
    invoice.sum()
}
";

fn split_parts(outcome: &SplitOutcome) -> (GitOid, GitOid) {
    match outcome {
        SplitOutcome::Split { refactor, feature } => (refactor.commit, feature.commit),
        other => panic!("not split: {other:?}"),
    }
}

#[test]
fn split_a_rename_with_its_callers_and_a_new_feature() {
    let mut repo = Repo::new();
    let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let work_text = BASE
        .replace("pub fn sum(&self)", "pub fn total(&self)")
        .replace("invoice.sum() + 1", "invoice.total() + 2")
        .replace("    invoice.sum()\n", "    invoice.total()\n")
        .replace(
            "pub fn audit",
            "pub fn discount(invoice: &Invoice) -> u32 {\n    invoice.total() / 10\n}\n\npub fn audit",
        );
    let work = repo.commit(&[base], &[("src/lib.rs", &work_text)]);
    repo.record(base, work);

    let outcome = repo.split(base, work);
    let (refactor, feature) = split_parts(&outcome);
    assert_eq!(repo.parent(refactor), base);
    assert_eq!(repo.parent(feature), refactor);
    assert_eq!(repo.tree_of(feature), repo.tree_of(work));
    let expected = BASE
        .replace("pub fn sum(&self)", "pub fn total(&self)")
        .replace("invoice.sum() + 1", "invoice.total() + 1")
        .replace("    invoice.sum()\n", "    invoice.total()\n");
    assert_eq!(repo.files_of(refactor)["src/lib.rs"], expected);
    assert_eq!(
        repo.message(refactor),
        "refactor: rename crate::Invoice::sum to total\n\nrename crate::Invoice::sum to total\n"
    );
    assert_eq!(repo.message(feature), "feat: discounts");
    assert!(repo.tape_events(refactor) > 0);
    assert!(repo.tape_events(feature) > 0);
    assert!(repo.wal.read_all().expect("log").is_empty());
}

#[test]
fn split_a_move_across_files_with_its_paths() {
    let mut repo = Repo::new();
    let lib = "pub mod billing;\n\npub fn charge() -> u32 {\n    billing::tax(1)\n}\n";
    let billing = "pub fn tax(x: u32) -> u32 {\n    x * 2\n}\n\npub fn fee() -> u32 {\n    3\n}\n";
    let base = repo.commit(&[], &[("src/lib.rs", lib), ("src/billing.rs", billing)]);
    let work_lib = "pub mod billing;\n\npub fn charge() -> u32 {\n    tax(1) + 1\n}\n\npub fn tax(x: u32) -> u32 {\n    x * 3\n}\n";
    let work_billing = "pub fn fee() -> u32 {\n    3\n}\n";
    let work = repo.commit(
        &[base],
        &[("src/lib.rs", work_lib), ("src/billing.rs", work_billing)],
    );
    repo.record(base, work);

    let outcome = repo.split(base, work);
    let (refactor, feature) = split_parts(&outcome);
    assert_eq!(repo.tree_of(feature), repo.tree_of(work));
    let files = repo.files_of(refactor);
    assert_eq!(
        files["src/lib.rs"],
        "pub mod billing;\n\npub fn charge() -> u32 {\n    tax(1)\n}\n\npub fn tax(x: u32) -> u32 {\n    x * 2\n}\n"
    );
    assert_eq!(files["src/billing.rs"], work_billing);
}

#[test]
fn split_work_with_one_lane_is_one_commit() {
    let mut repo = Repo::new();
    let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let work = repo.commit(&[base], &[("src/lib.rs", &BASE.replace("+ 1", "+ 5"))]);
    repo.record(base, work);
    match repo.split(base, work) {
        SplitOutcome::Single { commit, reason } => {
            assert_eq!(reason, NotSplit::OneLane(Lane::Feature));
            assert_eq!(repo.tree_of(commit.commit), repo.tree_of(work));
            assert_eq!(repo.parent(commit.commit), base);
            assert!(commit.tape.is_some());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn split_nothing_to_publish() {
    let mut repo = Repo::new();
    let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
    repo.identities().index_history(&[base]).expect("indexed");
    assert_eq!(repo.split(base, base), SplitOutcome::Nothing);
}

#[test]
fn split_is_deterministic() {
    let run = || {
        let mut repo = Repo::new();
        let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
        let work = repo.commit(
            &[base],
            &[("src/lib.rs", &BASE.replace("sum", "total").replace("+ 1", "+ 2"))],
        );
        repo.record(base, work);
        let outcome = repo.split(base, work);
        let (refactor, feature) = split_parts(&outcome);
        (outcome.clone(), repo.files_of(refactor), repo.files_of(feature))
    };
    assert_eq!(run(), run());
}

#[test]
fn split_a_module_split_lands_whole_in_the_refactor_commit() {
    let mut repo = Repo::new();
    let lib = "pub mod billing;\n\npub fn charge() -> u32 {\n    billing::tax(1)\n}\n";
    let billing = "pub fn tax(x: u32) -> u32 {\n    x * 2\n}\n\npub fn fee() -> u32 {\n    3\n}\n";
    let base = repo.commit(&[], &[("src/lib.rs", lib), ("src/billing.rs", billing)]);
    let work_lib = "pub mod billing;\n\npub fn charge() -> u32 {\n    billing::tax::tax(1)\n}\n";
    let work_billing = "pub mod tax;\n\npub fn fee() -> u32 {\n    4\n}\n";
    let work_tax = "pub fn tax(x: u32) -> u32 {\n    x * 2\n}\n";
    let work = repo.commit(
        &[base],
        &[
            ("src/lib.rs", work_lib),
            ("src/billing.rs", work_billing),
            ("src/billing/tax.rs", work_tax),
        ],
    );
    repo.record(base, work);

    let outcome = repo.split(base, work);
    let (refactor, feature) = split_parts(&outcome);
    assert_eq!(repo.tree_of(feature), repo.tree_of(work));
    let files = repo.files_of(refactor);
    assert_eq!(files["src/lib.rs"], work_lib);
    assert_eq!(
        files["src/billing.rs"],
        "pub mod tax;\n\npub fn fee() -> u32 {\n    3\n}\n"
    );
    assert_eq!(files["src/billing/tax.rs"], work_tax);
}

#[test]
fn split_events_go_to_the_tape_of_their_lane() {
    let mut repo = Repo::new();
    let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let work_text = BASE.replace("sum", "total").replace("+ 1", "+ 2").replace(
        "pub fn audit",
        "pub fn discount() -> u32 {\n    1\n}\n\npub fn audit",
    );
    let work = repo.commit(&[base], &[("src/lib.rs", &work_text)]);
    repo.record(base, work);
    match repo.split(base, work) {
        SplitOutcome::Split { refactor, feature } => {
            // The rename and the caller that only follows it; the new function and the edited caller.
            assert_eq!((refactor.events, feature.events), (2, 2));
            assert_eq!(repo.tape_events(refactor.commit), 2);
            assert_eq!(repo.tape_events(feature.commit), 2);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn split_falls_back_to_one_commit_when_the_refactor_version_does_not_parse() {
    let mut repo = Repo::new();
    // The base has a syntax error that the feature work fixes: the refactor version keeps it.
    let broken = BASE.replace("    invoice.sum()\n}", "    invoice.sum() +\n}");
    let base = repo.commit(&[], &[("src/lib.rs", &broken)]);
    let work_text = BASE.replace("sum", "total");
    let work = repo.commit(&[base], &[("src/lib.rs", &work_text)]);
    repo.record(base, work);
    match repo.split(base, work) {
        SplitOutcome::Single { commit, reason } => {
            assert_eq!(
                reason,
                NotSplit::SyntaxError {
                    path: "src/lib.rs".into()
                }
            );
            assert_eq!(repo.tree_of(commit.commit), repo.tree_of(work));
            assert_eq!(repo.parent(commit.commit), base);
            assert!(commit.tape.is_some());
            assert!(repo.wal.read_all().expect("log").is_empty());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn split_with_feature_work_that_leaves_a_syntax_error_still_ends_at_the_final_tree() {
    let mut repo = Repo::new();
    let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let work_text = BASE.replace("sum", "total").replace(
        "pub fn audit(invoice: &Invoice) -> u32 {",
        "pub fn audit(invoice: &Invoice) -> u32 { (",
    );
    let work = repo.commit(&[base], &[("src/lib.rs", &work_text)]);
    repo.record(base, work);
    let tip = repo.split(base, work).tip().expect("a commit");
    assert_eq!(repo.tree_of(tip), repo.tree_of(work));
}

/// Work generated from `BASE` by a seed: any mix of a rename, a caller edit, a new function, a
/// removal and a move to another file.
fn generated_work(seed: u64) -> Vec<(String, String)> {
    let mut text = BASE.to_owned();
    let mut other = None;
    if seed & 1 != 0 {
        text = text.replace("sum", "total");
    }
    if seed & 2 != 0 {
        text = text.replace("+ 1", "+ 7");
    }
    if seed & 4 != 0 {
        text.push_str("\npub fn extra() -> u32 {\n    9\n}\n");
    }
    if seed & 8 != 0 {
        let start = text.find("pub fn audit").expect("audit");
        let audit = text[start..].to_owned();
        text.truncate(start);
        if seed & 16 != 0 {
            other = Some(format!("use crate::Invoice;\n\n{audit}"));
            text = format!("pub mod checks;\n\n{text}");
        }
    }
    let mut files = vec![("src/lib.rs".to_owned(), text)];
    if let Some(other) = other {
        files.push(("src/checks.rs".to_owned(), other));
    }
    files
}

#[test]
fn split_always_ends_at_the_final_tree() {
    for seed in 0..32 {
        let mut repo = Repo::new();
        let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
        let work_files = generated_work(seed);
        let pairs: Vec<(&str, &str)> = work_files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect();
        let work = repo.commit(&[base], &pairs);
        repo.record(base, work);
        let outcome = repo.split(base, work);
        match &outcome {
            SplitOutcome::Nothing => assert_eq!(repo.tree_of(work), repo.tree_of(base), "seed {seed}"),
            SplitOutcome::Single { commit, reason } => {
                assert_eq!(repo.tree_of(commit.commit), repo.tree_of(work), "seed {seed}");
                // Every generated mix of the two lanes splits.
                assert!(matches!(reason, NotSplit::OneLane(_)), "seed {seed}: {reason:?}");
            }
            SplitOutcome::Split { refactor, feature } => {
                assert_eq!(repo.tree_of(feature.commit), repo.tree_of(work), "seed {seed}");
                assert_eq!(repo.parent(feature.commit), refactor.commit);
                assert_eq!(repo.parent(refactor.commit), base);
            }
        }
        assert!(repo.wal.read_all().expect("log").is_empty(), "seed {seed}");
    }
}

/// The lanes of the changes from `base` to `work`, by kind of delta and qualified path.
fn lanes_of(repo: &Repo, base: GitOid, work: GitOid) -> Vec<(String, String, Lane)> {
    use gedo_core::identity::SemanticDelta;
    use gedo_core::ports::Parser;
    repo.identities().index_history(&[work]).expect("indexed");
    let identities = repo.identities();
    let deltas = identities.changes_of(&work).expect("changes").deltas;
    let mut owned = Vec::new();
    for path in identities.changed_files(&work).expect("files") {
        let mut sides = Vec::new();
        for commit in [base, work] {
            let text = repo.files_of(commit).get(&path).cloned();
            let map = identities.map_at(&commit, &path).expect("indexed");
            sides.push(text.zip(map).map(|(text, map)| {
                let parsed = repo.history.parser.parse(&path, text.as_bytes()).expect("parses");
                (text.into_bytes(), parsed, map)
            }));
        }
        owned.push((path, sides));
    }
    let versions: Vec<gedo_crdt::FileVersions<'_>> = owned
        .iter()
        .map(|(path, sides)| {
            let view = |index: usize| {
                sides[index]
                    .as_ref()
                    .map(|(source, parsed, map)| gedo_crdt::FileVersion { source, parsed, map })
            };
            gedo_crdt::FileVersions {
                path: path.clone(),
                before: view(0),
                after: view(1),
            }
        })
        .collect();
    let plan = gedo_crdt::classify(&deltas, &versions);
    let name = |id| {
        let identities = repo.identities();
        [work, base]
            .iter()
            .flat_map(|commit| {
                identities
                    .files_at(commit)
                    .expect("indexed")
                    .into_iter()
                    .filter_map(|path| identities.map_at(commit, &path).expect("indexed"))
                    .flat_map(|map| map.nodes)
                    .collect::<Vec<_>>()
            })
            .find(|node| node.id == id)
            .map(|node| node.qualified_path)
            .unwrap_or_default()
    };
    let mut lanes: Vec<(String, String, Lane)> = plan
        .changes
        .iter()
        .map(|change| {
            let kind = match &change.delta {
                SemanticDelta::Created { .. } => "created",
                SemanticDelta::Moved { .. } => "moved",
                SemanticDelta::Renamed { .. } => "renamed",
                SemanticDelta::SignatureMutated { .. } => "signature",
                SemanticDelta::BodyModified { .. } => "body",
                SemanticDelta::Tombstoned { .. } => "removed",
            };
            (kind.to_owned(), name(change.delta.id()), change.lane)
        })
        .collect();
    lanes.sort();
    lanes
}

#[test]
fn lanes_of_renames_follow_ups_and_feature_work() {
    let repo = Repo::new();
    let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let work_text = BASE
        .replace("sum", "total")
        .replace("+ 1", "+ 2")
        .replace("-> u32 {\n    invoice.total()", "-> u64 {\n    invoice.total()")
        .replace(
            "pub fn audit",
            "pub fn discount() -> u32 {\n    1\n}\n\npub fn audit",
        );
    let work = repo.commit(&[base], &[("src/lib.rs", &work_text)]);
    let refactor = Lane::Refactor;
    let feature = Lane::Feature;
    assert_eq!(
        lanes_of(&repo, base, work),
        [
            ("body".to_owned(), "crate::audit".to_owned(), refactor),
            ("body".to_owned(), "crate::report".to_owned(), feature),
            ("created".to_owned(), "crate::discount".to_owned(), feature),
            ("renamed".to_owned(), "crate::Invoice::total".to_owned(), refactor),
            ("signature".to_owned(), "crate::audit".to_owned(), feature),
            ("signature".to_owned(), "crate::report".to_owned(), feature),
        ]
    );
    // The same work gives the same lanes.
    assert_eq!(lanes_of(&repo, base, work), lanes_of(&repo, base, work));
}

#[test]
fn split_outcome_is_pinned() {
    let mut repo = Repo::new();
    let base = repo.commit(&[], &[("src/lib.rs", BASE)]);
    let work_text = BASE.replace("sum", "total").replace("+ 1", "+ 2").replace(
        "pub fn audit",
        "pub fn discount() -> u32 {\n    1\n}\n\npub fn audit",
    );
    let work = repo.commit(&[base], &[("src/lib.rs", &work_text)]);
    repo.record(base, work);
    let (refactor, feature) = split_parts(&repo.split(base, work));
    let mut text = String::new();
    for commit in [refactor, feature] {
        text.push_str(&format!("commit {commit}\nmessage {:?}\n", repo.message(commit)));
        for (path, content) in repo.files_of(commit) {
            text.push_str(&format!("--- {path}\n{content}"));
        }
    }
    insta::assert_snapshot!("split_outcome", text);
}
