//! The tree CRDT: concurrent changes of the code converged per entity
//! (`specs/006-tree-crdt/research.md` R2 to R8).
//!
//! The state of the code is a set of entities keyed by identity, each with five parts compared on
//! their own: presence, place (unit, file and container path), name, signature and body. A side is
//! a set of events made after a common base; its state is the base with its events folded in the
//! total order of ADR-0014, each event applied once. Several sides converge part by part: a part
//! no side changed keeps the base value, a part changed by sides that agree takes their value, and
//! distinct values give a conflict that keeps the base value. Members of a container are a set
//! because membership is each member's own place; moves, renames and edits of different parts
//! commute; a removal against a change, a removed container against a new member and different
//! entities added on several sides at the same qualified path are conflicts (ADR-0009, ADR-0017).
//!
//! Everything here is pure and deterministic: the result depends only on the base and on the set
//! of events of each side, never on their order, their duplicates or the order of the sides
//! (conflicts name sides by their position in the input).

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use gedo_core::event::{Event, EventId, SemanticOp};
use gedo_core::identity::{AstNodeId, BlobAstMap, Location, SemanticDelta};
use gedo_core::syntax::{EntityKind, StructuralHash};

use crate::clock::order_key;

/// The separator of the parts of a qualified path. Every language supported today (Rust) uses
/// `::`; a language added later states its own (research R3).
const SEPARATOR: &str = "::";

/// How many alias steps are followed before an id is taken as resolved; a longer chain only comes
/// from corrupted data, and stopping keeps the fold total.
const MAX_ALIAS_STEPS: usize = 64;

/// A part of an entity, compared on its own (research R3), in the order conflicts are sorted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Part {
    /// Whether the entity exists.
    Presence,
    /// Its unit, file and container.
    Place,
    /// Its own name.
    Name,
    /// Its signature hash.
    Signature,
    /// Its body hash.
    Body,
}

/// Every part, in order.
pub const PARTS: [Part; 5] = [
    Part::Presence,
    Part::Place,
    Part::Name,
    Part::Signature,
    Part::Body,
];

/// Where an entity is: its unit, its file and its container's qualified path (empty at the root).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Place {
    /// The compilation unit.
    pub unit: String,
    /// The file's path.
    pub path: String,
    /// The qualified path of the container: the entity's qualified path without its own name.
    pub container: String,
}

/// One entity of a tree state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityState {
    /// The neutral kind.
    pub kind: EntityKind,
    /// Whether the entity exists; `false` after a removal.
    pub present: bool,
    /// Where it is.
    pub place: Place,
    /// Its own name: the last part of its qualified path.
    pub name: String,
    /// The signature hash; `None` for an entity created on a side whose signature no event names.
    pub signature: Option<StructuralHash>,
    /// The body hash; `None` like the signature.
    pub body: Option<StructuralHash>,
    /// The structural hash of the whole entity; `None` when unknown (parts converged from
    /// different sides, which only the merge driver can render and hash).
    pub structural: Option<StructuralHash>,
}

impl EntityState {
    /// The qualified path: the container and the name.
    pub fn qualified_path(&self) -> String {
        join_qualified(&self.place.container, &self.name)
    }

    /// The value of one part.
    pub fn value(&self, part: Part) -> PartValue {
        match part {
            Part::Presence => PartValue::Presence(self.present),
            Part::Place => PartValue::Place(self.place.clone()),
            Part::Name => PartValue::Name(self.name.clone()),
            Part::Signature => PartValue::Signature(self.signature),
            Part::Body => PartValue::Body(self.body),
        }
    }

    /// Whether one part is equal in two versions, without copying it.
    pub fn same(&self, other: &EntityState, part: Part) -> bool {
        match part {
            Part::Presence => self.present == other.present,
            Part::Place => self.place == other.place,
            Part::Name => self.name == other.name,
            Part::Signature => self.signature == other.signature,
            Part::Body => self.body == other.body,
        }
    }

