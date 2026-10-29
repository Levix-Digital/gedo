//! The events of a commit made without Gedo (research R10).

use gedo_core::event::{Author, AuthorKind, Clock, Event, Labels, Origin, SemanticOp};
use gedo_core::identity::{AstNodeId, IdentityMerged, SemanticDelta};
use gedo_core::syntax::StructuralHash;

use crate::author::fingerprint;

/// An entity whose structural hash after the change is unknown, so its event cannot be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("no structural hash for entity {id} after the change")]
pub struct ReconstructError {
    /// The entity.
    pub id: AstNodeId,
}

/// The events of a commit of `commit_generation` authored by `author_email`: its deltas against
/// the first parent in their order, then its identity merges, with sequences counted from 0.
///
/// Ids are used as recorded at the commit, never resolved through later aliases, so the result
/// depends only on the commit. `result_hash` gives an entity's structural hash in the commit.
pub fn reconstructed_events(
    commit_generation: u64,
    author_email: &str,
    deltas: &[SemanticDelta],
    merges: &[IdentityMerged],
    result_hash: impl Fn(AstNodeId) -> Option<StructuralHash>,
) -> Result<Vec<Event>, ReconstructError> {
    let author = Author {
        fingerprint: fingerprint(author_email),
        kind: AuthorKind::Person,
    };
    let operations = deltas
        .iter()
        .cloned()
        .map(SemanticOp::Delta)
        .chain(merges.iter().map(|merge| SemanticOp::IdentityMerged {
            kept: merge.kept,
            merged: merge.merged,
        }));
    operations
        .enumerate()
        .map(|(index, operation)| {
            let target = operation.target();
            let result_hash = if operation.has_no_result() {
                None
            } else {
                Some(result_hash(target).ok_or(ReconstructError { id: target })?)
            };
            Ok(Event {
                clock: Clock {
                    generation: commit_generation,
                    sequence: u32::try_from(index).unwrap_or(u32::MAX),
                },
                target,
                operation,
                result_hash,
                author: author.clone(),
                labels: Labels::new(),
                origin: Origin::Reconstructed,
            })
        })
        .collect()
}
