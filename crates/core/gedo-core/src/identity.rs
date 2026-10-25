//! Node identity: the id that names a code entity for its whole life, the entities of one file
//! version anchored to their ids, and the semantic deltas between two versions
//! (`specs/003-node-identity/data-model.md`).
//!
//! These values are stored with postcard, which encodes an enum variant by its position: never
//! reorder or remove a variant of an enum used here (or of [`EntityKind`], [`Language`] and
//! [`HashFormat`]); append new variants at the end.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::GitOid;
use crate::syntax::{ByteRange, EntityKind, HashFormat, Language, StructuralHash};

/// The identity of a code entity: 16 bytes, displayed as a UUID.
///
/// New ids are UUID version 8, derived from the entity's content when it first appears (ADR-0007);
/// ids of other versions written by earlier formats are valid and opaque. Ordered by bytes, which
/// is the order "the lower id" of reconciliation refers to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AstNodeId(pub [u8; 16]);

impl fmt::Display for AstNodeId {
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

impl fmt::Debug for AstNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AstNodeId({self})")
    }
}

/// The version of the matching, derivation and delta rules that produced a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MatchFormat {
    /// The first rules (`specs/003-node-identity/research.md`, R3 to R8 and R12).
    V1,
}

impl MatchFormat {
    /// The format's number.
    pub fn number(self) -> u16 {
        match self {
            MatchFormat::V1 => 1,
        }
    }
}

/// One entity of a file version with its identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchoredNode {
    /// The entity's identity.
    pub id: AstNodeId,
    /// The neutral kind.
    pub kind: EntityKind,
    /// The entity's own name.
    pub name: String,
    /// The language's full name of the entity.
    pub qualified_path: String,
    /// Position among the entities of the file with the same kind and qualified path.
    pub occurrence: u32,
    /// Where it is in the blob; `None` for a node carried while its file has a syntax error.
    pub byte_range: Option<ByteRange>,
    /// Index of the enclosing node in [`BlobAstMap::nodes`]; `None` at file level.
    pub container: Option<u32>,
    /// Hash of the whole entity.
    pub structural_hash: StructuralHash,
    /// Hash of the signature.
    pub signature_hash: StructuralHash,
    /// Hash of the body.
    pub body_hash: StructuralHash,
    /// Whether the entity contains a syntax error.
    pub has_error: bool,
}

impl AnchoredNode {
    /// Whether the node is carried from an earlier version without a place in this one.
    pub fn is_detached(&self) -> bool {
        self.byte_range.is_none()
    }
}

/// The entities of one file version anchored to their identities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobAstMap {
    /// The blob the nodes were found in.
    pub source_blob: GitOid,
    /// The file's path in the repository.
    pub path: String,
    /// The file's compilation unit.
    pub unit: String,
    /// The file's language.
    pub language: Language,
    /// The rules that produced the hashes.
    pub hash_format: HashFormat,
    /// The rules that produced the identities.
    pub match_format: MatchFormat,
    /// The nodes in source order; detached nodes last.
    pub nodes: Vec<AnchoredNode>,
    /// Whether the file contains a syntax error anywhere.
    pub has_error: bool,
}

impl BlobAstMap {
    /// Checks the invariants: ids unique, containers before their children, detached nodes last.
    pub fn check(&self) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        let mut detached_seen = false;
        for (index, node) in self.nodes.iter().enumerate() {
            if !ids.insert(node.id) {
                return Err(format!("{}: duplicate id {}", self.path, node.id));
            }
            if node
                .container
                .is_some_and(|container| container as usize >= index)
            {
                return Err(format!("{}: node {index} has a container after it", self.path));
            }
            if node.is_detached() {
                detached_seen = true;
            } else if detached_seen {
                return Err(format!(
                    "{}: node {index} is placed after a detached node",
                    self.path
                ));
            }
        }
        Ok(())
    }
}

/// Where an entity is: its unit, file and qualified path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Location {
    /// The compilation unit.
    pub unit: String,
    /// The file's path.
    pub path: String,
    /// The qualified path.
    pub qualified_path: String,
}

/// One change of one entity between two versions.
///
/// A list of deltas is ordered by variant, in declaration order, then by id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SemanticDelta {
    /// A new entity.
    Created {
        /// Its new identity.
        id: AstNodeId,
        /// Its kind.
        kind: EntityKind,
        /// Where it is.
        at: Location,
    },
    /// The entity's file, unit or container changed.
    Moved {
        /// The identity.
        id: AstNodeId,
        /// Where it was.
        from: Location,
        /// Where it is.
        to: Location,
    },
    /// The entity's own name changed.
    Renamed {
        /// The identity.
        id: AstNodeId,
        /// The old name.
        from_name: String,
        /// The new name.
        to_name: String,
        /// The qualified path after the change.
        qualified_path: String,
    },
    /// The signature hash changed.
    SignatureMutated {
        /// The identity.
        id: AstNodeId,
        /// The old signature hash.
        before: StructuralHash,
        /// The new signature hash.
        after: StructuralHash,
    },
    /// The body hash changed.
    BodyModified {
        /// The identity.
        id: AstNodeId,
        /// The old body hash.
        before: StructuralHash,
        /// The new body hash.
        after: StructuralHash,
    },
    /// The entity no longer exists.
    Tombstoned {
        /// The identity.
        id: AstNodeId,
        /// Its kind.
        kind: EntityKind,
        /// Where it was last.
        at: Location,
    },
}

