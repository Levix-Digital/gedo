//! Builds versions of a small code base from Rust sources and matches them.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use gedo_ast::{ChangeSet, EntityTokens, MatchOutcome, NewFile, OldFile, match_change};
use gedo_core::identity::{AstNodeId, BlobAstMap, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::ports::Parser;
use gedo_core::syntax::SyntaxMap;
use gedo_parse::TreeSitterParser;

/// One version of the code: each file's anchored map and syntax map.
#[derive(Clone, Debug, Default)]
pub struct Version {
    pub files: BTreeMap<String, (BlobAstMap, SyntaxMap)>,
}

impl Version {
    /// The id of the placed entity with this qualified path in `path`.
    pub fn id(&self, path: &str, qualified_path: &str) -> AstNodeId {
        self.try_id(path, qualified_path)
            .unwrap_or_else(|| panic!("{qualified_path} not found in {path}"))
    }

    pub fn try_id(&self, path: &str, qualified_path: &str) -> Option<AstNodeId> {
        self.files.get(path).and_then(|(map, _)| {
            map.nodes
                .iter()
                .find(|node| node.qualified_path == qualified_path && !node.is_detached())
                .map(|node| node.id)
        })
    }

    /// Every id of the version, detached nodes included.
    pub fn ids(&self) -> BTreeSet<AstNodeId> {
        self.files
            .values()
            .flat_map(|(map, _)| map.nodes.iter().map(|node| node.id))
            .collect()
    }
}

pub fn syntax(path: &str, source: &str) -> SyntaxMap {
    let parsed = TreeSitterParser::new()
        .parse(path, source.as_bytes())
        .expect("source parses");
    gedo_ast::normalize(&parsed, source.as_bytes()).expect("source normalizes")
}

pub fn blob(source: &str) -> GitOid {
    GitOid::from_bytes(&blake3::hash(source.as_bytes()).as_bytes()[..20]).expect("20 bytes")
}

/// The first version: every entity is created.
pub fn first(files: &[(&str, &str)]) -> (Version, MatchOutcome) {
    let changes: Vec<_> = files.iter().map(|&(path, source)| (path, Some(source))).collect();
    next(&Version::default(), &changes)
}

/// The next version: listed files change (`None` deletes), the others stay as they are.
pub fn next(previous: &Version, files: &[(&str, Option<&str>)]) -> (Version, MatchOutcome) {
    let listed: BTreeSet<&str> = files.iter().map(|&(path, _)| path).collect();
    let mut change = ChangeSet::default();
    for &(path, source) in files {
        if let Some((map, syntax_map)) = previous.files.get(path) {
            change.old.push(old_file(map, syntax_map));
        }
        if let Some(source) = source {
            change.new.push(NewFile {
                blob: blob(source),
                syntax: syntax(path, source),
            });
        }
    }
    let live: BTreeSet<AstNodeId> = previous
        .files
        .iter()
        .filter(|(path, _)| !listed.contains(path.as_str()))
        .flat_map(|(_, (map, _))| map.nodes.iter().map(|node| node.id))
        .collect();
    let outcome = match_change(&change, &live);
    let mut version = previous.clone();
    for &(path, _) in files {
        version.files.remove(path);
    }
    for (map, file) in outcome.maps.iter().zip(sorted_by_path(&change.new)) {
        assert_eq!(map.path, file.syntax.path);
        map.check().expect("map invariants hold");
        version
            .files
            .insert(map.path.clone(), (map.clone(), file.syntax.clone()));
    }
    (version, outcome)
}

fn sorted_by_path(files: &[NewFile]) -> Vec<&NewFile> {
    let mut sorted: Vec<_> = files.iter().collect();
    sorted.sort_by(|left, right| left.syntax.path.cmp(&right.syntax.path));
    sorted
}

/// The old side of a file: its map with the tokens of its placed nodes.
pub fn old_file(map: &BlobAstMap, syntax_map: &SyntaxMap) -> OldFile {
    let tokens = map
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            (!node.is_detached())
                .then(|| syntax_map.entities.get(index).map(EntityTokens::of))
                .flatten()
        })
        .collect();
    OldFile {
        map: map.clone(),
        tokens,
    }
}

/// The deltas of one id, as short names.
pub fn kinds_of(outcome: &MatchOutcome, id: AstNodeId) -> Vec<&'static str> {
    outcome
        .deltas
        .iter()
        .filter(|delta| delta.id() == id)
        .map(|delta| match delta {
            SemanticDelta::Created { .. } => "created",
            SemanticDelta::Moved { .. } => "moved",
            SemanticDelta::Renamed { .. } => "renamed",
            SemanticDelta::SignatureMutated { .. } => "signature",
            SemanticDelta::BodyModified { .. } => "body",
            SemanticDelta::Tombstoned { .. } => "tombstoned",
        })
        .collect()
}
