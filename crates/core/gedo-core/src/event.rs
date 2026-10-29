//! Events: immutable records of one change of one code entity, with their clock, author and
//! labels, and the tapes that group the events of one publish (`specs/004-event-model/data-model.md`).
//!
//! These values are stored with postcard, which encodes an enum variant by its position: never
//! reorder or remove a variant of an enum used here; append new variants at the end. Ids are
//! derived from content by `gedo-crdt`; this module holds the types and their invariants.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::identity::{AstNodeId, SemanticDelta};
use crate::syntax::StructuralHash;

/// Most labels on one event (ADR-0013).
pub const MAX_LABELS: usize = 16;
/// Longest label key or agent identifier, in bytes (ADR-0013).
pub const MAX_LABEL_KEY_BYTES: usize = 64;
/// Longest label value, in bytes (ADR-0013).
pub const MAX_LABEL_VALUE_BYTES: usize = 256;
/// Label keys with this prefix belong to Gedo; tools cannot record them (ADR-0013).
pub const RESERVED_LABEL_PREFIX: &str = "gedo.";
/// Most events in one tape.
pub const MAX_TAPE_EVENTS: usize = 65_536;

macro_rules! uuid_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub [u8; 16]);

        impl $name {
            /// The name of the id in a collection: 32 lowercase hex digits.
            pub fn entry_name(&self) -> String {
                self.0.iter().map(|byte| format!("{byte:02x}")).collect()
            }

            /// Parses a collection entry name written by [`Self::entry_name`].
            pub fn from_entry_name(name: &str) -> Option<Self> {
                if name.len() != 32 || !name.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
                    return None;
                }
                let mut bytes = [0u8; 16];
                for (index, byte) in bytes.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&name[index * 2..index * 2 + 2], 16).ok()?;
                }
                Some($name(bytes))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for (index, byte) in self.0.iter().enumerate() {
                    if matches!(index, 4 | 6 | 8 | 10) {
                        f.write_str("-")?;
                    }
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({self})", stringify!($name))
            }
        }
    };
}

uuid_id!(
    /// The id of an event: 16 bytes derived from its content, displayed as a UUID version 8.
    EventId
);
uuid_id!(
    /// The id of a tape: 16 bytes derived from its content, displayed as a UUID version 8.
    TapeId
);

/// An event's position in logical time (research R3).
///
/// The generation is the Lamport time of the commit graph (1 for a root, one more than the highest
/// parent otherwise); the sequence orders the events of one commit or one publish. Ordered by
/// generation, then sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Clock {
    /// The commit generation.
    pub generation: u64,
    /// The position within the generation.
    pub sequence: u32,
}

/// A one-way hash of a person's Git e-mail (research R4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuthorFingerprint(pub [u8; 32]);

impl fmt::Display for AuthorFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for AuthorFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AuthorFingerprint({self})")
    }
}

/// Why a label, a label text or an agent identifier was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    /// A key or an agent identifier breaks the character or length rule.
    #[error(
        "{what} {value:?} must be 1 to 64 bytes of a-z, 0-9, '.', '-' or '_', starting with a letter or digit"
    )]
    InvalidName {
        /// `label key` or `agent identifier`.
        what: &'static str,
        /// The refused text.
        value: String,
    },
    /// A value is empty, too long or holds a control character.
    #[error("label value {value:?} of {key} must be 1 to 256 bytes without control characters")]
    InvalidValue {
        /// The key of the value.
        key: String,
        /// The refused value.
        value: String,
    },
    /// More labels than [`MAX_LABELS`].
    #[error("{count} labels given; at most 16 are allowed")]
    TooMany {
        /// The number given.
        count: usize,
    },
    /// A key given twice.
    #[error("label {key} given twice")]
    Duplicate {
        /// The key.
        key: String,
    },
    /// A key with the prefix reserved for Gedo.
    #[error("label {key} uses the prefix gedo., which is reserved for Gedo")]
    Reserved {
        /// The key.
        key: String,
    },
    /// The text form could not be read.
    #[error("labels text at byte {position}: {reason}")]
    Malformed {
        /// Byte offset of the problem in the text.
        position: usize,
        /// What is wrong.
        reason: String,
    },
}

