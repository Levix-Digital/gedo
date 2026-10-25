//! Three-tier matching of the entities of a change set (research R3, R5, R6, R8).
//!
//! The old side holds the anchored entities of the previous versions of the changed files; the new
//! side holds the syntax maps of their new versions. Every new entity either continues exactly one
//! old entity, keeping its id, or gets a new derived id; every old entity is continued, carried
//! (its file has a syntax error) or tombstoned. Every collection is iterated in a fixed order and
//! scores are integers, so the outcome is identical on every machine.

use std::collections::{BTreeMap, BTreeSet};

use gedo_core::identity::{AnchoredNode, AstNodeId, BlobAstMap, Location, MatchFormat, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::syntax::{CodeEntity, EntityKind, NormalizedToken, StructuralHash, SyntaxMap};

use crate::delta;
use crate::identity::{IdentitySeed, derive_id};

/// The matching rules this build applies.
pub const MATCH_FORMAT: MatchFormat = MatchFormat::V1;

/// The score, in thousandths, at or above which tier 2c keeps an identity.
pub const SIMILARITY_THRESHOLD_THOUSANDTHS: u32 = 800;

/// Weights of the score terms, in thousandths; they sum to 1,000.
pub const SIGNATURE_WEIGHT: u32 = 450;
/// See [`SIGNATURE_WEIGHT`].
pub const BODY_WEIGHT: u32 = 400;
/// See [`SIGNATURE_WEIGHT`].
pub const SCOPE_WEIGHT: u32 = 150;

/// Above this many candidate pairs of one kind, tier 2c only scores pairs that share a name, a
/// body hash or a container.
pub const MAX_SCORED_PAIRS: usize = 50_000;

const FULL: u32 = 1_000;

/// The significant tokens of an entity, nested entities excluded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntityTokens {
    /// The tokens before the body.
    pub signature: Vec<NormalizedToken>,
    /// The tokens of the body.
    pub body: Vec<NormalizedToken>,
}

impl EntityTokens {
    /// The tokens of a code entity.
    pub fn of(entity: &CodeEntity) -> Self {
        Self {
            signature: entity.signature_tokens.clone(),
            body: entity.body_tokens.clone(),
        }
    }
}

/// The previous version of one file of a change set.
#[derive(Clone, Debug)]
pub struct OldFile {
    /// Its anchored entities.
    pub map: BlobAstMap,
    /// The tokens of each node of `map`, in the same order; `None` when unknown (a detached node).
    pub tokens: Vec<Option<EntityTokens>>,
}

/// The new version of one file of a change set.
#[derive(Clone, Debug)]
pub struct NewFile {
    /// The blob it was read from.
    pub blob: GitOid,
    /// Its entities.
    pub syntax: SyntaxMap,
}

/// The files that changed between two versions of the code: a file only on the old side was
/// deleted, a file only on the new side was added.
#[derive(Clone, Debug, Default)]
pub struct ChangeSet {
    /// Previous versions.
    pub old: Vec<OldFile>,
    /// New versions.
    pub new: Vec<NewFile>,
}

/// What matching found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchOutcome {
    /// One map per new file, sorted by path.
    pub maps: Vec<BlobAstMap>,
    /// The deltas, ordered by variant then id.
    pub deltas: Vec<SemanticDelta>,
}

