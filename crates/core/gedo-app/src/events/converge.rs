//! Convergence of commits (FEAT-066, `specs/006-tree-crdt/research.md` R9): the work of several
//! branches since a common base, folded and converged by the tree CRDT of `gedo_crdt::tree`.
//!
//! The base state comes from the identity results of the base commit, for the files the sides
//! changed (the rest of the base is read only when an event names an entity outside them). The
//! events of a side are those of its commits after the base, through `events_of_commit`, so a
//! commit published with Gedo gives its live events with their authors and labels, followed by the
//! live events the caller adds on top (the working tree, a stream). Nothing is written.

use std::collections::{BTreeSet, BinaryHeap, HashMap};

use gedo_core::event::Event;
use gedo_core::ids::GitOid;
use gedo_core::ports::{ObjectStore, Parser, RefStore, SymbolCache, WriteAheadLog};
use gedo_crdt::{Convergence, SideError, TreeError, TreeState, converge_with};

use super::{Context, EventError, EventLog};
use crate::index::IdentityIndex;

/// One side of a convergence: a tip commit and live events on top of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    /// The side's last commit.
    pub tip: GitOid,
    /// Events not published yet, recorded on top of `tip`; may be empty.
    pub live: Vec<Event>,
}

impl Side {
    /// A side made of the commits up to `tip` only.
    pub fn commit(tip: GitOid) -> Self {
        Side {
            tip,
            live: Vec::new(),
        }
    }
}

/// What a convergence found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConvergeReport {
    /// The converged state, the identity links and the conflicts.
    pub convergence: Convergence,
    /// For each side, its commits after the base, in generation order.
    pub commits: Vec<Vec<GitOid>>,
    /// For each side, the events folded (commits and live events).
    pub events: Vec<usize>,
}

/// Which walks reached a commit.
const FROM_TIP: u8 = 1;
const FROM_BASE: u8 = 2;

impl<O: ObjectStore, R: RefStore, W: WriteAheadLog> EventLog<'_, O, R, W> {
    /// Converges the sides over the common `base` (FR-012). The base and every commit of every side
    /// must be indexed in `identities`.
    pub fn converge<P, C>(
        &mut self,
        base: &GitOid,
        sides: &[Side],
        identities: &IdentityIndex<'_, O, P, C>,
    ) -> Result<ConvergeReport, EventError>
    where
        P: Parser + Sync,
        C: SymbolCache,
    {
        let mut changed: BTreeSet<String> = BTreeSet::new();
        let mut commits = Vec::with_capacity(sides.len());
        let mut events = Vec::with_capacity(sides.len());
        for side in sides {
            let after = self.commits_after(base, &side.tip)?;
            let mut side_events = Vec::new();
            for commit in &after {
                changed.extend(identities.changed_files(commit)?);
                side_events.extend(
                    self.events_of_commit(commit, identities)?
                        .events()
                        .into_iter()
                        .cloned(),
                );
            }
            side_events.extend(side.live.iter().cloned());
            tracing::debug!(%base, tip = %side.tip, commits = after.len(), "side read");
            commits.push(after);
            events.push(side_events);
        }

        let changed: Vec<String> = changed.into_iter().collect();
        let resolve = |id| identities.resolve(id);
        let convergence =
            match converge_with(&self.base_state(base, &changed, identities)?, &events, resolve) {
                Err(SideError {
                    error: TreeError::UnknownEntity { .. },
                    ..
                }) => {
                    // An event names an entity of a file no side commit changed (live events): read the
                    // whole base.
                    let everything = identities.files_at(base)?;
                    converge_with(&self.base_state(base, &everything, identities)?, &events, resolve)
                }
                other => other,
            }
            .map_err(|error| EventError::Tree {
                side: error.side,
                source: error.error,
            })?;
        tracing::info!(
            %base,
            sides = sides.len(),
            events = events.iter().map(Vec::len).sum::<usize>(),
            conflicts = convergence.conflicts.len(),
            "sides converged"
        );
        Ok(ConvergeReport {
            convergence,
            commits,
            events: events.iter().map(Vec::len).collect(),
        })
    }

    /// The tree state of the files `paths` at `base`; a path that does not exist there is skipped.
    fn base_state<P, C>(
        &self,
        base: &GitOid,
        paths: &[String],
        identities: &IdentityIndex<'_, O, P, C>,
    ) -> Result<TreeState, EventError>
    where
        P: Parser + Sync,
        C: SymbolCache,
    {
        identities.files_at(base)?;
        let mut maps = Vec::with_capacity(paths.len());
        for path in paths {
            if let Some(map) = identities.map_at(base, path)? {
                maps.push(map);
            }
        }
        Ok(TreeState::from_maps(&maps))
    }

    /// The commits reachable from `tip` and not from `base`, in generation order then by id. Commits
    /// are visited from the highest generation down, so a commit's flags are final when it is
    /// visited, and the walk stops once every commit left is reachable from the base.
    fn commits_after(&mut self, base: &GitOid, tip: &GitOid) -> Result<Vec<GitOid>, EventError> {
        let mut flags: HashMap<GitOid, u8> = HashMap::new();
        let mut queue: BinaryHeap<(u64, GitOid)> = BinaryHeap::new();
        let mut tip_only = 0usize;
        for (commit, flag) in [(*tip, FROM_TIP), (*base, FROM_BASE)] {
            let entry = flags.entry(commit).or_insert(0);
            if *entry == 0 {
                let generation = self.generation(&commit)?;
                queue.push((generation, commit));
            }
            *entry |= flag;
        }
        if flags.get(tip) == Some(&FROM_TIP) {
            tip_only += 1;
        }
        let mut found = Vec::new();
        while tip_only > 0 {
            let Some((generation, commit)) = queue.pop() else {
                break;
            };
            let flag = flags.get(&commit).copied().unwrap_or(0);
            if flag == FROM_TIP {
                tip_only -= 1;
                found.push((generation, commit));
            }
            let parents = self
                .objects
                .read_commit(&commit)
                .context(|| format!("read commit {commit}"))?
                .parents;
            for parent in parents {
                let before = flags.get(&parent).copied();
                let after = before.unwrap_or(0) | flag;
                if before == Some(after) {
                    continue;
                }
                match before {
                    None => {
                        let generation = self.generation(&parent)?;
                        queue.push((generation, parent));
                        if after == FROM_TIP {
                            tip_only += 1;
                        }
                    }
                    Some(FROM_TIP) => tip_only -= 1,
                    Some(_) => {}
                }
                flags.insert(parent, after);
            }
        }
        found.sort();
        Ok(found.into_iter().map(|(_, commit)| commit).collect())
    }
}