    /// Sets one part; a value of another part (or an entity) is ignored.
    fn set(&mut self, value: PartValue) {
        match value {
            PartValue::Presence(present) => self.present = present,
            PartValue::Place(place) => self.place = place,
            PartValue::Name(name) => self.name = name,
            PartValue::Signature(signature) => self.signature = signature,
            PartValue::Body(body) => self.body = body,
            PartValue::Entity(_) => {}
        }
    }
}

/// The value of a part, or an entity named by a conflict.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PartValue {
    /// Presence.
    Presence(bool),
    /// Place.
    Place(Place),
    /// Name.
    Name(String),
    /// Signature hash.
    Signature(Option<StructuralHash>),
    /// Body hash.
    Body(Option<StructuralHash>),
    /// One of the entities of an added-on-both-sides conflict.
    Entity(AstNodeId),
}

/// The entities of a version of the code, keyed by identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeState {
    entities: BTreeMap<AstNodeId, EntityState>,
}

impl TreeState {
    /// An empty state.
    pub fn new() -> Self {
        TreeState::default()
    }

    /// The state of the entities of the given file versions (the identity results of a commit).
    /// Every node is present; a node carried while its file has a syntax error keeps its place.
    pub fn from_maps(maps: &[BlobAstMap]) -> Self {
        let mut entities = BTreeMap::new();
        for map in maps {
            for node in &map.nodes {
                let (container, name) = split_qualified(&node.qualified_path);
                entities.entry(node.id).or_insert_with(|| EntityState {
                    kind: node.kind,
                    present: true,
                    place: Place {
                        unit: map.unit.clone(),
                        path: map.path.clone(),
                        container: container.to_owned(),
                    },
                    name: name.to_owned(),
                    signature: Some(node.signature_hash),
                    body: Some(node.body_hash),
                    structural: Some(node.structural_hash),
                });
            }
        }
        TreeState { entities }
    }

    /// Adds or replaces one entity.
    pub fn insert(&mut self, id: AstNodeId, entity: EntityState) {
        self.entities.insert(id, entity);
    }

    /// One entity, removed ones included.
    pub fn entity(&self, id: AstNodeId) -> Option<&EntityState> {
        self.entities.get(&id)
    }

    /// Every entity by id, removed ones included.
    pub fn entities(&self) -> &BTreeMap<AstNodeId, EntityState> {
        &self.entities
    }

    /// The present entities of the file `path` whose container is `container`, sorted by id
    /// (FR-013); the order of members in the file is the merge driver's (FEAT-070).
    pub fn members(&self, path: &str, container: &str) -> Vec<AstNodeId> {
        self.entities
            .iter()
            .filter(|(_, entity)| {
                entity.present && entity.place.path == path && entity.place.container == container
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// The state with every id resolved: an entity whose id is an alias of another goes under the
    /// kept id, unless the kept id has its own entity.
    fn resolved(&self, resolve: &impl Fn(AstNodeId) -> AstNodeId) -> Cow<'_, TreeState> {
        if self.entities.keys().all(|id| resolve(*id) == *id) {
            return Cow::Borrowed(self);
        }
        let mut entities = BTreeMap::new();
        for (id, entity) in &self.entities {
            let kept = resolve(*id);
            if kept == *id || !self.entities.contains_key(&kept) {
                entities.entry(kept).or_insert_with(|| entity.clone());
            }
        }
        Cow::Owned(TreeState { entities })
    }
}

/// The state of one side: the entities its events touched over the base, with the last event
/// that set each part (research R4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SideState {
    /// The entities the side's events touched, as the side leaves them; every other entity is as
    /// in the base.
    pub touched: BTreeMap<AstNodeId, EntityState>,
    /// The last event of the side that set each part of an entity.
    pub set_by: BTreeMap<(AstNodeId, Part), EventId>,
    /// The last event of the side about each entity.
    pub last_event: BTreeMap<AstNodeId, EventId>,
    /// Entities the base does not have, created on the side and still present.
    pub created: BTreeSet<AstNodeId>,
    /// Entities created and removed on the side: they leave no trace.
    pub neutral: BTreeSet<AstNodeId>,
}

impl SideState {
    /// The side's version of an entity: touched, or as in the base.
    pub fn entity<'a>(&'a self, base: &'a TreeState, id: AstNodeId) -> Option<&'a EntityState> {
        self.touched.get(&id).or_else(|| base.entity(id))
    }
}

/// An event a side cannot fold.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TreeError {
    /// The event is about an entity that is neither in the base nor created earlier on its side.
    #[error("event {event} changes entity {entity}, which is neither in the base nor created before it")]
    UnknownEntity {
        /// The event.
        event: EventId,
        /// Its entity, after identity merges.
        entity: AstNodeId,
    },
}

