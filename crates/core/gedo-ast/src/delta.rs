//! The deltas of an entity that continues an earlier one (research R12).
//!
//! `Moved` when its unit, file or container (by identity, so a renamed container moves nothing)
//! changed, `Renamed` when its own name changed,
//! `SignatureMutated` when its signature changed other than by the name, `BodyModified` when its
//! own body tokens changed (a change inside a nested entity is that entity's delta).

use gedo_core::identity::{AstNodeId, SemanticDelta};
use gedo_core::syntax::NormalizedToken;

use crate::matching::{NewEntity, OldEntity, without_name};

/// The deltas of `new`, which continues `old`, in variant order; `new_container` is the id of
/// `new`'s container.
pub(crate) fn continued(
    old: &OldEntity<'_>,
    new: &NewEntity<'_>,
    new_container: Option<AstNodeId>,
) -> Vec<SemanticDelta> {
    let (node, entity) = (old.node, new.entity);
    let id = node.id;
    let mut deltas = Vec::new();
    let moved = old.unit != new.unit || old.path != new.path || old.container_id != new_container;
    if moved {
        deltas.push(SemanticDelta::Moved {
            id,
            from: old.location(),
            to: new.location(),
        });
    }
    let renamed = node.name != entity.name;
    if renamed {
        deltas.push(SemanticDelta::Renamed {
            id,
            from_name: node.name.clone(),
            to_name: entity.name.clone(),
            qualified_path: entity.qualified_path.clone(),
        });
    }
    let signature_changed = match old.tokens {
        Some(tokens) if renamed => !same_ignoring(
            &tokens.signature,
            &node.name,
            &entity.signature_tokens,
            &entity.name,
        ),
        Some(tokens) => tokens.signature != entity.signature_tokens,
        // Without the old tokens, only the hash is known; a rename alone changes it too.
        None => !renamed && node.signature_hash != entity.signature_hash,
    };
    if signature_changed {
        deltas.push(SemanticDelta::SignatureMutated {
            id,
            before: node.signature_hash,
            after: entity.signature_hash,
        });
    }
    let body_changed = match old.tokens {
        Some(tokens) => tokens.body != entity.body_tokens,
        None => node.body_hash != entity.body_hash,
    };
    if body_changed {
        deltas.push(SemanticDelta::BodyModified {
            id,
            before: node.body_hash,
            after: entity.body_hash,
        });
    }
    deltas
}

/// Whether two token lists are equal once the first occurrence of each side's name is left out.
fn same_ignoring(
    left: &[NormalizedToken],
    left_name: &str,
    right: &[NormalizedToken],
    right_name: &str,
) -> bool {
    without_name(left, left_name) == without_name(right, right_name)
}
