//! Semantic lanes: the refactor and feature changes of a publish (FEAT-067,
//! `specs/007-semantic-lanes/research.md` R2 to R4 and R7).
//!
//! Each delta between the base and the final version is one part of one entity (ADR-0017). Moves
//! and renames are refactors; creations, removals, signature and body changes are features, except
//! the changes a move or a rename forces on references (follow-ups) and a module created only to
//! receive moved entities. Follow-ups are found by diffing tokens: a renamed name compares equal to
//! its new name, and a hunk made only of paths is a follow-up when it turns the old path of a moved
//! or renamed entity into its new one.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use gedo_core::event::{Event, SemanticOp};
use gedo_core::identity::{AstNodeId, BlobAstMap, SemanticDelta};
use gedo_core::syntax::{ByteRange, EntityKind, ParsedFile, RawEntity, TokenClass};
use similar::{Algorithm, DiffOp, capture_diff_slices};

use crate::tree::{Part, join_qualified, split_qualified};

/// The lane of a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lane {
    /// Keeps behavior: moves, renames and the reference updates they force.
    Refactor,
    /// Everything else.
    Feature,
}

/// One version of one file: its bytes, the parser's view and its identities.
#[derive(Clone, Copy, Debug)]
pub struct FileVersion<'a> {
    /// The file's bytes.
    pub source: &'a [u8],
    /// The parsed file.
    pub parsed: &'a ParsedFile,
    /// The identities of its entities, ids resolved through identity merges.
    pub map: &'a BlobAstMap,
}

/// A file the publish changes, in the base and in the final version (`None`: absent there).
#[derive(Clone, Debug)]
pub struct FileVersions<'a> {
    /// The file's path.
    pub path: String,
    /// The base version.
    pub before: Option<FileVersion<'a>>,
    /// The final version.
    pub after: Option<FileVersion<'a>>,
}

/// The names and paths that moves and renames change, so references to them can follow.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FollowUps {
    names: BTreeMap<String, String>,
    paths: Vec<(Vec<String>, Vec<String>)>,
}

impl FollowUps {
    /// The follow-ups of the `Renamed` and `Moved` deltas of a publish. A name renamed to two
    /// different names follows neither.
    pub fn from_deltas(deltas: &[SemanticDelta]) -> Self {
        let mut names: BTreeMap<String, Option<String>> = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for delta in deltas {
            match delta {
                SemanticDelta::Renamed {
                    from_name,
                    to_name,
                    qualified_path,
                    ..
                } => {
                    names
                        .entry(from_name.clone())
                        .and_modify(|to| {
                            if to.as_deref() != Some(to_name) {
                                *to = None;
                            }
                        })
                        .or_insert_with(|| Some(to_name.clone()));
                    let (container, _) = split_qualified(qualified_path);
                    paths.extend(path_pair(&join_qualified(container, from_name), qualified_path));
                }
                SemanticDelta::Moved { from, to, .. } => {
                    paths.extend(path_pair(&from.qualified_path, &to.qualified_path));
                }
                _ => {}
            }
        }
        FollowUps {
            names: names
                .into_iter()
                .filter_map(|(from, to)| Some((from, to?)))
                .collect(),
            paths: paths.into_iter().collect(),
        }
    }

    /// The new name of `name`, when a rename of the publish changes it.
    pub fn renamed(&self, name: &str) -> Option<&str> {
        self.names.get(name).map(String::as_str)
    }

    /// Whether the path `before` becomes `after` by a move or rename of the publish (research R3):
    /// both have the same prefix, then a suffix of an old path and the matching suffix of its new
    /// path.
    fn is_path_follow_up(&self, before: &[&str], after: &[&str]) -> bool {
        self.paths.iter().any(|(old, new)| {
            (1..=old.len()).any(|kept| {
                let Some(new_kept) = (kept + new.len()).checked_sub(old.len()) else {
                    return false;
                };
                if new_kept == 0 || new_kept > new.len() || kept > before.len() || new_kept > after.len() {
                    return false;
                }
                let (before_prefix, before_suffix) = before.split_at(before.len() - kept);
                let (after_prefix, after_suffix) = after.split_at(after.len() - new_kept);
                before_prefix == after_prefix
                    && before_suffix
                        .iter()
                        .copied()
                        .eq(old[old.len() - kept..].iter().map(String::as_str))
                    && after_suffix
                        .iter()
                        .copied()
                        .eq(new[new.len() - new_kept..].iter().map(String::as_str))
            })
        })
    }
}