/// Matches the old and new entities of a change set.
///
/// `live_ids` are the ids of the entities of the same version of the code outside the change set
/// (its unchanged files): a derived id never equals one of them.
pub fn match_change(change: &ChangeSet, live_ids: &BTreeSet<AstNodeId>) -> MatchOutcome {
    let mut olds = old_entities(change);
    let mut news = new_entities(change);
    olds.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
    news.sort_by(|left, right| left.order_key().cmp(&right.order_key()));

    let mut matcher = Matcher::new(&olds, &news);
    matcher.exact();
    matcher.same_place();
    matcher.same_content();
    matcher.similar();
    matcher.by_children();

    let mut taken: BTreeSet<AstNodeId> = live_ids.clone();
    taken.extend(matcher.new_to_old.iter().flatten().map(|&old| olds[old].node.id));
    let mut ids = Vec::with_capacity(news.len());
    for (index, new) in news.iter().enumerate() {
        let id = match matcher.new_to_old[index] {
            Some(old) => olds[old].node.id,
            None => {
                let id = derive_id(&new.seed(), &taken);
                taken.insert(id);
                id
            }
        };
        ids.push(id);
    }
    let mut deltas = Vec::new();
    for (index, new) in news.iter().enumerate() {
        match matcher.new_to_old[index] {
            Some(old) => {
                let new_container = matcher.news[index].container.map(|container| ids[container]);
                deltas.extend(delta::continued(&olds[old], new, new_container));
            }
            None => deltas.push(SemanticDelta::Created {
                id: ids[index],
                kind: new.entity.kind,
                at: new.location(),
            }),
        }
    }

    let damaged_paths: BTreeSet<&str> = change
        .new
        .iter()
        .filter(|file| file.syntax.has_error)
        .map(|file| file.syntax.path.as_str())
        .collect();
    let mut carried: BTreeMap<&str, Vec<&AnchoredNode>> = BTreeMap::new();
    for (index, old) in olds.iter().enumerate() {
        if matcher.old_used[index] {
            continue;
        }
        if damaged_paths.contains(old.path) {
            carried.entry(old.path).or_default().push(old.node);
        } else {
            deltas.push(SemanticDelta::Tombstoned {
                id: old.node.id,
                kind: old.node.kind,
                at: old.location(),
            });
        }
    }

    deltas.sort();
    MatchOutcome {
        maps: build_maps(change, &news, &ids, &carried),
        deltas,
    }
}

/// An entity of the old side.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OldEntity<'a> {
    pub(crate) unit: &'a str,
    pub(crate) path: &'a str,
    pub(crate) node: &'a AnchoredNode,
    pub(crate) tokens: Option<&'a EntityTokens>,
    /// The id of the enclosing node, if any.
    pub(crate) container_id: Option<AstNodeId>,
}

impl OldEntity<'_> {
    fn order_key(&self) -> (&str, &str, EntityKind, u32, &str, AstNodeId) {
        (
            self.unit,
            &self.node.qualified_path,
            self.node.kind,
            self.node.occurrence,
            self.path,
            self.node.id,
        )
    }

    pub(crate) fn location(&self) -> Location {
        Location {
            unit: self.unit.to_owned(),
            path: self.path.to_owned(),
            qualified_path: self.node.qualified_path.clone(),
        }
    }
}

/// An entity of the new side.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NewEntity<'a> {
    pub(crate) unit: &'a str,
    pub(crate) path: &'a str,
    pub(crate) file: usize,
    pub(crate) index: usize,
    pub(crate) entity: &'a CodeEntity,
    /// Position of the enclosing entity in the sorted new list, if any.
    container: Option<usize>,
}

impl NewEntity<'_> {
    fn order_key(&self) -> (&str, &str, EntityKind, u32, &str, usize) {
        (
            self.unit,
            &self.entity.qualified_path,
            self.entity.kind,
            self.entity.occurrence,
            self.path,
            self.index,
        )
    }

    fn seed(&self) -> IdentitySeed<'_> {
        IdentitySeed {
            kind: self.entity.kind,
            unit: self.unit,
            qualified_path: &self.entity.qualified_path,
            occurrence: self.entity.occurrence,
            structural_hash: self.entity.structural_hash,
        }
    }

    pub(crate) fn location(&self) -> Location {
        Location {
            unit: self.unit.to_owned(),
            path: self.path.to_owned(),
            qualified_path: self.entity.qualified_path.clone(),
        }
    }
}

fn old_entities(change: &ChangeSet) -> Vec<OldEntity<'_>> {
    let mut entities = Vec::new();
    for file in &change.old {
        for (index, node) in file.map.nodes.iter().enumerate() {
            entities.push(OldEntity {
                unit: &file.map.unit,
                path: &file.map.path,
                node,
                tokens: file.tokens.get(index).and_then(Option::as_ref),
                container_id: node
                    .container
                    .and_then(|container| file.map.nodes.get(container as usize))
                    .map(|container| container.id),
            });
        }
    }
    entities
}

fn new_entities(change: &ChangeSet) -> Vec<NewEntity<'_>> {
    let mut entities = Vec::new();
    for (file_index, file) in change.new.iter().enumerate() {
        for (index, entity) in file.syntax.entities.iter().enumerate() {
            entities.push(NewEntity {
                unit: &file.syntax.unit,
                path: &file.syntax.path,
                file: file_index,
                index,
                entity,
                container: None,
            });
        }
    }
    // `container` is resolved to positions in the sorted list by `Matcher::new`.
    entities
}

