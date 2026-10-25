//! The records the index keeps in the symbol cache, namespace `identity`
//! (`specs/003-node-identity/data-model.md`).
//!
//! Every value is a postcard payload in a [`RecordEnvelope`] of format 1. The cache is local, but a
//! damaged or foreign file must never crash Gedo: decoding checks sizes and formats and never
//! panics.

use gedo_core::StoreError;
use gedo_core::identity::{BlobAstMap, IdentityMerged, SemanticDelta};
use gedo_core::ids::GitOid;
use gedo_core::record::RecordEnvelope;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The cache namespace of the index.
pub const NAMESPACE: &str = "identity";

/// Format of every record of this module.
pub const FORMAT: u16 = 1;

/// A checkpoint is written every this many first-parent commits.
pub const CHECKPOINT_INTERVAL: u64 = 64;

/// BLAKE3 of an encoded map: the key a map is stored under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MapDigest(pub [u8; 32]);

/// What indexing one commit found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitRecord {
    /// The commit's parents, first parent first.
    pub parents: Vec<GitOid>,
    /// Distance to a root along first parents (a root is 0).
    pub depth: u64,
    /// The Rust files that differ from the first parent, sorted by path; `None` is a deletion.
    pub files: Vec<(String, Option<MapDigest>)>,
    /// The deltas against the first parent.
    pub deltas: Vec<SemanticDelta>,
    /// Identity merges found at this commit (a merge commit).
    pub merges: Vec<IdentityMerged>,
}

/// Every Rust file of a commit with the digest of its map.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Sorted by path.
    pub files: Vec<(String, MapDigest)>,
}

/// Every identity merge recorded by the index.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeList {
    /// In the order they were recorded.
    pub merges: Vec<IdentityMerged>,
}

/// Key of the record of `commit`.
pub fn commit_key(commit: &GitOid) -> Vec<u8> {
    format!("commit/{commit}").into_bytes()
}

/// Key of the checkpoint of `commit`.
pub fn checkpoint_key(commit: &GitOid) -> Vec<u8> {
    format!("checkpoint/{commit}").into_bytes()
}

/// Key of a map.
pub fn map_key(digest: &MapDigest) -> Vec<u8> {
    let hex: String = digest.0.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("map/{hex}").into_bytes()
}

/// Key of the merge list.
pub const MERGES_KEY: &[u8] = b"merges";

/// Encodes a record.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    RecordEnvelope::wrap(FORMAT, value)?.encode()
}

/// Decodes a record; `source` names it in errors.
pub fn decode<T: DeserializeOwned>(bytes: &[u8], source: &str) -> Result<T, StoreError> {
    RecordEnvelope::decode(bytes, &[FORMAT], source)?.unwrap(source)
}

/// Decodes a map and checks its invariants, so a damaged record never reaches matching.
pub fn decode_map(bytes: &[u8], source: &str) -> Result<BlobAstMap, StoreError> {
    let map: BlobAstMap = decode(bytes, source)?;
    map.check().map_err(|reason| StoreError::Corrupted {
        path: source.to_owned(),
        offset: None,
        reason,
    })?;
    Ok(map)
}

/// A map's encoding and digest.
pub fn encode_map(map: &BlobAstMap) -> Result<(MapDigest, Vec<u8>), StoreError> {
    let bytes = encode(map)?;
    Ok((MapDigest(*blake3::hash(&bytes).as_bytes()), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn records_round_trip() {
        let commit = GitOid::from_bytes(&[3; 20]).expect("valid id");
        let record = CommitRecord {
            parents: vec![commit],
            depth: 7,
            files: vec![
                ("src/lib.rs".into(), Some(MapDigest([1; 32]))),
                ("src/old.rs".into(), None),
            ],
            deltas: Vec::new(),
            merges: Vec::new(),
        };
        let bytes = encode(&record).expect("encodes");
        assert_eq!(decode::<CommitRecord>(&bytes, "test").expect("decodes"), record);
        assert_eq!(
            String::from_utf8(commit_key(&commit)).expect("text"),
            format!("commit/{}", "03".repeat(20))
        );
    }

    #[test]
    fn unknown_formats_are_refused() {
        let bytes = RecordEnvelope::wrap(9, &Checkpoint::default())
            .and_then(|envelope| envelope.encode())
            .expect("encodes");
        assert!(matches!(
            decode::<Checkpoint>(&bytes, "test"),
            Err(StoreError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn maps_that_break_their_invariants_are_refused() {
        use gedo_core::identity::{AnchoredNode, AstNodeId, MatchFormat};
        use gedo_core::syntax::{EntityKind, HashFormat, Language, StructuralHash};
        let node = AnchoredNode {
            id: AstNodeId([1; 16]),
            kind: EntityKind::Function,
            name: "f".into(),
            qualified_path: "crate::f".into(),
            occurrence: 0,
            byte_range: None,
            container: None,
            structural_hash: StructuralHash([0; 32]),
            signature_hash: StructuralHash([0; 32]),
            body_hash: StructuralHash([0; 32]),
            has_error: false,
        };
        let map = BlobAstMap {
            source_blob: GitOid::from_bytes(&[2; 20]).expect("valid id"),
            path: "src/lib.rs".into(),
            unit: String::new(),
            language: Language::Rust,
            hash_format: HashFormat::V1,
            match_format: MatchFormat::V1,
            nodes: vec![node.clone()],
            has_error: false,
        };
        let (_, bytes) = encode_map(&map).expect("encodes");
        assert_eq!(decode_map(&bytes, "test").expect("valid"), map);
        let duplicated = BlobAstMap {
            nodes: vec![node.clone(), node],
            ..map
        };
        let (_, bytes) = encode_map(&duplicated).expect("encodes");
        assert!(matches!(
            decode_map(&bytes, "test"),
            Err(StoreError::Corrupted { .. })
        ));
    }

    proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _record = decode::<CommitRecord>(&bytes, "fuzz");
            let _checkpoint = decode::<Checkpoint>(&bytes, "fuzz");
            let _map = decode_map(&bytes, "fuzz");
        }
    }
}