/// The segments of a moved or renamed path, without `crate`; none for paths through an
/// implementation block, which no reference spells.
fn path_pair(old: &str, new: &str) -> Option<(Vec<String>, Vec<String>)> {
    let segments = |path: &str| -> Option<Vec<String>> {
        if path.contains('<') {
            return None;
        }
        let path = path.strip_prefix("crate::").unwrap_or(path);
        Some(path.split("::").map(str::to_owned).collect())
    };
    let (old, new) = (segments(old)?, segments(new)?);
    (old != new).then_some((old, new))
}

/// One change of a publish with its lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanedChange {
    /// The delta between the base and the final version.
    pub delta: SemanticDelta,
    /// The part of the entity it changes.
    pub part: Part,
    /// Its lane.
    pub lane: Lane,
    /// Whether a feature change also carries follow-ups, so the refactor commit changes the part
    /// too (a body that calls a renamed function and gains a feature edit).
    pub with_follow_ups: bool,
}

/// The lanes of every change of a publish.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanePlan {
    /// The changes, in the order of the deltas.
    pub changes: Vec<LanedChange>,
    /// The follow-ups of the publish's moves and renames.
    pub follow_ups: FollowUps,
    containers: BTreeSet<AstNodeId>,
}

impl LanePlan {
    /// The lane of a part of an entity, when the publish changes it.
    pub fn lane_of(&self, id: AstNodeId, part: Part) -> Option<Lane> {
        self.changes
            .iter()
            .find(|change| change.delta.id() == id && change.part == part)
            .map(|change| change.lane)
    }

    /// Whether some change is in `lane`.
    pub fn has(&self, lane: Lane) -> bool {
        self.changes.iter().any(|change| change.lane == lane)
    }

    /// The parts the commit of `lane` changes: those in the lane and, for the refactor commit, the
    /// feature changes that also carry follow-ups.
    pub fn parts(&self, lane: Lane) -> BTreeSet<(AstNodeId, Part)> {
        self.changes
            .iter()
            .filter(|change| change.lane == lane || (lane == Lane::Refactor && change.with_follow_ups))
            .map(|change| (change.delta.id(), change.part))
            .collect()
    }

    /// Whether the entity holds other entities, so its body changes with theirs.
    pub fn is_container(&self, id: AstNodeId) -> bool {
        self.containers.contains(&id)
    }

    /// The entities whose place changes in the refactor lane, or that the refactor lane creates:
    /// they are rebuilt as a whole where the final version puts them.
    pub(crate) fn blocks(&self) -> BTreeSet<AstNodeId> {
        self.changes
            .iter()
            .filter(|change| {
                change.lane == Lane::Refactor && matches!(change.part, Part::Place | Part::Presence)
            })
            .map(|change| change.delta.id())
            .collect()
    }
}

/// The part of an entity a delta changes.
pub fn part_of(delta: &SemanticDelta) -> Part {
    match delta {
        SemanticDelta::Created { .. } | SemanticDelta::Tombstoned { .. } => Part::Presence,
        SemanticDelta::Moved { .. } => Part::Place,
        SemanticDelta::Renamed { .. } => Part::Name,
        SemanticDelta::SignatureMutated { .. } => Part::Signature,
        SemanticDelta::BodyModified { .. } => Part::Body,
    }
}

