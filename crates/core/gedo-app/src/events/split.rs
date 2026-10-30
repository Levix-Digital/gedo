//! Split publish: mixed work published as a refactor commit followed by a feature commit (FEAT-067,
//! `specs/007-semantic-lanes/research.md` R6 and R7).
//!
//! The single commit (the final tree on the base) is written and indexed first: its deltas are the
//! changes to classify, and it is what is published when the work has one lane or the split cannot
//! be verified. The refactor commit is built from the base and final sources, the feature commit has
//! the final tree; both are indexed, and the split is published only when the parts each one
//! changes are exactly its lane's. No branch is moved: the caller does that (FEAT-083).

use std::collections::{BTreeMap, BTreeSet};

use gedo_core::event::TapeId;
use gedo_core::identity::{AstNodeId, BlobAstMap};
use gedo_core::ids::GitOid;
use gedo_core::ports::{
    EntryMode, NewCommit, ObjectStore, Parser, RefStore, SymbolCache, TreeEntry, WriteAheadLog,
};
use gedo_core::syntax::ParsedFile;
use gedo_crdt::{
    BuildError, FileVersion, FileVersions, Lane, LanePlan, Part, build, classify, event_lane, part_of,
    refactor_message,
};

use super::{Context, EventError, EventLog};
use crate::index::IdentityIndex;

/// What to split-publish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitRequest {
    /// The commit the work started from.
    pub base: GitOid,
    /// The final tree.
    pub tree: GitOid,
    /// The message of the feature commit, or of the single commit.
    pub message: String,
    /// The message of the refactor commit; generated when `None`.
    pub refactor_message: Option<String>,
}

/// One commit written by a split publish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedCommit {
    /// The commit.
    pub commit: GitOid,
    /// Its tape; `None` when no event went to it.
    pub tape: Option<TapeId>,
    /// The changes it holds.
    pub changes: usize,
    /// The events of its tape.
    pub events: usize,
}

/// Why the work was published as one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotSplit {
    /// Every change is in one lane.
    OneLane(Lane),
    /// The refactor version could not be built.
    Build(BuildError),
    /// A file of the refactor version does not parse.
    SyntaxError {
        /// The file.
        path: String,
    },
    /// A commit of the split changes a part its lane does not hold, or misses one it holds.
    Mismatch {
        /// The entity.
        id: AstNodeId,
        /// The part.
        part: Part,
        /// The lane the part is in; `None` when the publish does not change it.
        expected: Option<Lane>,
    },
}

/// What a split publish did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SplitOutcome {
    /// A refactor commit on the base and a feature commit on it.
    Split {
        /// The refactor commit.
        refactor: PublishedCommit,
        /// The feature commit, whose tree is the final tree.
        feature: PublishedCommit,
    },
    /// One commit with every change and event.
    Single {
        /// The commit.
        commit: PublishedCommit,
        /// Why it was not split.
        reason: NotSplit,
    },
    /// The final tree is the base tree and the log is empty.
    Nothing,
}

impl SplitOutcome {
    /// The last commit written: the branch's new tip.
    pub fn tip(&self) -> Option<GitOid> {
        match self {
            SplitOutcome::Split { feature, .. } => Some(feature.commit),
            SplitOutcome::Single { commit, .. } => Some(commit.commit),
            SplitOutcome::Nothing => None,
        }
    }
}

/// The two versions of the changed files, read and parsed.
struct Sources {
    files: Vec<(String, Option<Version>, Option<Version>)>,
}

struct Version {
    source: Vec<u8>,
    parsed: ParsedFile,
    map: BlobAstMap,
}

impl Sources {
    fn versions(&self) -> Vec<FileVersions<'_>> {
        fn view(version: &Option<Version>) -> Option<FileVersion<'_>> {
            version.as_ref().map(|version| FileVersion {
                source: &version.source,
                parsed: &version.parsed,
                map: &version.map,
            })
        }
        self.files
            .iter()
            .map(|(path, before, after)| FileVersions {
                path: path.clone(),
                before: view(before),
                after: view(after),
            })
            .collect()
    }
}