fn valid_name(text: &str) -> bool {
    let bytes = text.as_bytes();
    (1..=MAX_LABEL_KEY_BYTES).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

/// The identifier of a coding agent (research R5).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Validates an agent identifier.
    pub fn new(text: &str) -> Result<Self, LabelError> {
        if valid_name(text) {
            Ok(AgentId(text.to_owned()))
        } else {
            Err(LabelError::InvalidName {
                what: "agent identifier",
                value: text.to_owned(),
            })
        }
    }

    /// The identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AgentId({})", self.0)
    }
}

/// Who made an event: a person, or a coding agent running on a person's Git identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuthorKind {
    /// A person.
    Person,
    /// A coding agent with its identifier.
    Agent(AgentId),
}

/// The author of an event.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Author {
    /// The fingerprint of the Git identity the work was done under.
    pub fingerprint: AuthorFingerprint,
    /// A person or an agent.
    pub kind: AuthorKind,
}

/// The key of a label: the same rule as an agent identifier.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LabelKey(String);

impl LabelKey {
    /// Validates a label key.
    pub fn new(text: &str) -> Result<Self, LabelError> {
        if valid_name(text) {
            Ok(LabelKey(text.to_owned()))
        } else {
            Err(LabelError::InvalidName {
                what: "label key",
                value: text.to_owned(),
            })
        }
    }

    /// The key as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the key uses the prefix reserved for Gedo.
    pub fn is_reserved(&self) -> bool {
        self.0.starts_with(RESERVED_LABEL_PREFIX)
    }
}

impl fmt::Display for LabelKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for LabelKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LabelKey({})", self.0)
    }
}

/// The value of a label: 1 to 256 bytes of UTF-8 without control characters, never interpreted.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LabelValue(String);

impl LabelValue {
    /// Validates the value of `key`.
    pub fn new(key: &LabelKey, text: &str) -> Result<Self, LabelError> {
        if (1..=MAX_LABEL_VALUE_BYTES).contains(&text.len()) && !text.chars().any(char::is_control) {
            Ok(LabelValue(text.to_owned()))
        } else {
            Err(LabelError::InvalidValue {
                key: key.to_string(),
                value: text.to_owned(),
            })
        }
    }

    /// The value as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LabelValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for LabelValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LabelValue({:?})", self.0)
    }
}

/// Opaque labels set by tools built on Gedo, sorted by key (ADR-0013).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Labels(BTreeMap<LabelKey, LabelValue>);

impl Labels {
    /// No labels.
    pub fn new() -> Self {
        Labels::default()
    }

    /// Labels from key and value texts; duplicates, invalid texts and more than [`MAX_LABELS`] are
    /// refused. Reserved keys are accepted here; see [`Labels::for_recording`].
    pub fn from_pairs<K: AsRef<str>, V: AsRef<str>>(pairs: &[(K, V)]) -> Result<Self, LabelError> {
        let mut labels = Labels::new();
        for (key, value) in pairs {
            let key = LabelKey::new(key.as_ref())?;
            let value = LabelValue::new(&key, value.as_ref())?;
            labels.insert(key, value)?;
        }
        Ok(labels)
    }

    /// Adds one label; a key already present or a seventeenth label is refused.
    pub fn insert(&mut self, key: LabelKey, value: LabelValue) -> Result<(), LabelError> {
        if self.0.contains_key(&key) {
            return Err(LabelError::Duplicate { key: key.to_string() });
        }
        if self.0.len() == MAX_LABELS {
            return Err(LabelError::TooMany {
                count: MAX_LABELS + 1,
            });
        }
        self.0.insert(key, value);
        Ok(())
    }

    /// These labels if a tool may record them: no key uses the reserved prefix.
    pub fn for_recording(self) -> Result<Self, LabelError> {
        match self.0.keys().find(|key| key.is_reserved()) {
            Some(key) => Err(LabelError::Reserved { key: key.to_string() }),
            None => Ok(self),
        }
    }

    /// The value of `key`.
    pub fn get(&self, key: &str) -> Option<&LabelValue> {
        self.0
            .iter()
            .find(|(name, _)| name.as_str() == key)
            .map(|(_, value)| value)
    }

    /// The labels in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&LabelKey, &LabelValue)> {
        self.0.iter()
    }

    /// The number of labels.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no labels.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Checks the rules on labels read back from storage (reserved keys allowed).
    pub fn check(&self) -> Result<(), LabelError> {
        if self.0.len() > MAX_LABELS {
            return Err(LabelError::TooMany { count: self.0.len() });
        }
        for (key, value) in &self.0 {
            LabelKey::new(key.as_str())?;
            LabelValue::new(key, value.as_str())?;
        }
        Ok(())
    }
}

