//! Deterministic derivation of new identities (research R7, ADR-0007).
//!
//! A new entity's id is a pure function of its kind, unit, qualified path, occurrence and
//! structural hash, so every machine that sees the same entity appear derives the same id.

use std::collections::BTreeSet;

use gedo_core::identity::AstNodeId;
use gedo_core::syntax::{EntityKind, StructuralHash};

/// Key-derivation context of `MatchFormat::V1`. Never change it: every new id would change.
const CONTEXT_V1: &str = "gedo node id v1";

/// What a new id is derived from.
#[derive(Clone, Copy, Debug)]
pub struct IdentitySeed<'a> {
    /// The entity's kind.
    pub kind: EntityKind,
    /// Its compilation unit.
    pub unit: &'a str,
    /// Its qualified path.
    pub qualified_path: &'a str,
    /// Its occurrence among equal kinds and paths of its file.
    pub occurrence: u32,
    /// Its structural hash.
    pub structural_hash: StructuralHash,
}

/// Derives the id of a new entity.
///
/// `taken` holds the ids already live in the same version of the code; when the derived id is one
/// of them (an entity renamed away and another created with its old name and content), the next
/// collision counter is used, so the result stays deterministic and unique.
pub fn derive_id(seed: &IdentitySeed<'_>, taken: &BTreeSet<AstNodeId>) -> AstNodeId {
    (0u32..)
        .map(|counter| derive_with_counter(seed, counter))
        .find(|id| !taken.contains(id))
        .unwrap_or_else(|| derive_with_counter(seed, u32::MAX))
}

fn derive_with_counter(seed: &IdentitySeed<'_>, counter: u32) -> AstNodeId {
    let mut hasher = blake3::Hasher::new_derive_key(CONTEXT_V1);
    hasher.update(&[kind_code(seed.kind)]);
    update_text(&mut hasher, seed.unit);
    update_text(&mut hasher, seed.qualified_path);
    hasher.update(&seed.occurrence.to_le_bytes());
    hasher.update(&seed.structural_hash.0);
    hasher.update(&counter.to_le_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    // RFC 9562: version 8 in the high nibble of byte 6, variant `10` in the top bits of byte 8.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    AstNodeId(bytes)
}

fn update_text(hasher: &mut blake3::Hasher, text: &str) {
    // Paths are far below 4 GiB, so every length fits.
    let length = u32::try_from(text.len()).unwrap_or(u32::MAX);
    hasher.update(&length.to_le_bytes());
    hasher.update(text.as_bytes());
}

/// One byte per kind. Never change a value: every derived id would change.
fn kind_code(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Module => 1,
        EntityKind::Function => 2,
        EntityKind::Method => 3,
        EntityKind::Struct => 4,
        EntityKind::Enum => 5,
        EntityKind::Union => 6,
        EntityKind::Trait => 7,
        EntityKind::Impl => 8,
        EntityKind::Constant => 9,
        EntityKind::Static => 10,
        EntityKind::TypeAlias => 11,
        EntityKind::AssociatedConstant => 12,
        EntityKind::AssociatedType => 13,
        EntityKind::Macro => 14,
        EntityKind::Import => 15,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(qualified_path: &str) -> IdentitySeed<'_> {
        IdentitySeed {
            kind: EntityKind::Function,
            unit: "src",
            qualified_path,
            occurrence: 0,
            structural_hash: StructuralHash([7; 32]),
        }
    }

    #[test]
    fn ids_are_version_8_with_the_rfc_variant() {
        let id = derive_id(&seed("crate::f"), &BTreeSet::new());
        assert_eq!(id.0[6] >> 4, 8);
        assert_eq!(id.0[8] >> 6, 0b10);
    }

    #[test]
    fn the_same_seed_gives_the_same_id_and_any_change_another() {
        let none = BTreeSet::new();
        let base = derive_id(&seed("crate::f"), &none);
        assert_eq!(derive_id(&seed("crate::f"), &none), base);
        assert_ne!(derive_id(&seed("crate::g"), &none), base);
        let mut other = seed("crate::f");
        other.unit = "crates/x/src";
        assert_ne!(derive_id(&other, &none), base);
        other = seed("crate::f");
        other.occurrence = 1;
        assert_ne!(derive_id(&other, &none), base);
        other = seed("crate::f");
        other.kind = EntityKind::Method;
        assert_ne!(derive_id(&other, &none), base);
        other = seed("crate::f");
        other.structural_hash = StructuralHash([8; 32]);
        assert_ne!(derive_id(&other, &none), base);
    }

    #[test]
    fn a_taken_id_moves_to_the_next_counter() {
        let first = derive_id(&seed("crate::f"), &BTreeSet::new());
        let taken = BTreeSet::from([first]);
        let second = derive_id(&seed("crate::f"), &taken);
        assert_ne!(second, first);
        assert_eq!(derive_id(&seed("crate::f"), &taken), second);
    }

    #[test]
    fn the_derivation_is_pinned() {
        // Changing this value means every published id changes: a new MatchFormat is needed.
        assert_eq!(
            derive_id(&seed("crate::f"), &BTreeSet::new()).to_string(),
            PINNED_ID
        );
    }

    const PINNED_ID: &str = "56efab55-a621-87bb-806f-1c250cc8a2d3";
}