struct Matcher<'s, 'a> {
    olds: &'s [OldEntity<'a>],
    news: Vec<NewEntity<'a>>,
    new_to_old: Vec<Option<usize>>,
    old_used: Vec<bool>,
}

impl<'s, 'a> Matcher<'s, 'a> {
    fn new(olds: &'s [OldEntity<'a>], news: &[NewEntity<'a>]) -> Self {
        let mut news = news.to_vec();
        let position: BTreeMap<(usize, usize), usize> = news
            .iter()
            .enumerate()
            .map(|(position, new)| ((new.file, new.index), position))
            .collect();
        for new in &mut news {
            new.container = new
                .entity
                .container
                .and_then(|container| position.get(&(new.file, container as usize)).copied());
        }
        Self {
            olds,
            new_to_old: vec![None; news.len()],
            old_used: vec![false; olds.len()],
            news,
        }
    }

    fn assign(&mut self, new: usize, old: usize) {
        self.new_to_old[new] = Some(old);
        self.old_used[old] = true;
    }

    fn unmatched_news(&self) -> Vec<usize> {
        (0..self.news.len())
            .filter(|&new| self.new_to_old[new].is_none())
            .collect()
    }

    /// Tier 1: same unit, kind, qualified path, occurrence and structural hash.
    fn exact(&mut self) {
        let mut by_key: BTreeMap<(&str, &str, EntityKind, u32, StructuralHash), usize> = BTreeMap::new();
        for (index, old) in self.olds.iter().enumerate() {
            by_key
                .entry((
                    old.unit,
                    &old.node.qualified_path,
                    old.node.kind,
                    old.node.occurrence,
                    old.node.structural_hash,
                ))
                .or_insert(index);
        }
        for new in self.unmatched_news() {
            let entity = self.news[new];
            let key = (
                entity.unit,
                entity.entity.qualified_path.as_str(),
                entity.entity.kind,
                entity.entity.occurrence,
                entity.entity.structural_hash,
            );
            if let Some(&old) = by_key.get(&key)
                && !self.old_used[old]
            {
                self.assign(new, old);
            }
        }
    }

    /// Tier 2a: same unit, kind, qualified path and occurrence; the content changed.
    fn same_place(&mut self) {
        let mut by_key: BTreeMap<(&str, &str, EntityKind, u32), Vec<usize>> = BTreeMap::new();
        for (index, old) in self.olds.iter().enumerate() {
            if !self.old_used[index] {
                by_key
                    .entry((
                        old.unit,
                        &old.node.qualified_path,
                        old.node.kind,
                        old.node.occurrence,
                    ))
                    .or_default()
                    .push(index);
            }
        }
        for new in self.unmatched_news() {
            let entity = self.news[new];
            let key = (
                entity.unit,
                entity.entity.qualified_path.as_str(),
                entity.entity.kind,
                entity.entity.occurrence,
            );
            let candidate = by_key
                .get(&key)
                .and_then(|olds| olds.iter().copied().find(|&old| !self.old_used[old]));
            if let Some(old) = candidate {
                self.assign(new, old);
            }
        }
    }

    /// Tier 2b: the same content elsewhere, when the content is unique on both sides.
    fn same_content(&mut self) {
        let mut olds: BTreeMap<(EntityKind, StructuralHash), Vec<usize>> = BTreeMap::new();
        for (index, old) in self.olds.iter().enumerate() {
            if !self.old_used[index] {
                olds.entry((old.node.kind, old.node.structural_hash))
                    .or_default()
                    .push(index);
            }
        }
        let mut news: BTreeMap<(EntityKind, StructuralHash), Vec<usize>> = BTreeMap::new();
        for new in self.unmatched_news() {
            let entity = self.news[new].entity;
            news.entry((entity.kind, entity.structural_hash))
                .or_default()
                .push(new);
        }
        for (key, new_group) in news {
            if let ([new], Some([old])) = (new_group.as_slice(), olds.get(&key).map(Vec::as_slice)) {
                self.assign(*new, *old);
            }
        }
    }