/// A fold error with the position of its side in the input.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("side {side}: {error}")]
pub struct SideError {
    /// The side's position.
    pub side: usize,
    /// What went wrong.
    #[source]
    pub error: TreeError,
}

/// What a conflict is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConflictKind {
    /// Sides set one part of the entity to distinct values; a removal against a change is a
    /// conflict on [`Part::Presence`].
    Part(Part),
    /// Different entities with the same kind and qualified path were added on several sides; the
    /// conflict is on the lowest id and names the others.
    AddedOnBothSides,
    /// The entity, a container, was removed on a side while other sides added or moved members
    /// into it; the conflict names those members.
    RemovedContainer,
}

/// One of the values of a conflict, with the sides that hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictValue {
    /// The value.
    pub value: PartValue,
    /// The positions of the sides that hold it, sorted.
    pub sides: Vec<usize>,
    /// For each side, in the order of `sides`, the event that set the value.
    pub events: Vec<EventId>,
}

/// A part of an entity on which sides disagree (research R8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The entity.
    pub entity: AstNodeId,
    /// What the conflict is about.
    pub kind: ConflictKind,
    /// The base value of the part; `None` for an entity the base does not have.
    pub base: Option<PartValue>,
    /// Each distinct value with its sides, sorted by value.
    pub values: Vec<ConflictValue>,
    /// For a removed container, the members added or moved into it; for entities added on both
    /// sides, the other entities. Sorted.
    pub others: Vec<AstNodeId>,
}

/// The result of converging a base and several sides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Convergence {
    /// The converged entities: every change that does not conflict applied, every conflicting part
    /// at its base value, entities added on both sides with different content absent.
    pub state: TreeState,
    /// Identities found for one entity added on several sides with equal content, as
    /// `(kept, merged)` with the lower id kept (ADR-0007), sorted.
    pub links: Vec<(AstNodeId, AstNodeId)>,
    /// The conflicts, sorted by entity then kind.
    pub conflicts: Vec<Conflict>,
}

impl Convergence {
    /// Whether no conflict was found.
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// Splits a qualified path at its last separator outside angle brackets into the container's
/// qualified path and the entity's own name: `crate::m::<impl fmt::Display for X>::fmt` gives
/// `crate::m::<impl fmt::Display for X>` and `fmt`. A path without separator is a name at the root.
pub fn split_qualified(qualified_path: &str) -> (&str, &str) {
    let bytes = qualified_path.as_bytes();
    let mut depth = 0usize;
    let mut split = None;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'<' => depth += 1,
            // `->` inside a type is not a closing bracket.
            b'>' if index == 0 || bytes[index - 1] != b'-' => depth = depth.saturating_sub(1),
            b':' if depth == 0 && bytes.get(index + 1) == Some(&b':') => {
                split = Some(index);
                index += 1;
            }
            _ => {}
        }
        index += 1;
    }
    match split {
        Some(at) => (&qualified_path[..at], &qualified_path[at + SEPARATOR.len()..]),
        None => ("", qualified_path),
    }
}

/// Joins a container's qualified path and a name.
pub fn join_qualified(container: &str, name: &str) -> String {
    if container.is_empty() {
        name.to_owned()
    } else {
        format!("{container}{SEPARATOR}{name}")
    }
}

/// The place and name a location gives.
fn place_of(location: &Location) -> (Place, String) {
    let (container, name) = split_qualified(&location.qualified_path);
    (
        Place {
            unit: location.unit.clone(),
            path: location.path.clone(),
            container: container.to_owned(),
        },
        name.to_owned(),
    )
}