impl<O: ObjectStore, R: RefStore, W: WriteAheadLog> EventLog<'_, O, R, W> {
    /// Publishes the work from `request.base` to `request.tree` as a refactor commit and a feature
    /// commit, or as one commit when it has one lane or the split cannot be verified (FR-005 to
    /// FR-015). Every commit written is indexed in `identities`; no branch is moved.
    pub fn publish_split<P, C>(
        &mut self,
        request: &SplitRequest,
        identities: &mut IdentityIndex<'_, O, P, C>,
        parser: &P,
    ) -> Result<SplitOutcome, EventError>
    where
        P: Parser + Sync,
        C: SymbolCache,
    {
        let base_tree = self
            .objects
            .read_commit(&request.base)
            .context(|| format!("read commit {}", request.base))?
            .tree;
        let pending = self.pending_unique()?;
        if base_tree == request.tree && pending.is_none() {
            return Ok(SplitOutcome::Nothing);
        }

        let single = self.write_commit(request.tree, request.base, &request.message)?;
        identities.index_history(&[single])?;
        let deltas = identities.changes_of(&single)?.deltas;
        let sources = self.sources(request.base, single, identities, parser)?;
        let versions = sources.versions();
        let plan = classify(&deltas, &versions);

        let attempt = if !plan.has(Lane::Refactor) || !plan.has(Lane::Feature) {
            let lane = if plan.has(Lane::Refactor) {
                Lane::Refactor
            } else {
                Lane::Feature
            };
            Err(NotSplit::OneLane(lane))
        } else {
            self.try_split(request, base_tree, &plan, &versions, identities, parser)?
        };

        let resolve = |id| identities.resolve(id);
        let (last_sequence, events) = match pending {
            Some((sequence, events)) => (Some(sequence), events),
            None => (None, Vec::new()),
        };
        let outcome = match attempt {
            Ok((refactor, feature)) => {
                let (refactor_events, feature_events): (Vec<_>, Vec<_>) = events
                    .into_iter()
                    .partition(|(_, event)| event_lane(&plan, event, resolve) == Lane::Refactor);
                let counts = (refactor_events.len(), feature_events.len());
                let tapes = match last_sequence {
                    Some(sequence) => self.store_tapes(
                        &[(refactor, refactor_events), (feature, feature_events)],
                        sequence,
                    )?,
                    None => vec![None, None],
                };
                SplitOutcome::Split {
                    refactor: PublishedCommit {
                        commit: refactor,
                        tape: tapes[0],
                        changes: plan.parts(Lane::Refactor).len(),
                        events: counts.0,
                    },
                    feature: PublishedCommit {
                        commit: feature,
                        tape: tapes[1],
                        changes: plan.parts(Lane::Feature).len(),
                        events: counts.1,
                    },
                }
            }
            Err(reason) => {
                let count = events.len();
                let tape = match last_sequence {
                    Some(sequence) => self.store_tapes(&[(single, events)], sequence)?[0],
                    None => None,
                };
                SplitOutcome::Single {
                    commit: PublishedCommit {
                        commit: single,
                        tape,
                        changes: plan.changes.len(),
                        events: count,
                    },
                    reason,
                }
            }
        };
        match &outcome {
            SplitOutcome::Split { refactor, feature } => tracing::info!(
                base = %request.base,
                refactor = %refactor.commit,
                feature = %feature.commit,
                refactor_changes = refactor.changes,
                feature_changes = feature.changes,
                refactor_events = refactor.events,
                feature_events = feature.events,
                "split published"
            ),
            SplitOutcome::Single { commit, reason } => tracing::info!(
                base = %request.base,
                commit = %commit.commit,
                changes = commit.changes,
                events = commit.events,
                reason = ?reason,
                "split published as one commit"
            ),
            SplitOutcome::Nothing => {}
        }
        Ok(outcome)
    }

    /// Builds, writes, indexes and verifies the two commits; the reason when the split is not safe.
    #[allow(clippy::type_complexity)]
    fn try_split<P, C>(
        &mut self,
        request: &SplitRequest,
        base_tree: GitOid,
        plan: &LanePlan,
        versions: &[FileVersions<'_>],
        identities: &mut IdentityIndex<'_, O, P, C>,
        parser: &P,
    ) -> Result<Result<(GitOid, GitOid), NotSplit>, EventError>
    where
        P: Parser + Sync,
        C: SymbolCache,
    {
        let files = match build(versions, plan) {
            Ok(files) => files,
            Err(error) => return Ok(Err(NotSplit::Build(error))),
        };
        for file in &files {
            if let Some(text) = &file.text {
                match parser.parse(&file.path, text) {
                    Ok(parsed) if !parsed.has_error => {}
                    _ => {
                        return Ok(Err(NotSplit::SyntaxError {
                            path: file.path.clone(),
                        }));
                    }
                }
            }
        }
        // One batch, so a large refactor is written as one pack (ADR-0015).
        let texts: Vec<&[u8]> = files.iter().filter_map(|file| file.text.as_deref()).collect();
        let mut blobs = self
            .objects
            .write_blobs(&texts)
            .context(|| "write the refactor files".into())?
            .into_iter();
        let replacements: BTreeMap<String, Option<GitOid>> = files
            .iter()
            .map(|file| (file.path.clone(), file.text.as_ref().and_then(|_| blobs.next())))
            .collect();
        let refactor_tree = self.replace_files(Some(base_tree), &replacements)?;
        tracing::debug!(files = files.len(), "refactor tree built");
        let refactor_tree = refactor_tree.unwrap_or(self.empty_tree()?);
        let message = request
            .refactor_message
            .clone()
            .unwrap_or_else(|| refactor_message(plan));
        let refactor = self.write_commit(refactor_tree, request.base, &message)?;
        let feature = self.write_commit(request.tree, refactor, &request.message)?;
        identities.index_history(&[feature])?;

        for (commit, lane) in [(refactor, Lane::Refactor), (feature, Lane::Feature)] {
            let changed: BTreeSet<(AstNodeId, Part)> = identities
                .changes_of(&commit)?
                .deltas
                .iter()
                .map(|delta| (identities.resolve(delta.id()), part_of(delta)))
                .collect();
            let expected = plan.parts(lane);
            // A container's body changes with the entities it holds, in either commit.
            let relevant = |(id, part): &(AstNodeId, Part)| !(*part == Part::Body && plan.is_container(*id));
            if let Some((id, part)) = changed
                .symmetric_difference(&expected)
                .find(|entry| relevant(entry))
            {
                return Ok(Err(NotSplit::Mismatch {
                    id: *id,
                    part: *part,
                    expected: plan.lane_of(*id, *part),
                }));
            }
        }
        Ok(Ok((refactor, feature)))
    }

    fn write_commit(&self, tree: GitOid, parent: GitOid, message: &str) -> Result<GitOid, EventError> {
        self.objects
            .write_commit(&NewCommit {
                tree,
                parents: vec![parent],
                message: message.to_owned(),
            })
            .context(|| "write a commit".into())
    }

    fn empty_tree(&self) -> Result<GitOid, EventError> {
        self.objects
            .write_tree(&[])
            .context(|| "write the empty tree".into())
    }

    /// Reads and parses both versions of every Rust file `single` changes against `base`.
    fn sources<P, C>(
        &self,
        base: GitOid,
        single: GitOid,
        identities: &IdentityIndex<'_, O, P, C>,
        parser: &P,
    ) -> Result<Sources, EventError>
    where
        P: Parser + Sync,
        C: SymbolCache,
    {
        let trees = [
            self.objects
                .read_commit(&base)
                .context(|| format!("read commit {base}"))?
                .tree,
            self.objects
                .read_commit(&single)
                .context(|| format!("read commit {single}"))?
                .tree,
        ];
        let mut files = Vec::new();
        // Every changed file walks the same trees, so each is read once.
        let mut read = BTreeMap::new();
        for path in identities.changed_files(&single)? {
            let mut sides = [None, None];
            for (side, (commit, tree)) in [(base, trees[0]), (single, trees[1])].into_iter().enumerate() {
                let Some(map) = identities.map_at(&commit, &path)? else {
                    continue;
                };
                let Some(blob) = self.blob_at(tree, &path, &mut read)? else {
                    continue;
                };
                let source = self
                    .objects
                    .read_blob(&blob)
                    .context(|| format!("read {path} ({blob})"))?;
                let Ok(parsed) = parser.parse(&path, &source) else {
                    continue;
                };
                sides[side] = Some(Version { source, parsed, map });
            }
            let [before, after] = sides;
            files.push((path, before, after));
        }
        Ok(Sources { files })
    }

    /// The blob at `path` in `tree`.
    fn blob_at(
        &self,
        tree: GitOid,
        path: &str,
        read: &mut BTreeMap<GitOid, Vec<TreeEntry>>,
    ) -> Result<Option<GitOid>, EventError> {
        let mut current = tree;
        let mut segments = path.split('/').peekable();
        while let Some(segment) = segments.next() {
            let entries = match read.entry(current) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => entry.insert(
                    self.objects
                        .read_tree(&current)
                        .context(|| format!("read tree {current}"))?,
                ),
            };
            let Some(entry) = entries.iter().find(|entry| entry.name == segment) else {
                return Ok(None);
            };
            match (segments.peek(), entry.mode) {
                (None, EntryMode::Blob | EntryMode::ExecutableBlob) => return Ok(Some(entry.oid)),
                (Some(_), EntryMode::Tree) => current = entry.oid,
                _ => return Ok(None),
            }
        }
        Ok(None)
    }

    /// `tree` with the files of `replacements` set (`Some`) or removed (`None`); `None` when the
    /// result is empty.
    fn replace_files(
        &self,
        tree: Option<GitOid>,
        replacements: &BTreeMap<String, Option<GitOid>>,
    ) -> Result<Option<GitOid>, EventError> {
        let mut entries: BTreeMap<String, TreeEntry> = match tree {
            Some(tree) => self
                .objects
                .read_tree(&tree)
                .context(|| format!("read tree {tree}"))?
                .into_iter()
                .map(|entry| (entry.name.clone(), entry))
                .collect(),
            None => BTreeMap::new(),
        };
        let mut nested: BTreeMap<String, BTreeMap<String, Option<GitOid>>> = BTreeMap::new();
        for (path, blob) in replacements {
            match path.split_once('/') {
                Some((directory, rest)) => {
                    nested
                        .entry(directory.to_owned())
                        .or_default()
                        .insert(rest.to_owned(), *blob);
                }
                None => match blob {
                    Some(oid) => {
                        let mode = entries
                            .get(path)
                            .map(|entry| entry.mode)
                            .filter(|mode| *mode == EntryMode::ExecutableBlob)
                            .unwrap_or(EntryMode::Blob);
                        entries.insert(
                            path.clone(),
                            TreeEntry {
                                name: path.clone(),
                                mode,
                                oid: *oid,
                            },
                        );
                    }
                    None => {
                        entries.remove(path);
                    }
                },
            }
        }
        for (directory, inner) in nested {
            let current = entries
                .get(&directory)
                .filter(|entry| entry.mode == EntryMode::Tree)
                .map(|entry| entry.oid);
            match self.replace_files(current, &inner)? {
                Some(oid) => {
                    entries.insert(
                        directory.clone(),
                        TreeEntry {
                            name: directory,
                            mode: EntryMode::Tree,
                            oid,
                        },
                    );
                }
                None => {
                    entries.remove(&directory);
                }
            }
        }
        if entries.is_empty() {
            return Ok(None);
        }
        let entries: Vec<TreeEntry> = entries.into_values().collect();
        Ok(Some(
            self.objects
                .write_tree(&entries)
                .context(|| "write a tree".into())?,
        ))
    }
}