impl SemanticDelta {
    /// The identity the delta concerns.
    pub fn id(&self) -> AstNodeId {
        match self {
            SemanticDelta::Created { id, .. }
            | SemanticDelta::Moved { id, .. }
            | SemanticDelta::Renamed { id, .. }
            | SemanticDelta::SignatureMutated { id, .. }
            | SemanticDelta::BodyModified { id, .. }
            | SemanticDelta::Tombstoned { id, .. } => *id,
        }
    }

    /// The position of the variant in the order of a delta list.
    pub fn rank(&self) -> u8 {
        match self {
            SemanticDelta::Created { .. } => 0,
            SemanticDelta::Moved { .. } => 1,
            SemanticDelta::Renamed { .. } => 2,
            SemanticDelta::SignatureMutated { .. } => 3,
            SemanticDelta::BodyModified { .. } => 4,
            SemanticDelta::Tombstoned { .. } => 5,
        }
    }
}

/// Two identities found for one entity: `merged` resolves to `kept` from now on (ADR-0007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IdentityMerged {
    /// The lower id, kept.
    pub kept: AstNodeId,
    /// The higher id, now an alias of `kept`.
    pub merged: AstNodeId,
    /// The merge commit where the two ids met.
    pub commit: GitOid,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u8, container: Option<u32>, placed: bool) -> AnchoredNode {
        AnchoredNode {
            id: AstNodeId([id; 16]),
            kind: EntityKind::Function,
            name: format!("f{id}"),
            qualified_path: format!("crate::f{id}"),
            occurrence: 0,
            byte_range: placed.then_some(ByteRange::new(0, 1)),
            container,
            structural_hash: StructuralHash([0; 32]),
            signature_hash: StructuralHash([0; 32]),
            body_hash: StructuralHash([0; 32]),
            has_error: false,
        }
    }

    fn map(nodes: Vec<AnchoredNode>) -> BlobAstMap {
        BlobAstMap {
            source_blob: GitOid::from_bytes(&[1; 20]).expect("valid id"),
            path: "src/lib.rs".into(),
            unit: "src".into(),
            language: Language::Rust,
            hash_format: HashFormat::V1,
            match_format: MatchFormat::V1,
            nodes,
            has_error: false,
        }
    }

    #[test]
    fn ids_display_as_uuids() {
        let mut bytes = [0u8; 16];
        bytes
            .iter_mut()
            .enumerate()
            .for_each(|(index, byte)| *byte = index as u8);
        assert_eq!(
            AstNodeId(bytes).to_string(),
            "00010203-0405-0607-0809-0a0b0c0d0e0f"
        );
    }

    #[test]
    fn ids_order_by_bytes() {
        assert!(AstNodeId([1; 16]) < AstNodeId([2; 16]));
        let mut low_first = [0xff; 16];
        low_first[0] = 0;
        assert!(AstNodeId(low_first) < AstNodeId([1; 16]));
    }

    #[test]
    fn map_invariants_are_checked() {
        assert!(
            map(vec![
                node(1, None, true),
                node(2, Some(0), true),
                node(3, None, false)
            ])
            .check()
            .is_ok()
        );
        assert!(
            map(vec![node(1, None, true), node(1, None, true)])
                .check()
                .is_err()
        );
        assert!(map(vec![node(1, Some(0), true)]).check().is_err());
        assert!(
            map(vec![node(1, None, false), node(2, None, true)])
                .check()
                .is_err()
        );
    }

    #[test]
    fn maps_round_trip_through_postcard() {
        let original = map(vec![node(1, None, true), node(2, Some(0), true)]);
        let bytes = postcard::to_allocvec(&original).expect("encodes");
        let decoded: BlobAstMap = postcard::from_bytes(&bytes).expect("decodes");
        assert_eq!(decoded, original);
    }

    #[test]
    fn deltas_order_by_variant_then_id() {
        let at = Location {
            unit: "src".into(),
            path: "src/lib.rs".into(),
            qualified_path: "crate::f".into(),
        };
        let mut deltas = [
            SemanticDelta::Tombstoned {
                id: AstNodeId([0; 16]),
                kind: EntityKind::Function,
                at: at.clone(),
            },
            SemanticDelta::Created {
                id: AstNodeId([9; 16]),
                kind: EntityKind::Function,
                at: at.clone(),
            },
            SemanticDelta::Created {
                id: AstNodeId([1; 16]),
                kind: EntityKind::Function,
                at,
            },
        ];
        deltas.sort();
        let ranks: Vec<_> = deltas
            .iter()
            .map(|delta| (delta.rank(), delta.id().0[0]))
            .collect();
        assert_eq!(ranks, [(0, 1), (0, 9), (5, 0)]);
    }
}
