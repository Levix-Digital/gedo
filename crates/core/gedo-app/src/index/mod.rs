//! History indexing: the identities of every Rust entity of every commit, from the Git history
//! alone (FEAT-015, `specs/003-node-identity/research.md` R10 and R11).
//!
//! Commits are indexed parents first. Each commit's changed Rust files are parsed and matched
//! against its first parent's results; at a merge commit they are also matched against the other
//! parents, and two ids found for one entity are reconciled (the lower id is kept). Since a commit's
//! ancestry is fixed by its id, its results are a pure function of the commit, whatever the order in
//! which branches are indexed and on whatever machine.

mod diff;
mod records;
mod walk;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use gedo_ast::{AliasTable, ChangeSet, EntityTokens, NewFile, OldFile, match_change};
use gedo_core::StoreError;
use gedo_core::identity::{AstNodeId, BlobAstMap, IdentityMerged, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::ports::{CommitInfo, ObjectStore, ParseError, Parser, SymbolCache};
use gedo_core::syntax::SyntaxMap;

pub use diff::FileChange;
pub use records::{CHECKPOINT_INTERVAL, MapDigest, NAMESPACE};

use records::{Checkpoint, CommitRecord, MergeList};

/// Commits whose changed files are read and parsed together, in parallel.
const BATCH_COMMITS: usize = 128;

/// Parsed files kept in memory before the parse cache is emptied.
const MAX_CACHED_PARSES: usize = 20_000;

/// Why indexing failed.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// Reading the history or the cache failed.
    #[error("{context}: {source}")]
    Store {
        /// What was being done.
        context: String,
        /// The storage error.
        #[source]
        source: StoreError,
    },

    /// A query named a commit that has not been indexed.
    #[error("commit {commit} is not indexed")]
    NotIndexed {
        /// The commit.
        commit: GitOid,
    },
}

trait Context<T> {
    fn context(self, context: impl FnOnce() -> String) -> Result<T, IndexError>;
}

impl<T> Context<T> for Result<T, StoreError> {
    fn context(self, context: impl FnOnce() -> String) -> Result<T, IndexError> {
        self.map_err(|source| IndexError::Store {
            context: context(),
            source,
        })
    }
}

/// What a call to [`IdentityIndex::index_history`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexReport {
    /// Commits indexed by this call.
    pub commits_indexed: u64,
    /// Files parsed by this call.
    pub files_parsed: u64,
    /// Files the parser refused (too large, too deep, too slow), with the commit and the reason;
    /// they are treated as files of no supported language in that commit.
    pub files_refused: Vec<(GitOid, String, ParseError)>,
}

/// The deltas and identity merges of one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitChanges {
    /// The deltas against the first parent.
    pub deltas: Vec<SemanticDelta>,
    /// The identity merges found at the commit.
    pub merges: Vec<IdentityMerged>,
}

/// The identities of a repository's history, kept in the symbol cache.
pub struct IdentityIndex<'a, O, P, C> {
    objects: &'a O,
    parser: &'a P,
    cache: &'a mut C,
    merges: MergeList,
    aliases: AliasTable,
}

/// Every Rust file of one commit with the digest of its map.
type FileTable = BTreeMap<String, MapDigest>;

type ParseResult = Result<Arc<SyntaxMap>, ParseError>;

impl<'a, O: ObjectStore, P: Parser + Sync, C: SymbolCache> IdentityIndex<'a, O, P, C> {
    /// Opens the index kept in `cache`.
    pub fn open(objects: &'a O, parser: &'a P, cache: &'a mut C) -> Result<Self, IndexError> {
        let merges: MergeList = match cache
            .get(NAMESPACE, records::MERGES_KEY)
            .context(|| "read the merge list".into())?
        {
            Some(bytes) => {
                records::decode(&bytes, "identity merges").context(|| "decode the merge list".into())?
            }
            None => MergeList::default(),
        };
        let aliases = AliasTable::from_merges(&merges.merges);
        Ok(Self {
            objects,
            parser,
            cache,
            merges,
            aliases,
        })
    }