/// What an event did.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SemanticOp {
    /// A semantic delta of one entity (feature 003).
    Delta(SemanticDelta),
    /// Two identities found for one entity: `merged` resolves to `kept` from now on.
    IdentityMerged {
        /// The lower id, kept.
        kept: AstNodeId,
        /// The higher id, now an alias of `kept`.
        merged: AstNodeId,
    },
}

impl SemanticOp {
    /// The entity the operation concerns.
    pub fn target(&self) -> AstNodeId {
        match self {
            SemanticOp::Delta(delta) => delta.id(),
            SemanticOp::IdentityMerged { merged, .. } => *merged,
        }
    }

    /// Whether the entity has no structural hash after the operation (a tombstone or a merge).
    pub fn has_no_result(&self) -> bool {
        matches!(
            self,
            SemanticOp::Delta(SemanticDelta::Tombstoned { .. }) | SemanticOp::IdentityMerged { .. }
        )
    }
}

/// Where an event comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Origin {
    /// Recorded while someone worked.
    Live,
    /// Derived from the Git history of a commit made without Gedo.
    Reconstructed,
}

/// One immutable change of one entity; its id is derived from these fields (research R2).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Event {
    /// The position in logical time.
    pub clock: Clock,
    /// The entity concerned; equal to `operation.target()`.
    pub target: AstNodeId,
    /// What happened.
    pub operation: SemanticOp,
    /// The entity's structural hash after the operation; `None` exactly for a tombstone and an
    /// identity merge.
    pub result_hash: Option<StructuralHash>,
    /// Who did it.
    pub author: Author,
    /// Opaque labels.
    pub labels: Labels,
    /// Live or reconstructed.
    pub origin: Origin,
}

impl Event {
    /// Checks the invariants of an event read back from storage.
    pub fn check(&self) -> Result<(), String> {
        if self.target != self.operation.target() {
            return Err(format!(
                "target {} is not the operation's {}",
                self.target,
                self.operation.target()
            ));
        }
        if self.operation.has_no_result() != self.result_hash.is_none() {
            return Err("result hash present for a tombstone or merge, or missing otherwise".into());
        }
        if let AuthorKind::Agent(agent) = &self.author.kind {
            AgentId::new(agent.as_str()).map_err(|error| error.to_string())?;
        }
        self.labels.check().map_err(|error| error.to_string())?;
        if self.origin == Origin::Reconstructed && !self.labels.is_empty() {
            return Err("a reconstructed event carries labels".into());
        }
        Ok(())
    }
}

/// The ordered event ids of one publish; no commit id, so rewrites leave it unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tape {
    /// The events in recording order.
    pub events: Vec<EventId>,
}

