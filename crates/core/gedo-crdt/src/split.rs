//! The refactor version of a publish: the base with only the refactor lane applied (FEAT-067,
//! `specs/007-semantic-lanes/research.md` R5 and R8).
//!
//! Each changed file is diffed as units (tokens with the text before them, and whole blocks for the
//! entities the refactor lane moves or creates). Equal units and follow-up hunks take the final
//! text; every other hunk keeps the base text, so creations, removals, feature edits and formatting
//! stay as in the base. Blocks are dropped where the base had them and rebuilt where the final
//! version puts them, recursively, from their base and final texts.

use std::collections::BTreeSet;

use gedo_core::identity::{AstNodeId, SemanticDelta};

use crate::lanes::{Entities, FileVersions, Hunk, Lane, LanePlan, Region, diff_regions};

/// Why the refactor version could not be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    /// An entity the refactor lane moves or creates has no placed version where it is needed.
    #[error("entity {id} has no placed version in the {side} files")]
    MissingEntity {
        /// The entity.
        id: AstNodeId,
        /// `base` or `final`.
        side: &'static str,
    },
}

/// The refactor version of one changed file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefactorFile {
    /// The file's path.
    pub path: String,
    /// Its text; `None` when it is absent from the refactor version.
    pub text: Option<Vec<u8>>,
}

/// Builds the refactor version of every changed file.
pub fn build(files: &[FileVersions<'_>], plan: &LanePlan) -> Result<Vec<RefactorFile>, BuildError> {
    let entities = Entities::new(files);
    let blocks = plan.blocks();
    let builder = Builder {
        entities: &entities,
        blocks: &blocks,
        plan,
    };
    files
        .iter()
        .map(|file| {
            Ok(RefactorFile {
                path: file.path.clone(),
                text: builder.file(file)?,
            })
        })
        .collect()
}

struct Builder<'a, 'b> {
    entities: &'b Entities<'a>,
    blocks: &'b BTreeSet<AstNodeId>,
    plan: &'b LanePlan,
}

impl<'a> Builder<'a, '_> {
    fn is_block(&self) -> impl Fn(AstNodeId) -> bool + '_ {
        |id| self.blocks.contains(&id)
    }

    fn file(&self, file: &FileVersions<'a>) -> Result<Option<Vec<u8>>, BuildError> {
        let is_block = self.is_block();
        match (file.before, file.after) {
            (None, None) => Ok(None),
            (Some(before), Some(after)) => {
                let (before, after) = (
                    Region::of_file(before, &is_block),
                    Region::of_file(after, &is_block),
                );
                Ok(Some(self.merge(&before, &after)?.0))
            }
            (None, Some(after)) => {
                let empty = Region {
                    source: after.source,
                    units: Vec::new(),
                    end: 0,
                };
                let (text, blocks) = self.merge(&empty, &Region::of_file(after, &is_block))?;
                Ok((blocks > 0).then_some(text))
            }
            (Some(before), None) => {
                // Absent when every entity of the file moved out.
                let moved_out = before
                    .map
                    .nodes
                    .iter()
                    .filter(|node| node.container.is_none() && node.byte_range.is_some())
                    .all(|node| self.blocks.contains(&node.id));
                if moved_out {
                    return Ok(None);
                }
                let region = Region::of_file(before, &is_block);
                let empty = Region {
                    source: before.source,
                    units: Vec::new(),
                    end: 0,
                };
                Ok(Some(self.merge(&region, &empty)?.0))
            }
        }
    }

    /// The refactor text of two regions and the number of blocks it holds.
    fn merge(&self, before: &Region<'a>, after: &Region<'a>) -> Result<(Vec<u8>, usize), BuildError> {
        let mut text = Vec::new();
        let mut blocks = 0;
        // Whether the last text is not the base's, so the next unit continues the final layout.
        let mut after_follow_up = false;
        for hunk in diff_regions(before, after, &self.plan.follow_ups) {
            let Hunk {
                before: old,
                after: new,
                equal,
                follow_up,
            } = hunk;
            if equal {
                // The base's layout with the final tokens (renamed references included).
                for (old, new) in old.into_iter().zip(new) {
                    if std::mem::take(&mut after_follow_up) {
                        text.extend_from_slice(after.lead_text(new));
                    } else {
                        text.extend_from_slice(before.lead_text(old));
                    }
                    match after.units[new].block {
                        Some(id) => {
                            text.extend_from_slice(&self.entity(id)?);
                            blocks += 1;
                        }
                        None => {
                            let unit = &after.units[new];
                            text.extend_from_slice(&after.source[unit.start..unit.end]);
                        }
                    }
                }
                continue;
            }
            if follow_up {
                for unit in new {
                    blocks += self.emit_after(after, unit, &mut text)?;
                }
                after_follow_up = true;
                continue;
            }
            // A feature hunk keeps the base, without what moved away, and gains what moved in. When
            // its last text is not the base's (a block dropped or moved in), the next unit continues
            // the final layout.
            after_follow_up = false;
            for unit in old {
                if before.units[unit].block.is_none() {
                    text.extend_from_slice(before.text(unit));
                    after_follow_up = false;
                } else {
                    after_follow_up = true;
                }
            }
            for unit in new {
                if after.units[unit].block.is_some() {
                    blocks += self.emit_after(after, unit, &mut text)?;
                    after_follow_up = true;
                }
            }
        }
        let tail = if after.units.is_empty() && after.end == 0 {
            before.tail()
        } else {
            after.tail()
        };
        text.extend_from_slice(tail);
        Ok((text, blocks))
    }

    /// Emits a unit of the final region: a token as it is, a block rebuilt.
    fn emit_after(&self, after: &Region<'a>, unit: usize, text: &mut Vec<u8>) -> Result<usize, BuildError> {
        let Some(id) = after.units[unit].block else {
            text.extend_from_slice(after.text(unit));
            return Ok(0);
        };
        text.extend_from_slice(after.lead_text(unit));
        text.extend_from_slice(&self.entity(id)?);
        Ok(1)
    }

    /// The refactor text of an entity the refactor lane moves or creates.
    fn entity(&self, id: AstNodeId) -> Result<Vec<u8>, BuildError> {
        let is_block = self.is_block();
        let after = self
            .entities
            .after
            .get(&id)
            .and_then(|located| Region::of_entity(*located, id, &is_block))
            .ok_or(BuildError::MissingEntity { id, side: "final" })?;
        let created = self.plan.lane_of(id, crate::tree::Part::Presence) == Some(Lane::Refactor);
        if created {
            // Created by the refactor lane: its own text is final, its blocks rebuilt.
            let again = self
                .entities
                .after
                .get(&id)
                .and_then(|located| Region::of_entity(*located, id, &is_block))
                .ok_or(BuildError::MissingEntity { id, side: "final" })?;
            return Ok(self.merge(&again, &after)?.0);
        }
        let before = self
            .entities
            .before
            .get(&id)
            .and_then(|located| Region::of_entity(*located, id, &is_block))
            .ok_or(BuildError::MissingEntity { id, side: "base" })?;
        Ok(self.merge(&before, &after)?.0)
    }
}