    /// Indexes every commit reachable from `tips` that has no record yet.
    ///
    /// Idempotent and resumable: each commit is written in one cache transaction after its parents,
    /// so an interrupted call is completed by the next one with the same result.
    pub fn index_history(&mut self, tips: &[GitOid]) -> Result<IndexReport, IndexError> {
        let order = walk::commits_to_index(self.objects, tips, |oid| {
            Ok(self.cache.get(NAMESPACE, &records::commit_key(oid))?.is_some())
        })
        .context(|| "walk the history".into())?;
        let mut report = IndexReport::default();
        let mut run = Run::default();
        for batch in order.chunks(BATCH_COMMITS) {
            self.prepare_batch(batch, &mut run, &mut report)?;
            for (oid, commit) in batch {
                self.index_commit(oid, commit, &mut run, &mut report)?;
                report.commits_indexed += 1;
            }
        }
        tracing::info!(
            commits = report.commits_indexed,
            files_parsed = report.files_parsed,
            files_refused = report.files_refused.len(),
            "history indexed"
        );
        Ok(report)
    }

    /// The map of `path` at `commit`, ids resolved through identity merges; `None` when the file
    /// does not exist there or is not of a supported language.
    pub fn map_at(&self, commit: &GitOid, path: &str) -> Result<Option<BlobAstMap>, IndexError> {
        self.record(commit)?
            .ok_or(IndexError::NotIndexed { commit: *commit })?;
        let table = self.table_of(commit)?;
        let Some(digest) = table.get(path) else {
            return Ok(None);
        };
        let mut map = self.load_map(digest)?;
        for node in &mut map.nodes {
            node.id = self.aliases.resolve(node.id);
        }
        Ok(Some(map))
    }

    /// The paths of every indexed Rust file at `commit`, sorted.
    pub fn files_at(&self, commit: &GitOid) -> Result<Vec<String>, IndexError> {
        self.record(commit)?
            .ok_or(IndexError::NotIndexed { commit: *commit })?;
        Ok(self.table_of(commit)?.into_keys().collect())
    }

    /// The deltas and identity merges of `commit`.
    pub fn changes_of(&self, commit: &GitOid) -> Result<CommitChanges, IndexError> {
        let record = self
            .record(commit)?
            .ok_or(IndexError::NotIndexed { commit: *commit })?;
        Ok(CommitChanges {
            deltas: record.deltas,
            merges: record.merges,
        })
    }

    /// Resolves an id through every identity merge known to the index.
    pub fn resolve(&self, id: AstNodeId) -> AstNodeId {
        self.aliases.resolve(id)
    }

    /// Reads and parses, in parallel, the changed files of a batch of commits.
    fn prepare_batch(
        &self,
        batch: &[(GitOid, CommitInfo)],
        run: &mut Run,
        report: &mut IndexReport,
    ) -> Result<(), IndexError> {
        if run.parses.len() > MAX_CACHED_PARSES {
            run.parses.clear();
        }
        let mut wanted: BTreeSet<(String, GitOid)> = BTreeSet::new();
        for (oid, commit) in batch {
            let changes = self.changes(oid, commit)?;
            for change in &changes {
                for blob in [change.old, change.new].into_iter().flatten() {
                    if !run.parses.contains_key(&(change.path.clone(), blob)) {
                        wanted.insert((change.path.clone(), blob));
                    }
                }
            }
            run.diffs.insert(*oid, changes);
        }
        let mut sources = Vec::with_capacity(wanted.len());
        for (path, blob) in wanted {
            let bytes = self
                .objects
                .read_blob(&blob)
                .context(|| format!("read {path} ({blob})"))?;
            sources.push((path, blob, bytes));
        }
        report.files_parsed += sources.len() as u64;
        for (key, result) in parse_all(self.parser, sources) {
            run.parses.insert(key, result);
        }
        Ok(())
    }

    /// The Rust files that differ between `commit` and its first parent.
    fn changes(&self, oid: &GitOid, commit: &CommitInfo) -> Result<Vec<FileChange>, IndexError> {
        let parent_tree = match commit.parents.first() {
            Some(parent) => Some(
                self.objects
                    .read_commit(parent)
                    .context(|| format!("read commit {parent}"))?
                    .tree,
            ),
            None => None,
        };
        let keep = |path: &str| self.parser.language_of(path).is_some();
        diff::diff_trees(self.objects, parent_tree.as_ref(), Some(&commit.tree), &keep)
            .context(|| format!("diff commit {oid} against its first parent"))
    }