    /// Tier 2c: scored similarity among the remaining entities of the same kind.
    fn similar(&mut self) {
        let mut olds_by_kind: BTreeMap<EntityKind, Vec<usize>> = BTreeMap::new();
        for (index, old) in self.olds.iter().enumerate() {
            if !self.old_used[index] {
                olds_by_kind.entry(old.node.kind).or_default().push(index);
            }
        }
        let mut news_by_kind: BTreeMap<EntityKind, Vec<usize>> = BTreeMap::new();
        for new in self.unmatched_news() {
            news_by_kind
                .entry(self.news[new].entity.kind)
                .or_default()
                .push(new);
        }
        let mut pairs = Vec::new();
        for (kind, news) in &news_by_kind {
            let Some(olds) = olds_by_kind.get(kind) else {
                continue;
            };
            let restricted = olds.len().saturating_mul(news.len()) > MAX_SCORED_PAIRS;
            for &new in news {
                for &old in olds {
                    if restricted && !self.related(old, new) {
                        continue;
                    }
                    let score = self.score(old, new);
                    if score >= SIMILARITY_THRESHOLD_THOUSANDTHS {
                        pairs.push((score, old, new));
                    }
                }
            }
        }
        // Highest score first; ties by old id, then by the new entity's position (its sort key).
        pairs.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| self.olds[left.1].node.id.cmp(&self.olds[right.1].node.id))
                .then_with(|| left.2.cmp(&right.2))
        });
        for (_, old, new) in pairs {
            if !self.old_used[old] && self.new_to_old[new].is_none() {
                self.assign(new, old);
            }
        }
    }

    /// Tier 2d: an unmatched container (an `impl` whose type was renamed, a module) continues the
    /// unmatched old container of the same kind that more than half of its children came from.
    fn by_children(&mut self) {
        let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (index, new) in self.news.iter().enumerate() {
            if let Some(container) = new.container {
                children.entry(container).or_default().push(index);
            }
        }
        let old_by_id: BTreeMap<AstNodeId, usize> = self
            .olds
            .iter()
            .enumerate()
            .map(|(index, old)| (old.node.id, index))
            .collect();
        // Innermost containers first, so an outer container can count an inner one's match.
        let mut containers: Vec<usize> = children.keys().copied().collect();
        containers.sort_by_key(|&container| std::cmp::Reverse(self.depth(container)));
        for container in containers {
            if self.new_to_old[container].is_some() {
                continue;
            }
            let mut votes: BTreeMap<usize, usize> = BTreeMap::new();
            for &child in &children[&container] {
                let origin = self.new_to_old[child]
                    .and_then(|old| self.olds[old].container_id)
                    .and_then(|id| old_by_id.get(&id).copied());
                if let Some(old) = origin {
                    *votes.entry(old).or_default() += 1;
                }
            }
            let kind = self.news[container].entity.kind;
            let winner = votes
                .iter()
                .filter(|&(&old, _)| !self.old_used[old] && self.olds[old].node.kind == kind)
                .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)));
            if let Some((&old, &count)) = winner
                && count * 2 > children[&container].len()
            {
                self.assign(container, old);
            }
        }
    }

    fn depth(&self, mut new: usize) -> usize {
        let mut depth = 0;
        while let Some(container) = self.news[new].container {
            depth += 1;
            new = container;
        }
        depth
    }

    /// Whether a pair shares a name, a body hash or a container.
    fn related(&self, old: usize, new: usize) -> bool {
        let (old, new_entity) = (&self.olds[old], &self.news[new]);
        old.node.name == new_entity.entity.name
            || old.node.body_hash == new_entity.entity.body_hash
            || (old.container_id.is_some() && old.container_id == self.container_id(new))
    }

    /// The id the new entity's container continues, if it is matched.
    pub(crate) fn container_id(&self, new: usize) -> Option<AstNodeId> {
        let container = self.news[new].container?;
        self.new_to_old[container].map(|old| self.olds[old].node.id)
    }

    /// The similarity of a pair in thousandths (research R5).
    fn score(&self, old: usize, new: usize) -> u32 {
        let (old_entity, new_entity) = (&self.olds[old], &self.news[new]);
        let (old_node, entity) = (old_entity.node, new_entity.entity);
        let signature = match old_entity.tokens {
            Some(tokens) => dice(
                without_name(&tokens.signature, &old_node.name),
                without_name(&entity.signature_tokens, &entity.name),
            ),
            None => equal_score(old_node.signature_hash == entity.signature_hash),
        };
        let body = if old_node.body_hash == entity.body_hash {
            FULL
        } else {
            match old_entity.tokens {
                Some(tokens) => dice(tokens.body.iter().collect(), entity.body_tokens.iter().collect()),
                None => 0,
            }
        };
        let scope = match (old_entity.container_id, self.news[new].container) {
            (None, None) if old_entity.unit == new_entity.unit => FULL,
            (Some(old_container), Some(_)) if Some(old_container) == self.container_id(new) => FULL,
            _ if old_entity.unit == new_entity.unit => FULL / 2,
            _ => 0,
        };
        (SIGNATURE_WEIGHT * signature + BODY_WEIGHT * body + SCOPE_WEIGHT * scope) / FULL
    }
}