/// Identity merges as an alias table over an outer resolution (the merges the caller knows).
struct Aliases<'a> {
    merged: BTreeMap<AstNodeId, AstNodeId>,
    outer: &'a dyn Fn(AstNodeId) -> AstNodeId,
}

impl<'a> Aliases<'a> {
    fn new<'e>(
        events: impl IntoIterator<Item = &'e Event>,
        outer: &'a dyn Fn(AstNodeId) -> AstNodeId,
    ) -> Self {
        let mut merged = BTreeMap::new();
        for event in events {
            if let SemanticOp::IdentityMerged { kept, merged: alias } = event.operation {
                // The lower id is kept whatever the record says (ADR-0007).
                let (low, high) = (kept.min(alias), kept.max(alias));
                if low != high {
                    let entry = merged.entry(high).or_insert(low);
                    *entry = (*entry).min(low);
                }
            }
        }
        Aliases { merged, outer }
    }

    fn resolve(&self, id: AstNodeId) -> AstNodeId {
        let mut current = (self.outer)(id);
        for _ in 0..MAX_ALIAS_STEPS {
            match self.merged.get(&current) {
                Some(next) => current = (self.outer)(*next),
                None => break,
            }
        }
        current
    }
}

/// The events of a list, each once, in the total order of ADR-0014.
fn ordered(events: &[Event]) -> Vec<(EventId, &Event)> {
    let unique: BTreeMap<_, &Event> = events.iter().map(|event| (order_key(event), event)).collect();
    unique.into_iter().map(|((_, id), event)| (id, event)).collect()
}

/// Folds one side's events over `base` (research R4). Ids are resolved through the identity merges
/// among the events.
pub fn fold(base: &TreeState, events: &[Event]) -> Result<SideState, TreeError> {
    let outer = |id: AstNodeId| id;
    let aliases = Aliases::new(events, &outer);
    fold_resolved(base, events, &aliases)
}

fn fold_resolved(base: &TreeState, events: &[Event], aliases: &Aliases<'_>) -> Result<SideState, TreeError> {
    let mut side = SideState::default();
    for (event_id, event) in ordered(events) {
        let SemanticOp::Delta(delta) = &event.operation else {
            // Identity merges are applied through the aliases.
            continue;
        };
        let id = aliases.resolve(delta.id());
        let entity = match side.touched.entry(id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => match (base.entity(id), delta) {
                (Some(original), _) => entry.insert(original.clone()),
                (None, SemanticDelta::Created { kind, at, .. }) => {
                    side.created.insert(id);
                    let (place, name) = place_of(at);
                    entry.insert(EntityState {
                        kind: *kind,
                        present: true,
                        place,
                        name,
                        signature: None,
                        body: None,
                        structural: None,
                    })
                }
                (None, _) => {
                    return Err(TreeError::UnknownEntity {
                        event: event_id,
                        entity: id,
                    });
                }
            },
        };
        let parts: &[Part] = match delta {
            SemanticDelta::Created { kind, at, .. } => {
                let (place, name) = place_of(at);
                entity.kind = *kind;
                entity.present = true;
                entity.place = place;
                entity.name = name;
                &[Part::Presence, Part::Place, Part::Name]
            }
            SemanticDelta::Moved { to, .. } => {
                let (place, name) = place_of(to);
                entity.place = place;
                entity.name = name;
                &[Part::Place, Part::Name]
            }
            SemanticDelta::Renamed { qualified_path, .. } => {
                entity.name = split_qualified(qualified_path).1.to_owned();
                &[Part::Name]
            }
            SemanticDelta::SignatureMutated { after, .. } => {
                entity.signature = Some(*after);
                &[Part::Signature]
            }
            SemanticDelta::BodyModified { after, .. } => {
                entity.body = Some(*after);
                &[Part::Body]
            }
            SemanticDelta::Tombstoned { .. } => {
                entity.present = false;
                &[Part::Presence]
            }
        };
        if let Some(hash) = event.result_hash {
            entity.structural = Some(hash);
        }
        for part in parts {
            side.set_by.insert((id, *part), event_id);
        }
        side.last_event.insert(id, event_id);
    }
    let gone: Vec<AstNodeId> = side
        .created
        .iter()
        .copied()
        .filter(|id| side.touched.get(id).is_some_and(|entity| !entity.present))
        .collect();
    for id in &gone {
        side.created.remove(id);
        side.touched.remove(id);
        side.last_event.remove(id);
        side.neutral.insert(*id);
    }
    if !gone.is_empty() {
        side.set_by
            .retain(|(entity, _), _| !side.neutral.contains(entity));
    }
    Ok(side)
}