    fn index_commit(
        &mut self,
        oid: &GitOid,
        commit: &CommitInfo,
        run: &mut Run,
        report: &mut IndexReport,
    ) -> Result<(), IndexError> {
        let first_parent = commit.parents.first();
        let mut table = match (first_parent, run.current.take()) {
            (None, _) => FileTable::new(),
            (Some(parent), Some((current, table))) if current == *parent => table,
            (Some(parent), _) => self.table_of(parent)?,
        };
        let depth = match first_parent {
            Some(parent) => {
                self.record(parent)?
                    .ok_or(IndexError::NotIndexed { commit: *parent })?
                    .depth
                    + 1
            }
            None => 0,
        };
        let changes = match run.diffs.remove(oid) {
            Some(changes) => changes,
            None => self.changes(oid, commit)?,
        };

        let mut change_set = ChangeSet::default();
        for change in &changes {
            if let Some(digest) = table.get(&change.path) {
                let map = self.load_map(digest)?;
                change_set.old.push(self.old_file(map, run, report)?);
            }
            if let Some(blob) = change.new {
                match self.parse(&change.path, blob, run, report)? {
                    Ok(syntax) => change_set.new.push(NewFile {
                        blob,
                        syntax: (*syntax).clone(),
                    }),
                    Err(error) => {
                        tracing::warn!(commit = %oid, path = %change.path, %error, "file refused by the parser");
                        report.files_refused.push((*oid, change.path.clone(), error));
                    }
                }
            }
        }
        let changed: BTreeSet<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        let mut live = BTreeSet::new();
        for (path, digest) in &table {
            if !changed.contains(path.as_str()) {
                live.extend(self.ids_of(digest, run)?);
            }
        }
        let outcome = match_change(&change_set, &live);
        let mut maps: BTreeMap<String, BlobAstMap> = outcome
            .maps
            .into_iter()
            .map(|map| (map.path.clone(), map))
            .collect();
        let mut deltas = outcome.deltas;
        let mut merges = Vec::new();
        for parent in commit.parents.iter().skip(1) {
            let found = self.reconcile(
                oid,
                commit,
                parent,
                &table,
                &live,
                &mut maps,
                &mut deltas,
                run,
                report,
            )?;
            merges.extend(found);
        }
        merges.sort();

        let mut entries = Vec::new();
        let mut files: BTreeMap<String, Option<MapDigest>> =
            changes.iter().map(|change| (change.path.clone(), None)).collect();
        for map in maps.values() {
            let (digest, bytes) =
                records::encode_map(map).context(|| format!("encode the map of {}", map.path))?;
            run.ids
                .insert(digest, map.nodes.iter().map(|node| node.id).collect());
            files.insert(map.path.clone(), Some(digest));
            entries.push((records::map_key(&digest), bytes));
        }
        for (path, digest) in &files {
            match digest {
                Some(digest) => table.insert(path.clone(), *digest),
                None => table.remove(path),
            };
        }
        let files: Vec<(String, Option<MapDigest>)> = files.into_iter().collect();
        if depth % CHECKPOINT_INTERVAL == 0 {
            let checkpoint = Checkpoint {
                files: table
                    .iter()
                    .map(|(path, digest)| (path.clone(), *digest))
                    .collect(),
            };
            entries.push((
                records::checkpoint_key(oid),
                records::encode(&checkpoint).context(|| "encode a checkpoint".into())?,
            ));
        }
        if !merges.is_empty() {
            self.merges.merges.extend(merges.iter().copied());
            entries.push((
                records::MERGES_KEY.to_vec(),
                records::encode(&self.merges).context(|| "encode the merge list".into())?,
            ));
            merges.iter().for_each(|merge| self.aliases.add(merge));
        }
        let record = CommitRecord {
            parents: commit.parents.clone(),
            depth,
            files,
            deltas,
            merges,
        };
        // The commit record goes last in the same transaction: its presence means "indexed".
        entries.push((
            records::commit_key(oid),
            records::encode(&record).context(|| "encode a commit record".into())?,
        ));
        self.cache
            .put(NAMESPACE, &entries)
            .context(|| format!("write the records of commit {oid}"))?;
        tracing::debug!(commit = %oid, files = record.files.len(), deltas = record.deltas.len(), "commit indexed");
        run.current = Some((*oid, table));
        Ok(())
    }