/// Gives each delta of a publish its lane (research R2 to R4). `files` are the files the publish
/// changes; `deltas` its changes between their base and final versions.
pub fn classify(deltas: &[SemanticDelta], files: &[FileVersions<'_>]) -> LanePlan {
    let follow_ups = FollowUps::from_deltas(deltas);
    let entities = Entities::new(files);
    let moved: BTreeSet<AstNodeId> = deltas
        .iter()
        .filter(|delta| matches!(delta, SemanticDelta::Moved { .. }))
        .map(SemanticDelta::id)
        .collect();
    let created: BTreeSet<AstNodeId> = deltas
        .iter()
        .filter(|delta| matches!(delta, SemanticDelta::Created { .. }))
        .map(SemanticDelta::id)
        .collect();

    // Edits of an entity's own tokens, nested entities aside.
    let mut own_edits: BTreeMap<AstNodeId, [OwnEdit; 2]> = BTreeMap::new();
    for delta in deltas {
        if matches!(
            delta,
            SemanticDelta::SignatureMutated { .. } | SemanticDelta::BodyModified { .. }
        ) {
            own_edits
                .entry(delta.id())
                .or_insert_with(|| entities.own_edit_lanes(delta.id(), &follow_ups));
        }
    }

    let mut changes: Vec<LanedChange> = deltas
        .iter()
        .map(|delta| {
            let part = part_of(delta);
            let mut with_follow_ups = false;
            let mut edit = |edit: OwnEdit| {
                with_follow_ups = edit.feature && edit.follow_ups;
                if edit.feature {
                    Lane::Feature
                } else {
                    Lane::Refactor
                }
            };
            let lane = match delta {
                SemanticDelta::Moved { .. } | SemanticDelta::Renamed { .. } => Lane::Refactor,
                SemanticDelta::Tombstoned { .. } => Lane::Feature,
                SemanticDelta::Created { id, kind, at } => {
                    if *kind == EntityKind::Module
                        && entities.module_receives_only_moves(*id, &at.qualified_path, &moved, &created)
                    {
                        Lane::Refactor
                    } else {
                        Lane::Feature
                    }
                }
                SemanticDelta::SignatureMutated { id, .. } => edit(own_edits[id][0]),
                SemanticDelta::BodyModified { id, .. } => edit(own_edits[id][1]),
            };
            LanedChange {
                delta: delta.clone(),
                part,
                lane,
                with_follow_ups,
            }
        })
        .collect();

    // A container's body follows the changes of the entities it holds.
    let containers = entities.containers();
    let feature_inside: BTreeSet<AstNodeId> = changes
        .iter()
        .filter(|change| change.lane == Lane::Feature && change.part != Part::Place)
        .flat_map(|change| entities.ancestors(change.delta.id()))
        .collect();
    for change in &mut changes {
        if change.part == Part::Body
            && containers.contains(&change.delta.id())
            && feature_inside.contains(&change.delta.id())
        {
            change.lane = Lane::Feature;
        }
    }
    LanePlan {
        changes,
        follow_ups,
        containers,
    }
}

/// The lane of an event of the log (research R7); `resolve` maps ids through identity merges.
pub fn event_lane(plan: &LanePlan, event: &Event, resolve: impl Fn(AstNodeId) -> AstNodeId) -> Lane {
    let lane = |id: AstNodeId, part| plan.lane_of(resolve(id), part).unwrap_or(Lane::Feature);
    match &event.operation {
        SemanticOp::Delta(delta) => match delta {
            SemanticDelta::Moved { .. } | SemanticDelta::Renamed { .. } => Lane::Refactor,
            SemanticDelta::Tombstoned { .. } => Lane::Feature,
            other => lane(other.id(), part_of(other)),
        },
        SemanticOp::IdentityMerged { kept, merged } => {
            let ids = [resolve(*kept), resolve(*merged)];
            let lanes: BTreeSet<Lane> = plan
                .changes
                .iter()
                .filter(|change| ids.contains(&change.delta.id()))
                .map(|change| change.lane)
                .collect();
            if lanes.contains(&Lane::Refactor) && !lanes.contains(&Lane::Feature) {
                Lane::Refactor
            } else {
                Lane::Feature
            }
        }
    }
}

/// Where an entity is in one version.
#[derive(Clone, Copy)]
pub(crate) struct Located<'a> {
    pub(crate) version: FileVersion<'a>,
    pub(crate) node: usize,
    pub(crate) raw: Option<&'a RawEntity>,
}

impl Located<'_> {
    pub(crate) fn byte_range(&self) -> Option<ByteRange> {
        self.version.map.nodes[self.node].byte_range
    }
}

/// Every placed entity of the base and final versions of the changed files, by id.
pub(crate) struct Entities<'a> {
    pub(crate) before: BTreeMap<AstNodeId, Located<'a>>,
    pub(crate) after: BTreeMap<AstNodeId, Located<'a>>,
}