fn equal_score(equal: bool) -> u32 {
    if equal { FULL } else { 0 }
}

/// The tokens without the first identifier equal to the entity's own name.
pub(crate) fn without_name<'t>(tokens: &'t [NormalizedToken], name: &str) -> Vec<&'t NormalizedToken> {
    let mut skipped = false;
    tokens
        .iter()
        .filter(|token| {
            if !skipped && token.text == name {
                skipped = true;
                return false;
            }
            true
        })
        .collect()
}

/// The Dice coefficient of two token multisets, in thousandths; two empty sets are equal.
fn dice(mut left: Vec<&NormalizedToken>, mut right: Vec<&NormalizedToken>) -> u32 {
    if left.is_empty() && right.is_empty() {
        return FULL;
    }
    let key = |token: &&NormalizedToken| (token.class.code(), token.text.clone());
    left.sort_by_key(key);
    right.sort_by_key(key);
    let (mut i, mut j, mut common) = (0, 0, 0u64);
    while i < left.len() && j < right.len() {
        match key(&left[i]).cmp(&key(&right[j])) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let total = (left.len() + right.len()) as u64;
    u32::try_from(2 * common * u64::from(FULL) / total).unwrap_or(FULL)
}

fn build_maps(
    change: &ChangeSet,
    news: &[NewEntity<'_>],
    ids: &[AstNodeId],
    carried: &BTreeMap<&str, Vec<&AnchoredNode>>,
) -> Vec<BlobAstMap> {
    let mut by_position: BTreeMap<(usize, usize), AstNodeId> = BTreeMap::new();
    for (new, id) in news.iter().zip(ids) {
        by_position.insert((new.file, new.index), *id);
    }
    let mut maps: Vec<BlobAstMap> = change
        .new
        .iter()
        .enumerate()
        .map(|(file_index, file)| {
            let mut nodes: Vec<AnchoredNode> = file
                .syntax
                .entities
                .iter()
                .enumerate()
                .map(|(index, entity)| AnchoredNode {
                    id: by_position[&(file_index, index)],
                    kind: entity.kind,
                    name: entity.name.clone(),
                    qualified_path: entity.qualified_path.clone(),
                    occurrence: entity.occurrence,
                    byte_range: Some(entity.byte_range),
                    container: entity.container,
                    structural_hash: entity.structural_hash,
                    signature_hash: entity.signature_hash,
                    body_hash: entity.body_hash,
                    has_error: entity.has_error,
                })
                .collect();
            let mut detached: Vec<AnchoredNode> = carried
                .get(file.syntax.path.as_str())
                .into_iter()
                .flatten()
                .map(|node| AnchoredNode {
                    byte_range: None,
                    container: None,
                    ..(*node).clone()
                })
                .collect();
            detached.sort_by(|left, right| {
                (&left.qualified_path, left.kind, left.occurrence, left.id).cmp(&(
                    &right.qualified_path,
                    right.kind,
                    right.occurrence,
                    right.id,
                ))
            });
            nodes.extend(detached);
            BlobAstMap {
                source_blob: file.blob,
                path: file.syntax.path.clone(),
                unit: file.syntax.unit.clone(),
                language: file.syntax.language,
                hash_format: file.syntax.hash_format,
                match_format: MATCH_FORMAT,
                nodes,
                has_error: file.syntax.has_error,
            }
        })
        .collect();
    maps.sort_by(|left, right| left.path.cmp(&right.path));
    maps
}