/// The generated message of the refactor commit (research R8).
pub fn refactor_message(plan: &LanePlan) -> String {
    let mut renames = Vec::new();
    let mut moves = Vec::new();
    let mut modules = Vec::new();
    for change in plan.changes.iter().filter(|change| change.lane == Lane::Refactor) {
        match &change.delta {
            SemanticDelta::Renamed {
                from_name,
                to_name,
                qualified_path,
                ..
            } => {
                let (container, _) = crate::tree::split_qualified(qualified_path);
                let old = crate::tree::join_qualified(container, from_name);
                renames.push((old.clone(), format!("rename {old} to {to_name}")));
            }
            SemanticDelta::Moved { from, to, .. } => {
                let target = if from.path == to.path {
                    to.qualified_path.clone()
                } else {
                    to.path.clone()
                };
                moves.push((
                    from.qualified_path.clone(),
                    format!("move {} to {target}", from.qualified_path),
                ));
            }
            SemanticDelta::Created { at, .. } => {
                modules.push((
                    at.qualified_path.clone(),
                    format!("add module {}", at.qualified_path),
                ));
            }
            _ => {}
        }
    }
    renames.sort();
    moves.sort();
    modules.sort();
    let items: Vec<String> = renames
        .into_iter()
        .chain(moves)
        .chain(modules)
        .map(|(_, item)| item)
        .collect();
    let mut summary = items.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
    if items.len() > 3 {
        summary.push_str(&format!("; and {} more", items.len() - 3));
    }
    let mut message = format!("refactor: {summary}\n");
    if !items.is_empty() {
        message.push('\n');
        for item in &items {
            message.push_str(item);
            message.push('\n');
        }
    }
    message
}

#[cfg(test)]
mod tests {
    use gedo_core::identity::Location;

    use super::*;
    use crate::lanes::classify;

    fn rename(id: u8, from: &str, to: &str) -> SemanticDelta {
        SemanticDelta::Renamed {
            id: AstNodeId([id; 16]),
            from_name: from.into(),
            to_name: to.into(),
            qualified_path: format!("crate::{to}"),
        }
    }

    #[test]
    fn the_refactor_message_lists_renames_then_moves() {
        let mut deltas: Vec<SemanticDelta> = vec![rename(1, "b", "b2"), rename(2, "a", "a2")];
        deltas.push(SemanticDelta::Moved {
            id: AstNodeId([3; 16]),
            from: Location {
                unit: "src".into(),
                path: "src/lib.rs".into(),
                qualified_path: "crate::tax".into(),
            },
            to: Location {
                unit: "src".into(),
                path: "src/tax.rs".into(),
                qualified_path: "crate::tax::tax".into(),
            },
        });
        let plan = classify(&deltas, &[]);
        assert_eq!(
            refactor_message(&plan),
            "refactor: rename crate::a to a2; rename crate::b to b2; move crate::tax to src/tax.rs\n\n\
             rename crate::a to a2\nrename crate::b to b2\nmove crate::tax to src/tax.rs\n"
        );
        deltas.push(rename(4, "c", "c2"));
        let plan = classify(&deltas, &[]);
        assert!(refactor_message(&plan).starts_with(
            "refactor: rename crate::a to a2; rename crate::b to b2; rename crate::c to c2; and 1 more\n"
        ));
    }
}