impl<'a> Entities<'a> {
    pub(crate) fn new(files: &[FileVersions<'a>]) -> Self {
        let mut before = BTreeMap::new();
        let mut after = BTreeMap::new();
        for file in files {
            for (version, into) in [(file.before, &mut before), (file.after, &mut after)] {
                let Some(version) = version else { continue };
                let raws: BTreeMap<ByteRange, &RawEntity> = version
                    .parsed
                    .entities
                    .iter()
                    .filter(|raw| raw.kind != EntityKind::Import)
                    .map(|raw| (raw.byte_range, raw))
                    .collect();
                for (index, node) in version.map.nodes.iter().enumerate() {
                    let Some(range) = node.byte_range else { continue };
                    into.insert(
                        node.id,
                        Located {
                            version,
                            node: index,
                            raw: raws.get(&range).copied(),
                        },
                    );
                }
            }
        }
        Entities { before, after }
    }

    /// The entities that hold other entities, in either version.
    fn containers(&self) -> BTreeSet<AstNodeId> {
        self.before
            .values()
            .chain(self.after.values())
            .filter_map(|located| {
                let nodes = &located.version.map.nodes;
                let container = nodes[located.node].container?;
                Some(nodes[container as usize].id)
            })
            .collect()
    }

    /// The containers of `id`, in either version.
    fn ancestors(&self, id: AstNodeId) -> Vec<AstNodeId> {
        let mut ancestors = Vec::new();
        for located in [self.before.get(&id), self.after.get(&id)].into_iter().flatten() {
            let nodes = &located.version.map.nodes;
            let mut container = nodes[located.node].container;
            while let Some(index) = container {
                ancestors.push(nodes[index as usize].id);
                container = nodes[index as usize].container;
            }
        }
        ancestors
    }

    /// Whether a created module holds, in the final version, only entities moved into it.
    fn module_receives_only_moves(
        &self,
        id: AstNodeId,
        qualified_path: &str,
        moved: &BTreeSet<AstNodeId>,
        created: &BTreeSet<AstNodeId>,
    ) -> bool {
        let prefix = format!("{qualified_path}::");
        let members: Vec<AstNodeId> = self
            .after
            .values()
            .map(|located| &located.version.map.nodes[located.node])
            .filter(|node| node.id != id && node.qualified_path.starts_with(&prefix))
            .map(|node| node.id)
            .collect();
        !members.is_empty()
            && members
                .iter()
                .all(|member| moved.contains(member) && !created.contains(member))
    }

    /// The edits of an entity's own tokens, for its signature and for its body.
    fn own_edit_lanes(&self, id: AstNodeId, follow_ups: &FollowUps) -> [OwnEdit; 2] {
        let unknown = [OwnEdit::FEATURE; 2];
        let (Some(before), Some(after)) = (self.before.get(&id), self.after.get(&id)) else {
            return unknown;
        };
        let (Some(before_region), Some(after_region)) = (
            Region::of_entity(*before, id, &|_| true),
            Region::of_entity(*after, id, &|_| true),
        ) else {
            return unknown;
        };
        let body_start = |located: &Located<'_>| {
            located
                .raw
                .and_then(|raw| raw.body_start)
                .map_or(usize::MAX, |start| start as usize)
        };
        let after_body = body_start(after);
        let before_body = body_start(before);
        let mut edits = [OwnEdit::default(); 2];
        for hunk in diff_regions(&before_region, &after_region, follow_ups) {
            let part = match (hunk.after.first(), hunk.before.first()) {
                (Some(unit), _) => usize::from(after_region.units[*unit].first_token >= after_body),
                (None, Some(unit)) => usize::from(before_region.units[*unit].first_token >= before_body),
                (None, None) => continue,
            };
            if hunk.equal {
                // Renamed references compare equal to their new names.
                for (old, new) in hunk.before.iter().zip(&hunk.after) {
                    if before_region.units[*old].key != after_region.units[*new].key {
                        let part = usize::from(after_region.units[*new].first_token >= after_body);
                        edits[part].follow_ups = true;
                    }
                }
            } else if hunk.follow_up {
                edits[part].follow_ups = true;
            } else {
                edits[part].feature = true;
            }
        }
        edits
    }
}