    /// Matches the merge's files that differ from another parent or from the first parent against
    /// that other parent's results;
    /// an entity that continues an id of that parent other than its own is reconciled, the lower
    /// id kept (research R11). `maps` holds the merge's changed files and receives the files whose
    /// ids changed.
    #[allow(clippy::too_many_arguments)]
    fn reconcile(
        &self,
        oid: &GitOid,
        commit: &CommitInfo,
        parent: &GitOid,
        first_parent_table: &FileTable,
        live: &BTreeSet<AstNodeId>,
        maps: &mut BTreeMap<String, BlobAstMap>,
        deltas: &mut [SemanticDelta],
        run: &mut Run,
        report: &mut IndexReport,
    ) -> Result<Vec<IdentityMerged>, IndexError> {
        let theirs = self.table_of(parent)?;
        let parent_tree = self
            .objects
            .read_commit(parent)
            .context(|| format!("read commit {parent}"))?
            .tree;
        let keep = |path: &str| self.parser.language_of(path).is_some();
        let differences = diff::diff_trees(self.objects, Some(&parent_tree), Some(&commit.tree), &keep)
            .context(|| format!("diff merge {oid} against parent {parent}"))?;
        // The files that differ from this parent, and the files the merge changed against its
        // first parent (which may equal this parent's version).
        let mut candidates: BTreeMap<String, GitOid> = differences
            .into_iter()
            .filter_map(|change| change.new.map(|blob| (change.path, blob)))
            .collect();
        for map in maps.values() {
            candidates.insert(map.path.clone(), map.source_blob);
        }
        let mut ours = Vec::new();
        let mut other = ChangeSet::default();
        for (path, blob) in candidates {
            let our_map = match (maps.get(&path), first_parent_table.get(&path)) {
                (Some(map), _) => map.clone(),
                (None, Some(digest)) => self.load_map(digest)?,
                (None, None) => continue,
            };
            let Ok(syntax) = self.parse(&path, blob, run, report)? else {
                continue;
            };
            if let Some(digest) = theirs.get(&path) {
                let map = self.load_map(digest)?;
                other.old.push(self.old_file(map, run, report)?);
            }
            other.new.push(NewFile {
                blob,
                syntax: (*syntax).clone(),
            });
            ours.push(our_map);
        }
        if other.old.is_empty() {
            return Ok(Vec::new());
        }
        let outcome = match_change(&other, &BTreeSet::new());
        let created: BTreeSet<AstNodeId> = outcome
            .deltas
            .iter()
            .filter_map(|delta| match delta {
                SemanticDelta::Created { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        let present: BTreeSet<AstNodeId> = maps
            .values()
            .chain(&ours)
            .flat_map(|map| map.nodes.iter().map(|node| node.id))
            .chain(live.iter().copied())
            .collect();
        let mut renames: BTreeMap<AstNodeId, AstNodeId> = BTreeMap::new();
        let mut merges = Vec::new();
        // Both lists hold the same files sorted by path, and each pair of maps comes from the
        // same syntax map, so their placed nodes are in the same order.
        for (map, theirs) in ours.iter().zip(&outcome.maps) {
            for (our, their) in map.nodes.iter().zip(&theirs.nodes) {
                let (x, y) = (our.id, their.id);
                if our.is_detached() || x == y || created.contains(&y) || present.contains(&y) {
                    continue;
                }
                let (kept, merged) = if x < y { (x, y) } else { (y, x) };
                merges.push(IdentityMerged {
                    kept,
                    merged,
                    commit: *oid,
                });
                if kept != x {
                    renames.insert(x, kept);
                }
            }
        }
        if !renames.is_empty() {
            for mut map in ours {
                let mut renamed = false;
                for node in &mut map.nodes {
                    if let Some(kept) = renames.get(&node.id) {
                        node.id = *kept;
                        renamed = true;
                    }
                }
                if renamed {
                    maps.insert(map.path.clone(), map);
                }
            }
            for delta in deltas.iter_mut() {
                rename_delta(delta, &renames);
            }
            deltas.sort();
        }
        Ok(merges)
    }

    fn old_file(
        &self,
        map: BlobAstMap,
        run: &mut Run,
        report: &mut IndexReport,
    ) -> Result<OldFile, IndexError> {
        let syntax = self.parse(&map.path, map.source_blob, run, report)?.ok();
        let tokens = map
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                if node.is_detached() {
                    return None;
                }
                syntax
                    .as_ref()
                    .and_then(|syntax| syntax.entities.get(index))
                    .map(EntityTokens::of)
            })
            .collect();
        Ok(OldFile { map, tokens })
    }

    fn parse(
        &self,
        path: &str,
        blob: GitOid,
        run: &mut Run,
        report: &mut IndexReport,
    ) -> Result<ParseResult, IndexError> {
        let key = (path.to_owned(), blob);
        if let Some(result) = run.parses.get(&key) {
            return Ok(result.clone());
        }
        let bytes = self
            .objects
            .read_blob(&blob)
            .context(|| format!("read {path} ({blob})"))?;
        report.files_parsed += 1;
        let result = parse_one(self.parser, path, &bytes);
        run.parses.insert(key, result.clone());
        Ok(result)
    }

    fn ids_of(&self, digest: &MapDigest, run: &mut Run) -> Result<Vec<AstNodeId>, IndexError> {
        if let Some(ids) = run.ids.get(digest) {
            return Ok(ids.clone());
        }
        let ids: Vec<AstNodeId> = self.load_map(digest)?.nodes.iter().map(|node| node.id).collect();
        run.ids.insert(*digest, ids.clone());
        Ok(ids)
    }

    fn record(&self, commit: &GitOid) -> Result<Option<CommitRecord>, IndexError> {
        let bytes = self
            .cache
            .get(NAMESPACE, &records::commit_key(commit))
            .context(|| format!("read the record of commit {commit}"))?;
        bytes
            .map(|bytes| records::decode(&bytes, &format!("identity record of {commit}")))
            .transpose()
            .context(|| format!("decode the record of commit {commit}"))
    }

    fn load_map(&self, digest: &MapDigest) -> Result<BlobAstMap, IndexError> {
        let key = records::map_key(digest);
        let bytes = self
            .cache
            .get(NAMESPACE, &key)
            .context(|| "read a map".into())?
            .ok_or_else(|| IndexError::Store {
                context: "read a map".into(),
                source: StoreError::Corrupted {
                    path: String::from_utf8_lossy(&key).into_owned(),
                    offset: None,
                    reason: "a commit record names a map that is missing".into(),
                },
            })?;
        records::decode(&bytes, "identity map").context(|| "decode a map".into())
    }

    /// Every Rust file of an indexed commit: the nearest checkpoint along first parents, then the
    /// records after it.
    fn table_of(&self, commit: &GitOid) -> Result<FileTable, IndexError> {
        let mut pending = Vec::new();
        let mut current = *commit;
        let mut table = loop {
            let checkpoint = self
                .cache
                .get(NAMESPACE, &records::checkpoint_key(&current))
                .context(|| format!("read the checkpoint of {current}"))?;
            if let Some(bytes) = checkpoint {
                let checkpoint: Checkpoint = records::decode(&bytes, &format!("checkpoint of {current}"))
                    .context(|| "decode a checkpoint".into())?;
                break checkpoint.files.into_iter().collect::<FileTable>();
            }
            let record = self
                .record(&current)?
                .ok_or(IndexError::NotIndexed { commit: current })?;
            let parent = record.parents.first().copied();
            pending.push(record);
            match parent {
                Some(parent) => current = parent,
                None => break FileTable::new(),
            }
        };
        for record in pending.iter().rev() {
            for (path, digest) in &record.files {
                match digest {
                    Some(digest) => table.insert(path.clone(), *digest),
                    None => table.remove(path),
                };
            }
        }
        Ok(table)
    }
}

/// State kept for one call of `index_history`.
#[derive(Default)]
struct Run {
    /// The last indexed commit and its files, reused when the next commit is its child.
    current: Option<(GitOid, FileTable)>,
    diffs: HashMap<GitOid, Vec<FileChange>>,
    parses: HashMap<(String, GitOid), ParseResult>,
    ids: HashMap<MapDigest, Vec<AstNodeId>>,
}

fn rename_delta(delta: &mut SemanticDelta, renames: &BTreeMap<AstNodeId, AstNodeId>) {
    let id = match delta {
        SemanticDelta::Created { id, .. }
        | SemanticDelta::Moved { id, .. }
        | SemanticDelta::Renamed { id, .. }
        | SemanticDelta::SignatureMutated { id, .. }
        | SemanticDelta::BodyModified { id, .. }
        | SemanticDelta::Tombstoned { id, .. } => id,
    };
    if let Some(kept) = renames.get(id) {
        *id = *kept;
    }
}

fn parse_one<P: Parser>(parser: &P, path: &str, bytes: &[u8]) -> ParseResult {
    let parsed = parser.parse(path, bytes)?;
    Ok(Arc::new(gedo_ast::normalize(&parsed, bytes)?))
}

/// Parses files on all cores; the results do not depend on the number of threads.
fn parse_all<P: Parser + Sync>(
    parser: &P,
    sources: Vec<(String, GitOid, Vec<u8>)>,
) -> Vec<((String, GitOid), ParseResult)> {
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(sources.len().max(1));
    let chunk = sources.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = sources
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|(path, blob, bytes)| ((path.clone(), *blob), parse_one(parser, path, bytes)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}
