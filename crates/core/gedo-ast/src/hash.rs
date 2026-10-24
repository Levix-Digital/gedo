//! `HashFormat::V1` hashing of entities (research R6).
//!
//! An entity's signature and body are hashed separately, each over a stream of items: a
//! significant token (class, length, text), or a nested entity (its structural hash). Nested
//! entities whose order carries no meaning (imports, module declarations without a body) are
//! taken out of the stream and appended in sorted order. The structural hash combines both.

use gedo_core::syntax::{CodeEntity, EntityKind, NormalizedToken, ParsedFile, RawEntity, StructuralHash};

/// Key-derivation context of format V1. Never change it: every hash would change.
const CONTEXT_V1: &str = "gedo structural hash v1";

/// The first byte of each hashed stream, so the three hashes of an entity never collide.
const PART_STRUCTURAL: u8 = 0;
const PART_SIGNATURE: u8 = 1;
const PART_BODY: u8 = 2;

/// Item tags that cannot be confused with a token's class code (1 to 10).
const ITEM_NESTED: u8 = 0xF0;
const ITEM_UNORDERED: u8 = 0xF1;

/// The entities of `file` with their tokens and hashes, without imports, containers reindexed.
pub fn hash_entities(file: &ParsedFile, tokens: &[Option<NormalizedToken>]) -> Vec<CodeEntity> {
    let children = direct_children(file);
    let mut hashed: Vec<Option<HashedEntity>> = vec![None; file.entities.len()];
    // Containers precede their children, so walking backwards hashes every child first.
    for index in (0..file.entities.len()).rev() {
        hashed[index] = Some(hash_entity(file, index, &children[index], tokens, &hashed));
    }
    collect(file, hashed)
}

#[derive(Clone)]
struct HashedEntity {
    signature_tokens: Vec<NormalizedToken>,
    body_tokens: Vec<NormalizedToken>,
    structural: StructuralHash,
    signature: StructuralHash,
    body: StructuralHash,
}

fn direct_children(file: &ParsedFile) -> Vec<Vec<usize>> {
    let mut children = vec![Vec::new(); file.entities.len()];
    for (index, entity) in file.entities.iter().enumerate() {
        if let Some(container) = entity.container {
            children[container as usize].push(index);
        }
    }
    children
}

/// Whether the entity's position among its siblings carries no meaning.
fn is_order_insensitive(entity: &RawEntity) -> bool {
    entity.kind == EntityKind::Import || (entity.kind == EntityKind::Module && entity.body_start.is_none())
}

struct Part {
    hasher: blake3::Hasher,
    tokens: Vec<NormalizedToken>,
    unordered: Vec<StructuralHash>,
}

impl Part {
    fn new(tag: u8) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(CONTEXT_V1);
        hasher.update(&[tag]);
        Self {
            hasher,
            tokens: Vec::new(),
            unordered: Vec::new(),
        }
    }

    fn push_token(&mut self, token: &NormalizedToken) {
        let text = token.text.as_bytes();
        // Source files are limited far below 4 GiB, so every length fits.
        let length = u32::try_from(text.len()).unwrap_or(u32::MAX);
        self.hasher.update(&[token.class.code()]);
        self.hasher.update(&length.to_le_bytes());
        self.hasher.update(text);
        self.tokens.push(token.clone());
    }

    fn push_nested(&mut self, hash: StructuralHash) {
        self.hasher.update(&[ITEM_NESTED]);
        self.hasher.update(&hash.0);
    }

    fn finish(mut self) -> (StructuralHash, Vec<NormalizedToken>) {
        self.unordered.sort();
        for hash in &self.unordered {
            self.hasher.update(&[ITEM_UNORDERED]);
            self.hasher.update(&hash.0);
        }
        (StructuralHash(*self.hasher.finalize().as_bytes()), self.tokens)
    }
}

fn hash_entity(
    file: &ParsedFile,
    index: usize,
    children: &[usize],
    tokens: &[Option<NormalizedToken>],
    hashed: &[Option<HashedEntity>],
) -> HashedEntity {
    let entity = &file.entities[index];
    let body_start = entity.body_start.unwrap_or(entity.tokens.end);
    let mut signature = Part::new(PART_SIGNATURE);
    let mut body = Part::new(PART_BODY);
    let mut next_child = children.iter().peekable();
    let mut position = entity.tokens.start;
    while position < entity.tokens.end {
        let part = if position < body_start {
            &mut signature
        } else {
            &mut body
        };
        if let Some(&&child) = next_child.peek()
            && file.entities[child].tokens.start == position
        {
            next_child.next();
            let child_entity = &file.entities[child];
            let child_hash = hashed[child]
                .as_ref()
                .expect("children come after their container and are hashed first")
                .structural;
            if is_order_insensitive(child_entity) {
                part.unordered.push(child_hash);
            } else {
                part.push_nested(child_hash);
            }
            position = child_entity.tokens.end;
            continue;
        }
        if let Some(token) = &tokens[position as usize] {
            part.push_token(token);
        }
        position += 1;
    }
    let (signature_hash, signature_tokens) = signature.finish();
    let (body_hash, body_tokens) = body.finish();
    let mut structural = blake3::Hasher::new_derive_key(CONTEXT_V1);
    structural.update(&[PART_STRUCTURAL]);
    structural.update(&signature_hash.0);
    structural.update(&body_hash.0);
    HashedEntity {
        signature_tokens,
        body_tokens,
        structural: StructuralHash(*structural.finalize().as_bytes()),
        signature: signature_hash,
        body: body_hash,
    }
}

/// Builds the public entities, dropping imports and renumbering containers.
fn collect(file: &ParsedFile, hashed: Vec<Option<HashedEntity>>) -> Vec<CodeEntity> {
    let mut new_index = vec![None; file.entities.len()];
    let mut entities = Vec::with_capacity(file.entities.len());
    for (index, (entity, hashes)) in file.entities.iter().zip(hashed).enumerate() {
        let Some(hashes) = hashes.filter(|_| entity.kind != EntityKind::Import) else {
            continue;
        };
        new_index[index] = Some(entities.len() as u32);
        entities.push(CodeEntity {
            kind: entity.kind,
            language_kind: entity.language_kind.clone(),
            name: entity.name.clone(),
            qualified_path: entity.qualified_path.clone(),
            occurrence: entity.occurrence,
            byte_range: entity.byte_range,
            // Containers precede their children, so the container's new index is known; an
            // import contains only imports.
            container: entity
                .container
                .and_then(|container| new_index[container as usize]),
            has_error: entity.has_error,
            signature_tokens: hashes.signature_tokens,
            body_tokens: hashes.body_tokens,
            structural_hash: hashes.structural,
            signature_hash: hashes.signature,
            body_hash: hashes.body,
        });
    }
    entities
}