/// What the edits of one part of an entity's own tokens hold.
#[derive(Clone, Copy, Debug, Default)]
struct OwnEdit {
    feature: bool,
    follow_ups: bool,
}

impl OwnEdit {
    const FEATURE: OwnEdit = OwnEdit {
        feature: true,
        follow_ups: false,
    };
}

/// A unit of a region: one token with the text before it, or a whole nested entity.
#[derive(Clone, Debug)]
pub(crate) struct Unit {
    /// The diff key.
    pub(crate) key: String,
    /// Where its text starts, the text before the token included.
    pub(crate) lead: usize,
    /// Where its own text starts.
    pub(crate) start: usize,
    /// Where it ends.
    pub(crate) end: usize,
    /// Its first token's index in the file.
    pub(crate) first_token: usize,
    /// The token's class; `None` for a block.
    pub(crate) class: Option<TokenClass>,
    /// The nested entity, for a block.
    pub(crate) block: Option<AstNodeId>,
}

/// A span of one version of a file cut into units.
pub(crate) struct Region<'a> {
    pub(crate) source: &'a [u8],
    pub(crate) units: Vec<Unit>,
    pub(crate) end: usize,
}

impl<'a> Region<'a> {
    /// The whole file.
    pub(crate) fn of_file(version: FileVersion<'a>, is_block: &dyn Fn(AstNodeId) -> bool) -> Self {
        Self::cut(version, 0..version.source.len(), None, is_block)
    }

    /// One entity, itself not a block.
    pub(crate) fn of_entity(
        located: Located<'a>,
        id: AstNodeId,
        is_block: &dyn Fn(AstNodeId) -> bool,
    ) -> Option<Self> {
        let range = located.byte_range()?.as_usize();
        Some(Self::cut(located.version, range, Some(id), is_block))
    }

    fn cut(
        version: FileVersion<'a>,
        span: Range<usize>,
        owner: Option<AstNodeId>,
        is_block: &dyn Fn(AstNodeId) -> bool,
    ) -> Self {
        // The outermost block entities inside the span, by start.
        let mut blocks: Vec<(Range<usize>, AstNodeId)> = version
            .map
            .nodes
            .iter()
            .filter(|node| Some(node.id) != owner && is_block(node.id))
            .filter_map(|node| Some((node.byte_range?.as_usize(), node.id)))
            .filter(|(range, _)| span.start <= range.start && range.end <= span.end && *range != span)
            .collect();
        blocks.sort_by_key(|(range, _)| (range.start, std::cmp::Reverse(range.end)));
        let mut outer: Vec<(Range<usize>, AstNodeId)> = Vec::new();
        for block in blocks {
            if outer.last().is_none_or(|(last, _)| block.0.start >= last.end) {
                outer.push(block);
            }
        }

        let mut units = Vec::new();
        let mut lead = span.start;
        let mut blocks = outer.into_iter().peekable();
        let tokens = &version.parsed.tokens;
        let mut index = tokens.partition_point(|token| (token.byte_range.start as usize) < span.start);
        while index < tokens.len() {
            let token = tokens[index];
            let range = token.byte_range.as_usize();
            if range.start >= span.end {
                break;
            }
            if let Some((block, id)) = blocks.next_if(|(block, _)| block.start <= range.start) {
                units.push(Unit {
                    key: format!("\u{0}{id}"),
                    lead,
                    start: block.start,
                    end: block.end,
                    first_token: index,
                    class: None,
                    block: Some(id),
                });
                lead = block.end;
                while index < tokens.len() && (tokens[index].byte_range.start as usize) < block.end {
                    index += 1;
                }
                continue;
            }
            units.push(Unit {
                key: String::from_utf8_lossy(&version.source[range.clone()]).into_owned(),
                lead,
                start: range.start,
                end: range.end,
                first_token: index,
                class: Some(token.class),
                block: None,
            });
            lead = range.end;
            index += 1;
        }
        Region {
            source: version.source,
            units,
            end: span.end,
        }
    }