impl Tape {
    /// Checks the invariants: 1 to [`MAX_TAPE_EVENTS`] events, no duplicates.
    pub fn check(&self) -> Result<(), String> {
        if self.events.is_empty() || self.events.len() > MAX_TAPE_EVENTS {
            return Err(format!(
                "{} events; a tape holds 1 to {MAX_TAPE_EVENTS}",
                self.events.len()
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        match self.events.iter().find(|id| !seen.insert(**id)) {
            Some(id) => Err(format!("event {id} listed twice")),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Location;
    use crate::syntax::EntityKind;

    fn location() -> Location {
        Location {
            unit: "src".into(),
            path: "src/lib.rs".into(),
            qualified_path: "crate::f".into(),
        }
    }

    fn event(operation: SemanticOp, result_hash: Option<StructuralHash>) -> Event {
        Event {
            clock: Clock {
                generation: 1,
                sequence: 0,
            },
            target: operation.target(),
            operation,
            result_hash,
            author: Author {
                fingerprint: AuthorFingerprint([7; 32]),
                kind: AuthorKind::Person,
            },
            labels: Labels::new(),
            origin: Origin::Live,
        }
    }

    fn created() -> SemanticOp {
        SemanticOp::Delta(SemanticDelta::Created {
            id: AstNodeId([1; 16]),
            kind: EntityKind::Function,
            at: location(),
        })
    }

    #[test]
    fn ids_display_as_uuids_and_round_trip_as_entry_names() {
        let id = EventId([0xab; 16]);
        assert_eq!(id.to_string(), "abababab-abab-abab-abab-abababababab");
        assert_eq!(id.entry_name().len(), 32);
        assert_eq!(EventId::from_entry_name(&id.entry_name()), Some(id));
        for refused in ["", "ab", &"AB".repeat(16), &"zz".repeat(16), &"ab".repeat(17)] {
            assert_eq!(EventId::from_entry_name(refused), None, "{refused}");
        }
    }

    #[test]
    fn clocks_order_by_generation_then_sequence() {
        let clock = |generation, sequence| Clock { generation, sequence };
        assert!(clock(1, 9) < clock(2, 0));
        assert!(clock(2, 0) < clock(2, 1));
    }

    #[test]
    fn names_follow_the_character_rule() {
        for accepted in ["dc.plan", "a", "0-x_y.z", &"a".repeat(64)] {
            assert!(LabelKey::new(accepted).is_ok(), "{accepted}");
            assert!(AgentId::new(accepted).is_ok(), "{accepted}");
        }
        for refused in ["", "Plan", ".plan", "-x", "a b", "é", &"a".repeat(65)] {
            assert!(LabelKey::new(refused).is_err(), "{refused}");
            assert!(AgentId::new(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn values_follow_the_length_and_control_rules() {
        let key = LabelKey::new("dc.plan").expect("valid");
        assert!(LabelValue::new(&key, "gedo_event_model").is_ok());
        assert!(LabelValue::new(&key, "milestone M2, ação").is_ok());
        assert!(LabelValue::new(&key, &"x".repeat(256)).is_ok());
        for refused in ["", "a\nb", "\u{7}", &"x".repeat(257)] {
            assert!(LabelValue::new(&key, refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn labels_refuse_duplicates_excess_and_reserved_keys_when_recording() {
        let labels = Labels::from_pairs(&[("dc.plan", "p"), ("dc.milestone", "M2")]).expect("valid");
        assert_eq!(labels.len(), 2);
        assert_eq!(labels.get("dc.milestone").map(LabelValue::as_str), Some("M2"));
        assert!(labels.clone().for_recording().is_ok());

        assert!(matches!(
            Labels::from_pairs(&[("a", "1"), ("a", "2")]),
            Err(LabelError::Duplicate { .. })
        ));
        let many: Vec<(String, String)> = (0..17).map(|n| (format!("k{n}"), "v".into())).collect();
        assert!(matches!(
            Labels::from_pairs(&many),
            Err(LabelError::TooMany { .. })
        ));

        let reserved = Labels::from_pairs(&[("gedo.stream", "s")]).expect("readable");
        assert!(reserved.check().is_ok(), "reserved keys are valid when read back");
        assert!(matches!(
            reserved.for_recording(),
            Err(LabelError::Reserved { .. })
        ));
    }

    #[test]
    fn events_check_their_invariants() {
        assert!(event(created(), Some(StructuralHash([2; 32]))).check().is_ok());
        assert!(event(created(), None).check().is_err(), "a creation has a result");

        let merge = SemanticOp::IdentityMerged {
            kept: AstNodeId([1; 16]),
            merged: AstNodeId([2; 16]),
        };
        assert_eq!(merge.target(), AstNodeId([2; 16]));
        assert!(event(merge.clone(), None).check().is_ok());
        assert!(event(merge, Some(StructuralHash([2; 32]))).check().is_err());

        let mut wrong_target = event(created(), Some(StructuralHash([2; 32])));
        wrong_target.target = AstNodeId([9; 16]);
        assert!(wrong_target.check().is_err());

        let mut labelled = event(created(), Some(StructuralHash([2; 32])));
        labelled.labels = Labels::from_pairs(&[("dc.plan", "p")]).expect("valid");
        assert!(labelled.check().is_ok());
        labelled.origin = Origin::Reconstructed;
        assert!(labelled.check().is_err(), "reconstructed events have no labels");
    }

    #[test]
    fn tapes_hold_one_to_the_limit_of_distinct_events() {
        assert!(Tape { events: vec![] }.check().is_err());
        assert!(
            Tape {
                events: vec![EventId([1; 16]), EventId([2; 16])]
            }
            .check()
            .is_ok()
        );
        assert!(
            Tape {
                events: vec![EventId([1; 16]), EventId([1; 16])]
            }
            .check()
            .is_err()
        );
    }
}
