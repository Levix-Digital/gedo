//! One iterative pass over a syntax tree that collects tokens and entities (contract guarantees
//! 3 to 6 of `contracts/parser-port.md`). The cursor walk never recurses, so deep trees cannot
//! overflow the stack.

use std::collections::{HashMap, HashSet};

use gedo_core::ports::ParseError;
use gedo_core::syntax::{ByteRange, EntityKind, RawEntity, Token, TokenRange};
use tree_sitter::{Node, Tree};

use crate::MAX_NESTING_DEPTH;
use crate::rust::{kinds, paths, tokens};

/// An entity whose node is still being walked.
struct OpenEntity {
    index: usize,
    node_id: usize,
    body_id: Option<usize>,
    /// Qualified path under which nested entities are named.
    scope: String,
}

/// Attributes and comments seen just before a possible entity, which belong to it.
#[derive(Clone, Copy)]
struct Prefix {
    parent_id: Option<usize>,
    token: u32,
    byte: u32,
}

struct Collector<'source> {
    code: &'source [u8],
    offset: u32,
    tokens: Vec<Token>,
    entities: Vec<RawEntity>,
    open: Vec<OpenEntity>,
    root_scope: String,
    prefix: Option<Prefix>,
    optional_braces: HashSet<usize>,
    occurrences: HashMap<(EntityKind, String), u32>,
}

/// Collects the tokens and entities of `tree`, parsed from `code`; byte ranges are shifted by
/// `offset` (a byte order mark before `code`).
pub fn collect(
    tree: &Tree,
    code: &[u8],
    offset: u32,
    path: &str,
) -> Result<(Vec<Token>, Vec<RawEntity>), ParseError> {
    let mut collector = Collector {
        code,
        offset,
        tokens: Vec::new(),
        entities: Vec::new(),
        open: Vec::new(),
        root_scope: paths::module_path(path),
        prefix: None,
        optional_braces: HashSet::new(),
        occurrences: HashMap::new(),
    };
    let mut cursor = tree.walk();
    // The ancestors of the cursor's node: asking Tree-sitter for a node's parent walks down from
    // the root each time, which made the walk quadratic in depth.
    let mut ancestors: Vec<Node<'_>> = Vec::new();
    loop {
        if ancestors.len() > MAX_NESTING_DEPTH as usize {
            return Err(ParseError::TooDeep {
                path: path.to_owned(),
                limit: MAX_NESTING_DEPTH,
            });
        }
        let node = cursor.node();
        let is_leaf = collector.enter(node, ancestors.last().copied());
        if !is_leaf && cursor.goto_first_child() {
            ancestors.push(node);
            continue;
        }
        loop {
            collector.leave(cursor.node());
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Ok((collector.tokens, collector.entities));
            }
            ancestors.pop();
        }
    }
}

impl Collector<'_> {
    fn shifted(&self, byte: usize) -> u32 {
        byte as u32 + self.offset
    }

    /// Visits a node on the way down; returns whether its children are skipped.
    fn enter(&mut self, node: Node<'_>, parent: Option<Node<'_>>) -> bool {
        self.track_prefix(node, parent);
        if let Some(top) = self.open.last()
            && top.body_id == Some(node.id())
        {
            let index = top.index;
            self.entities[index].body_start = Some(self.tokens.len() as u32);
        }
        let enclosing = self.open.last().map(|top| self.entities[top.index].kind);
        if let Some(kind) = kinds::entity_kind(node.kind(), enclosing) {
            self.open_entity(node, parent, kind);
        }
        if kinds::is_optional_block(node, parent) {
            self.optional_braces.insert(node.id());
        }
        let is_leaf = node.child_count() == 0 || kinds::is_atomic(node.kind());
        if is_leaf {
            self.push_token(node, parent);
        }
        is_leaf
    }

    fn leave(&mut self, node: Node<'_>) {
        if self.open.last().is_some_and(|top| top.node_id == node.id())
            && let Some(top) = self.open.pop()
        {
            let token_end = self.tokens.len() as u32;
            let byte_end = self.shifted(node.end_byte());
            let entity = &mut self.entities[top.index];
            entity.tokens.end = token_end;
            entity.byte_range.end = byte_end;
        }
    }

    /// Outer attributes and comments directly before an item are part of it: they start a prefix
    /// that the next sibling takes if it is an entity and that any other sibling discards. Nodes
    /// below the prefix's level (the attribute's own children) leave it alone.
    fn track_prefix(&mut self, node: Node<'_>, parent: Option<Node<'_>>) {
        let parent_id = parent.map(|parent| parent.id());
        let is_attachable = matches!(node.kind(), "attribute_item" | "line_comment" | "block_comment");
        let same_level = self.prefix.is_some_and(|prefix| prefix.parent_id == parent_id);
        if is_attachable {
            if !same_level {
                self.prefix = Some(Prefix {
                    parent_id,
                    token: self.tokens.len() as u32,
                    byte: self.shifted(node.start_byte()),
                });
            }
        } else if same_level && kinds::entity_kind(node.kind(), None).is_none() {
            self.prefix = None;
        }
    }

    fn open_entity(&mut self, node: Node<'_>, parent: Option<Node<'_>>, kind: EntityKind) {
        let parent_id = parent.map(|parent| parent.id());
        let prefix = self.prefix.take().filter(|prefix| prefix.parent_id == parent_id);
        let scope = self
            .open
            .last()
            .map_or(self.root_scope.as_str(), |top| top.scope.as_str())
            .to_owned();
        let name = node
            .child_by_field_name("name")
            .map(|name| String::from_utf8_lossy(&self.code[name.byte_range()]).into_owned())
            .unwrap_or_default();
        let (qualified_path, child_scope) = match kind {
            EntityKind::Impl => paths::impl_paths(node, self.code, &scope),
            EntityKind::Import => (scope.clone(), scope.clone()),
            _ => {
                let path = format!("{scope}::{name}");
                (path.clone(), path)
            }
        };
        let byte_range = ByteRange::new(
            prefix.map_or_else(|| self.shifted(node.start_byte()), |prefix| prefix.byte),
            self.shifted(node.end_byte()),
        );
        let occurrence = self
            .occurrences
            .entry((kind, qualified_path.clone()))
            .or_insert(0);
        let entity = RawEntity {
            kind,
            language_kind: node.kind().to_owned(),
            name,
            qualified_path,
            occurrence: *occurrence,
            byte_range,
            container: self.open.last().map(|top| top.index as u32),
            tokens: TokenRange {
                start: prefix.map_or(self.tokens.len() as u32, |prefix| prefix.token),
                end: self.tokens.len() as u32,
            },
            body_start: None,
            has_error: node.has_error(),
        };
        *occurrence += 1;
        self.open.push(OpenEntity {
            index: self.entities.len(),
            node_id: node.id(),
            body_id: kinds::body_node(node).map(|body| body.id()),
            scope: child_scope,
        });
        self.entities.push(entity);
    }

    fn push_token(&mut self, node: Node<'_>, parent: Option<Node<'_>>) {
        let range = node.byte_range();
        if range.is_empty() {
            // A missing node inserted by error recovery: its error is already on the entities.
            return;
        }
        let optional_brace = parent.is_some_and(|parent| self.optional_braces.contains(&parent.id()));
        let class = tokens::classify(node, parent, &self.code[range.clone()], optional_brace);
        self.tokens.push(Token {
            class,
            byte_range: ByteRange::new(self.shifted(range.start), self.shifted(range.end)),
        });
    }
}