    /// The text of a unit, with what comes before it.
    pub(crate) fn text(&self, unit: usize) -> &'a [u8] {
        let unit = &self.units[unit];
        &self.source[unit.lead..unit.end]
    }

    /// The text before a block.
    pub(crate) fn lead_text(&self, unit: usize) -> &'a [u8] {
        let unit = &self.units[unit];
        &self.source[unit.lead..unit.start]
    }

    /// The text after the last unit.
    pub(crate) fn tail(&self) -> &'a [u8] {
        let start = self
            .units
            .last()
            .map_or(self.end.min(self.source.len()), |unit| unit.end);
        &self.source[start..self.end]
    }
}

/// A step of the diff of two regions: units that are equal, or a hunk that differs.
#[derive(Clone, Debug)]
pub(crate) struct Hunk {
    /// Units of the base region.
    pub(crate) before: Vec<usize>,
    /// Units of the final region.
    pub(crate) after: Vec<usize>,
    /// Equal units (`before[i]` equals `after[i]`).
    pub(crate) equal: bool,
    /// For a hunk that differs: whether it only follows a move or rename.
    pub(crate) follow_up: bool,
}

/// The diff of two regions in steps (research R3): tokens compare equal when their texts are equal
/// or the base token is renamed to the final one, blocks when they are the same entity.
pub(crate) fn diff_regions(before: &Region<'_>, after: &Region<'_>, follow_ups: &FollowUps) -> Vec<Hunk> {
    let before_keys: Vec<&str> = before
        .units
        .iter()
        .map(|unit| match unit.class {
            Some(TokenClass::Identifier) => follow_ups.renamed(&unit.key).unwrap_or(&unit.key),
            _ => unit.key.as_str(),
        })
        .collect();
    let after_keys: Vec<&str> = after.units.iter().map(|unit| unit.key.as_str()).collect();
    let mut steps: Vec<Hunk> = Vec::new();
    for op in capture_diff_slices(Algorithm::Myers, &before_keys, &after_keys) {
        let (old, new) = (op.old_range(), op.new_range());
        if let DiffOp::Equal { .. } = op {
            steps.push(Hunk {
                before: old.collect(),
                after: new.collect(),
                equal: true,
                follow_up: false,
            });
            continue;
        }
        match steps.last_mut() {
            Some(last) if !last.equal => {
                last.before.extend(old);
                last.after.extend(new);
            }
            _ => steps.push(Hunk {
                before: old.collect(),
                after: new.collect(),
                equal: false,
                follow_up: false,
            }),
        }
    }
    for at in 0..steps.len() {
        if !steps[at].equal {
            let follow_up = is_follow_up_hunk(&steps, at, before, after, follow_ups);
            steps[at].follow_up = follow_up;
        }
    }
    steps
}

fn is_path_unit(unit: &Unit) -> bool {
    match unit.class {
        Some(TokenClass::Identifier) => true,
        Some(TokenClass::Keyword) => matches!(unit.key.as_str(), "crate" | "self" | "super" | "Self"),
        Some(TokenClass::Punctuation) => unit.key == "::",
        _ => false,
    }
}