/// Converges a base and any number of sides (research R5 to R8). With one side, the result is that
/// side's state with no conflict.
pub fn converge(base: &TreeState, sides: &[Vec<Event>]) -> Result<Convergence, SideError> {
    converge_with(base, sides, |id| id)
}

/// [`converge`] with the identity merges the caller already knows (the identity index): every id
/// of the base and of the events is resolved through them and through the merges among the events.
pub fn converge_with(
    base: &TreeState,
    sides: &[Vec<Event>],
    resolve: impl Fn(AstNodeId) -> AstNodeId,
) -> Result<Convergence, SideError> {
    let aliases = Aliases::new(sides.iter().flatten(), &resolve);
    let base = base.resolved(&|id| aliases.resolve(id));
    let base = base.as_ref();
    let folded = sides
        .iter()
        .enumerate()
        .map(|(side, events)| {
            fold_resolved(base, events, &aliases).map_err(|error| SideError { side, error })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let touched: BTreeSet<AstNodeId> = folded
        .iter()
        .flat_map(|side| side.touched.keys().copied())
        .collect();
    let mut state = base.clone();
    let mut conflicts = Vec::new();
    for id in touched {
        match base.entity(id) {
            Some(original) => converge_existing(id, original, base, &folded, &mut state, &mut conflicts),
            None => converge_created(id, &folded, &mut state, &mut conflicts),
        }
    }
    let links = added_on_several_sides(base, &folded, &mut state, &mut conflicts);
    removed_containers(base, &folded, &mut state, &mut conflicts);
    conflicts.sort_by_key(|conflict| (conflict.entity, conflict.kind));
    Ok(Convergence {
        state,
        links,
        conflicts,
    })
}

/// Groups the distinct values the sides set for a part, each with its sides and events.
fn group(values: impl IntoIterator<Item = (PartValue, usize, EventId)>) -> Vec<ConflictValue> {
    let mut grouped: BTreeMap<PartValue, (Vec<usize>, Vec<EventId>)> = BTreeMap::new();
    for (value, side, event) in values {
        let entry = grouped.entry(value).or_default();
        entry.0.push(side);
        entry.1.push(event);
    }
    grouped
        .into_iter()
        .map(|(value, (sides, events))| ConflictValue { value, sides, events })
        .collect()
}

/// The event a side recorded for a part, or its last event about the entity.
fn event_for(side: &SideState, id: AstNodeId, part: Part) -> EventId {
    side.set_by
        .get(&(id, part))
        .or_else(|| side.last_event.get(&id))
        .copied()
        .unwrap_or(EventId([0; 16]))
}

/// Converges an entity of the base.
fn converge_existing(
    id: AstNodeId,
    original: &EntityState,
    base: &TreeState,
    folded: &[SideState],
    state: &mut TreeState,
    conflicts: &mut Vec<Conflict>,
) {
    let versions: Vec<(usize, &EntityState)> = folded
        .iter()
        .enumerate()
        .filter_map(|(index, side)| side.touched.get(&id).map(|entity| (index, entity)))
        .collect();
    let changed = |entity: &EntityState, part: Part| !entity.same(original, part);
    if let [(_, only)] = versions.as_slice() {
        // One side touched the entity: its version is the result.
        let mut result = (*only).clone();
        if PARTS.iter().all(|part| !changed(only, *part)) {
            result.structural = original.structural;
        }
        state.insert(id, result);
        return;
    }

    // A removal against a change of any other part (research R6).
    let removers: Vec<usize> = versions
        .iter()
        .filter(|(_, entity)| original.present && !entity.present)
        .map(|(index, _)| *index)
        .collect();
    let changers: Vec<usize> = versions
        .iter()
        .filter(|(_, entity)| entity.present && PARTS[1..].iter().any(|part| changed(entity, *part)))
        .map(|(index, _)| *index)
        .collect();
    if !removers.is_empty() && !changers.is_empty() {
        let values = removers
            .iter()
            .map(|&side| {
                (
                    PartValue::Presence(false),
                    side,
                    event_for(&folded[side], id, Part::Presence),
                )
            })
            .chain(changers.iter().map(|&side| {
                (
                    PartValue::Presence(true),
                    side,
                    folded[side]
                        .last_event
                        .get(&id)
                        .copied()
                        .unwrap_or(EventId([0; 16])),
                )
            }));
        conflicts.push(Conflict {
            entity: id,
            kind: ConflictKind::Part(Part::Presence),
            base: Some(original.value(Part::Presence)),
            values: group(values),
            others: Vec::new(),
        });
        return;
    }

    let mut result = original.clone();
    for part in PARTS {
        let values = group(
            versions
                .iter()
                .filter(|(_, entity)| changed(entity, part))
                .map(|(side, entity)| (entity.value(part), *side, event_for(&folded[*side], id, part))),
        );
        match values.len() {
            0 => {}
            1 => result.set(values[0].value.clone()),
            _ => conflicts.push(Conflict {
                entity: id,
                kind: ConflictKind::Part(part),
                base: Some(original.value(part)),
                values,
                others: Vec::new(),
            }),
        }
    }
    result.structural = if PARTS.iter().all(|part| result.same(original, *part)) {
        original.structural
    } else {
        // Known only when the sides that hold every part of the result agree on it.
        let hashes: BTreeSet<Option<StructuralHash>> = folded
            .iter()
            .filter_map(|side| side.entity(base, id))
            .filter(|entity| PARTS.iter().all(|part| entity.same(&result, *part)))
            .map(|entity| entity.structural)
            .collect();
        match hashes.len() {
            1 => hashes.into_iter().next().flatten(),
            _ => None,
        }
    };
    state.insert(id, result);
}

/// Converges an entity the base does not have, created on one or several sides.
fn converge_created(
    id: AstNodeId,
    folded: &[SideState],
    state: &mut TreeState,
    conflicts: &mut Vec<Conflict>,
) {
    let versions: Vec<(usize, &EntityState)> = folded
        .iter()
        .enumerate()
        .filter_map(|(index, side)| side.touched.get(&id).map(|entity| (index, entity)))
        .collect();
    let Some((_, first)) = versions.first() else {
        return;
    };
    let mut agreed = true;
    for part in PARTS {
        let values = group(
            versions
                .iter()
                .map(|(side, entity)| (entity.value(part), *side, event_for(&folded[*side], id, part))),
        );
        if values.len() > 1 {
            agreed = false;
            conflicts.push(Conflict {
                entity: id,
                kind: ConflictKind::Part(part),
                base: None,
                values,
                others: Vec::new(),
            });
        }
    }
    if agreed {
        let mut entity = (*first).clone();
        if versions
            .iter()
            .any(|(_, other)| other.structural != entity.structural)
        {
            entity.structural = None;
        }
        state.insert(id, entity);
    }
}

/// Different entities added on several sides with the same kind and qualified path (research
/// R7): one entity when their structural hashes are equal, a conflict otherwise. Implementation
/// blocks are left out: several blocks for one type are legal and have the same qualified path.
fn added_on_several_sides(
    base: &TreeState,
    folded: &[SideState],
    state: &mut TreeState,
    conflicts: &mut Vec<Conflict>,
) -> Vec<(AstNodeId, AstNodeId)> {
    let mut groups: BTreeMap<(EntityKind, String), BTreeMap<AstNodeId, Vec<usize>>> = BTreeMap::new();
    for (index, side) in folded.iter().enumerate() {
        for id in &side.created {
            if base.entity(*id).is_some() {
                continue;
            }
            let Some(entity) = state.entity(*id) else {
                continue;
            };
            if entity.kind == EntityKind::Impl || !entity.present {
                continue;
            }
            groups
                .entry((entity.kind, entity.qualified_path()))
                .or_default()
                .entry(*id)
                .or_default()
                .push(index);
        }
    }
    let mut links = Vec::new();
    for entities in groups.into_values() {
        let sides: BTreeSet<usize> = entities.values().flatten().copied().collect();
        if entities.len() < 2 || sides.len() < 2 {
            continue;
        }
        let ids: Vec<AstNodeId> = entities.keys().copied().collect();
        let kept = ids[0];
        let hashes: BTreeSet<Option<StructuralHash>> = ids
            .iter()
            .map(|id| state.entity(*id).and_then(|entity| entity.structural))
            .collect();
        if hashes.len() == 1 && hashes.iter().all(Option::is_some) {
            for merged in &ids[1..] {
                state.entities.remove(merged);
                links.push((kept, *merged));
            }
        } else {
            let values = entities.iter().flat_map(|(id, holders)| {
                holders.iter().map(move |side| {
                    (
                        PartValue::Entity(*id),
                        *side,
                        event_for(&folded[*side], *id, Part::Presence),
                    )
                })
            });
            conflicts.push(Conflict {
                entity: kept,
                kind: ConflictKind::AddedOnBothSides,
                base: None,
                values: group(values),
                others: ids[1..].to_vec(),
            });
            for id in &ids {
                state.entities.remove(id);
            }
        }
    }
    links.sort();
    links
}

/// Containers removed on a side while other sides added or moved members into them (research
/// R6): the container keeps its base state and the conflict names the members.
fn removed_containers(
    base: &TreeState,
    folded: &[SideState],
    state: &mut TreeState,
    conflicts: &mut Vec<Conflict>,
) {
    let mut removed: BTreeMap<(String, String), Vec<AstNodeId>> = BTreeMap::new();
    for (id, entity) in &state.entities {
        if !entity.present && base.entity(*id).is_some_and(|original| original.present) {
            removed
                .entry((entity.place.path.clone(), entity.qualified_path()))
                .or_default()
                .push(*id);
        }
    }
    if removed.is_empty() {
        return;
    }
    let mut members: BTreeMap<AstNodeId, BTreeSet<AstNodeId>> = BTreeMap::new();
    for (id, entity) in &state.entities {
        if !entity.present {
            continue;
        }
        let arrived = base
            .entity(*id)
            .is_none_or(|original| original.place != entity.place || !original.present);
        if !arrived {
            continue;
        }
        let key = (entity.place.path.clone(), entity.place.container.clone());
        for container in removed.get(&key).into_iter().flatten() {
            members.entry(*container).or_default().insert(*id);
        }
    }
    for (container, arrived) in members {
        let Some(original) = base.entity(container) else {
            continue;
        };
        let values = folded.iter().enumerate().filter_map(|(index, side)| {
            side.touched
                .get(&container)
                .filter(|entity| !entity.present)
                .map(|_| {
                    (
                        PartValue::Presence(false),
                        index,
                        event_for(side, container, Part::Presence),
                    )
                })
        });
        conflicts.push(Conflict {
            entity: container,
            kind: ConflictKind::RemovedContainer,
            base: Some(PartValue::Presence(true)),
            values: group(values),
            others: arrived.into_iter().collect(),
        });
        state.insert(container, original.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_paths_split_at_the_last_top_level_separator() {
        assert_eq!(split_qualified("crate::m::f"), ("crate::m", "f"));
        assert_eq!(split_qualified("f"), ("", "f"));
        assert_eq!(
            split_qualified("crate::m::<impl fmt::Display for X>::fmt"),
            ("crate::m::<impl fmt::Display for X>", "fmt")
        );
        assert_eq!(
            split_qualified("crate::<impl Fn() -> a::B for C>"),
            ("crate", "<impl Fn() -> a::B for C>")
        );
        assert_eq!(join_qualified("crate::m", "f"), "crate::m::f");
        assert_eq!(join_qualified("", "f"), "f");
    }
}