/// Whether the hunk at `at` turns the old path of a moved or renamed entity into its new path,
/// the equal path tokens around it included.
fn is_follow_up_hunk(
    steps: &[Hunk],
    at: usize,
    before: &Region<'_>,
    after: &Region<'_>,
    follow_ups: &FollowUps,
) -> bool {
    let hunk = &steps[at];
    let all_paths =
        |region: &Region<'_>, units: &[usize]| units.iter().all(|&unit| is_path_unit(&region.units[unit]));
    if !all_paths(before, &hunk.before) || !all_paths(after, &hunk.after) {
        return false;
    }
    // The equal path tokens right before and after the hunk.
    let mut leading = Vec::new();
    if let Some(previous) = at
        .checked_sub(1)
        .map(|index| &steps[index])
        .filter(|step| step.equal)
    {
        for &unit in previous.before.iter().rev() {
            if !is_path_unit(&before.units[unit]) {
                break;
            }
            leading.push(unit);
        }
    }
    leading.reverse();
    let leading_after: Vec<usize> = steps
        .get(at.wrapping_sub(1))
        .filter(|step| step.equal)
        .map(|step| step.after[step.after.len() - leading.len()..].to_vec())
        .unwrap_or_default();
    let mut trailing = 0;
    if let Some(next) = steps.get(at + 1).filter(|step| step.equal) {
        trailing = next
            .before
            .iter()
            .take_while(|&&unit| is_path_unit(&before.units[unit]))
            .count();
    }
    let trailing_before: Vec<usize> = steps
        .get(at + 1)
        .map(|step| step.before[..trailing].to_vec())
        .unwrap_or_default();
    let trailing_after: Vec<usize> = steps
        .get(at + 1)
        .map(|step| step.after[..trailing].to_vec())
        .unwrap_or_default();
    let segments = |region: &'_ Region<'_>, units: Vec<usize>| -> Vec<String> {
        units
            .into_iter()
            .map(|unit| &region.units[unit].key)
            .filter(|key| key.as_str() != "::" && key.as_str() != "crate")
            .cloned()
            .collect()
    };
    let before_path = segments(
        before,
        leading
            .iter()
            .chain(&hunk.before)
            .chain(&trailing_before)
            .copied()
            .collect(),
    );
    let after_path = segments(
        after,
        leading_after
            .iter()
            .chain(&hunk.after)
            .chain(&trailing_after)
            .copied()
            .collect(),
    );
    // Segments after the hunk may name a member of the moved entity: compare without them too.
    let trailing_segments = segments(before, trailing_before.clone()).len();
    let before_refs: Vec<&str> = before_path.iter().map(String::as_str).collect();
    let after_refs: Vec<&str> = after_path.iter().map(String::as_str).collect();
    (0..=trailing_segments.min(before_refs.len()).min(after_refs.len())).any(|drop| {
        follow_ups.is_path_follow_up(
            &before_refs[..before_refs.len() - drop],
            &after_refs[..after_refs.len() - drop],
        )
    })
}

#[cfg(test)]
mod tests {
    use gedo_core::identity::Location;

    use super::*;

    fn at(path: &str, qualified_path: &str) -> Location {
        Location {
            unit: "src".into(),
            path: path.into(),
            qualified_path: qualified_path.into(),
        }
    }

    fn follow_ups() -> FollowUps {
        FollowUps::from_deltas(&[
            SemanticDelta::Renamed {
                id: AstNodeId([1; 16]),
                from_name: "sum".into(),
                to_name: "total".into(),
                qualified_path: "crate::Invoice::total".into(),
            },
            SemanticDelta::Moved {
                id: AstNodeId([2; 16]),
                from: at("src/billing.rs", "crate::billing::tax"),
                to: at("src/billing/tax.rs", "crate::billing::tax::tax"),
            },
        ])
    }

    #[test]
    fn renamed_names_follow_and_ambiguous_ones_do_not() {
        assert_eq!(follow_ups().renamed("sum"), Some("total"));
        let ambiguous = FollowUps::from_deltas(&[
            SemanticDelta::Renamed {
                id: AstNodeId([1; 16]),
                from_name: "new".into(),
                to_name: "create".into(),
                qualified_path: "crate::A::create".into(),
            },
            SemanticDelta::Renamed {
                id: AstNodeId([2; 16]),
                from_name: "new".into(),
                to_name: "build".into(),
                qualified_path: "crate::B::build".into(),
            },
        ]);
        assert_eq!(ambiguous.renamed("new"), None);
    }

    #[test]
    fn paths_follow_a_move_with_any_prefix_and_suffix_length() {
        let follow_ups = follow_ups();
        let path = |text: &'static str| -> Vec<&'static str> { text.split("::").collect() };
        assert!(follow_ups.is_path_follow_up(&path("billing::tax"), &path("billing::tax::tax")));
        assert!(
            follow_ups.is_path_follow_up(&path("super::billing::tax"), &path("super::billing::tax::tax"))
        );
        assert!(follow_ups.is_path_follow_up(&path("Invoice::sum"), &path("Invoice::total")));
        assert!(!follow_ups.is_path_follow_up(&path("billing::fee"), &path("billing::tax::fee")));
        assert!(!follow_ups.is_path_follow_up(&path("other::tax"), &path("billing::tax::tax")));
    }
}
