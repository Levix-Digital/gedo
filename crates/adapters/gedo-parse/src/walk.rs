//! One iterative pass over a syntax tree that collects tokens and entities (contract guarantees
//! 3 to 6 of `contracts/parser-port.md`). The cursor walk never recurses, so deep trees cannot
//! overflow the stack.

use std::collections::{HashMap, HashSet};

use gedo_core::ports::ParseError;
use gedo_core::syntax::{ByteRange, EntityKind, RawEntity, Token, TokenClass, TokenRange};
use tree_sitter::{Node, Tree};

use crate::rust::{kinds, paths, tokens};
use crate::{Deadline, MAX_NESTING_DEPTH};

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
    /// Whether one of the attached nodes contains a syntax error.
    has_error: bool,
}

/// The `;` of a `return`, `break` or `continue` statement, optional only if no other code follows
/// the statement in its block (research R5).
#[derive(Clone, Copy)]
struct PendingJump {
    block_id: usize,
    token: usize,
}

/// How many nodes the walk visits between two looks at the clock.
const NODES_PER_DEADLINE_CHECK: usize = 4_096;

struct Collector<'source> {
    code: &'source [u8],
    offset: u32,
    tokens: Vec<Token>,
    entities: Vec<RawEntity>,
    open: Vec<OpenEntity>,
    root_scope: String,
    prefix: Option<Prefix>,
    optional_braces: HashSet<usize>,
    /// Lists with a single element (`kinds::single_element`), by node id.
    single_elements: HashMap<usize, kinds::SingleElement>,
    occurrences: HashMap<(EntityKind, String), u32>,
    /// The node of the last token pushed, with the token's index.
    last_leaf: Option<(usize, usize)>,
    pending_jump: Option<PendingJump>,
}

/// Collects the tokens and entities of `tree`, parsed from `code`; byte ranges are shifted by
/// `offset` (a byte order mark before `code`). Fails with `Timeout` once `deadline` has passed.
pub fn collect(
    tree: &Tree,
    code: &[u8],
    offset: u32,
    path: &str,
    deadline: Deadline,
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
        single_elements: HashMap::new(),
        occurrences: HashMap::new(),
        last_leaf: None,
        pending_jump: None,
    };
    let mut cursor = tree.walk();
    // The ancestors of the cursor's node: asking Tree-sitter for a node's parent walks down from
    // the root each time, which made the walk quadratic in depth.
    let mut ancestors: Vec<Node<'_>> = Vec::new();
    let mut visited = 0usize;
    loop {
        if visited.is_multiple_of(NODES_PER_DEADLINE_CHECK) && deadline.has_passed() {
            return Err(deadline.error(path));
        }
        visited += 1;
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
            let node = cursor.node();
            collector.leave(node, ancestors.last().copied());
            if cursor.goto_next_sibling() {
                break;
            }
            // Asking Tree-sitter for a node's next sibling walks the parent's children from the
            // first one; the cursor tells it here for free.
            collector.leave_last_child(node);
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
        if let Some(pending) = self.pending_jump
            && parent.is_some_and(|parent| parent.id() == pending.block_id)
            && !matches!(node.kind(), "line_comment" | "block_comment" | "}")
        {
            // Code follows the jump statement: its `;` stays (research R5).
            self.pending_jump = None;
        }
        if let Some(top) = self.open.last()
            && top.body_id == Some(node.id())
        {
            let index = top.index;
            self.entities[index].body_start = Some(self.tokens.len() as u32);
        }
        if let Some(single) = kinds::single_element(node) {
            self.single_elements.insert(node.id(), single);
        }
        let enclosing = self.open.last().map(|top| self.entities[top.index].kind);
        if let Some(kind) = kinds::entity_kind(node.kind(), enclosing)
            .or_else(|| self.is_import_item(node, parent).then_some(EntityKind::Import))
        {
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

    fn leave(&mut self, node: Node<'_>, parent: Option<Node<'_>>) {
        if let Some(pending) = self.pending_jump
            && pending.block_id == node.id()
        {
            // Nothing but comments followed the jump statement in its block.
            self.tokens[pending.token].class = TokenClass::OptionalPunctuation;
            self.pending_jump = None;
        }
        if kinds::is_jump_statement(node)
            && let (Some(parent), Some((leaf_id, token))) = (parent, self.last_leaf)
            && node
                .child(node.child_count().saturating_sub(1))
                .is_some_and(|last| last.id() == leaf_id)
            && &self.code[node.end_byte().saturating_sub(1)..node.end_byte()] == b";"
        {
            self.pending_jump = Some(PendingJump {
                block_id: parent.id(),
                token,
            });
        }
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

    /// Whether `node` is an item of a `use` list with several items, an import of its own.
    fn is_import_item(&self, node: Node<'_>, parent: Option<Node<'_>>) -> bool {
        kinds::is_use_list_item(node, parent)
            && parent.is_some_and(|parent| !self.single_elements.contains_key(&parent.id()))
    }

    /// Called after `leave` when `node` is the last child of its parent: a `,` that ends its list
    /// without a closing delimiter after it (`where` clauses) is optional (research R5).
    fn leave_last_child(&mut self, node: Node<'_>) {
        if let Some((leaf_id, token)) = self.last_leaf
            && leaf_id == node.id()
            && self.tokens[token].class == TokenClass::Separator
        {
            self.tokens[token].class = TokenClass::OptionalPunctuation;
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
            if same_level {
                if let Some(prefix) = &mut self.prefix {
                    prefix.has_error |= node.has_error();
                }
            } else {
                self.prefix = Some(Prefix {
                    parent_id,
                    token: self.tokens.len() as u32,
                    byte: self.shifted(node.start_byte()),
                    has_error: node.has_error(),
                });
            }
        } else if same_level
            && kinds::entity_kind(node.kind(), None).is_none()
            && !self.is_import_item(node, parent)
        {
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
            has_error: node.has_error() || prefix.is_some_and(|prefix| prefix.has_error),
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
        let text = &self.code[range.clone()];
        let single = parent.and_then(|parent| self.single_elements.get(&parent.id()).copied());
        let class = match (single, text) {
            (Some(kinds::SingleElement::Tuple), b",") => TokenClass::Punctuation,
            (Some(kinds::SingleElement::UseList), b"," | b"{" | b"}") => TokenClass::OptionalPunctuation,
            _ => tokens::classify(node, parent, text, optional_brace),
        };
        self.last_leaf = Some((node.id(), self.tokens.len()));
        self.tokens.push(Token {
            class,
            byte_range: ByteRange::new(self.shifted(range.start), self.shifted(range.end)),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn tree(code: &[u8]) -> Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("grammar loads");
        parser.parse(code, None).expect("parses")
    }

    fn comma_classes(code: &[u8]) -> Vec<TokenClass> {
        let (tokens, _) = collect(
            &tree(code),
            code,
            0,
            "src/lib.rs",
            Deadline::starting_now(Duration::from_secs(60)),
        )
        .expect("walks");
        tokens
            .iter()
            .filter(|token| &code[token.byte_range.as_usize()] == b",")
            .map(|token| token.class)
            .collect()
    }

    #[test]
    fn a_comma_that_ends_its_list_is_optional() {
        assert_eq!(
            comma_classes(b"fn f<T, U>(a: u8, b: u8) where T: Copy, U: Copy, {}"),
            [
                TokenClass::Separator,
                TokenClass::Separator,
                TokenClass::Separator,
                TokenClass::OptionalPunctuation,
            ]
        );
    }

    /// Walk time of a flat array of `elements` numbers, the best of two runs; the parse is not
    /// timed.
    fn array_walk_time(elements: usize) -> Duration {
        let code = format!("const A: [u8; 0] = [{}];\n", "0,".repeat(elements)).into_bytes();
        let tree = tree(&code);
        (0..2)
            .map(|_| {
                let started = std::time::Instant::now();
                collect(
                    &tree,
                    &code,
                    0,
                    "src/lib.rs",
                    Deadline::starting_now(Duration::from_secs(600)),
                )
                .expect("walks");
                started.elapsed()
            })
            .min()
            .expect("two runs")
    }

    #[test]
    fn the_walk_takes_linear_time() {
        // 64 times the elements take about 64 times as long. Asking each comma for its next
        // sibling made the walk quadratic: about 170 times as long at these sizes.
        let small = array_walk_time(6_250);
        let large = array_walk_time(400_000);
        assert!(
            large < small * 110,
            "6,250 elements: {small:?}, 400,000 elements: {large:?}"
        );
    }

    #[test]
    fn the_walk_stops_at_the_deadline() {
        let code = b"fn f() {}";
        assert!(matches!(
            collect(
                &tree(code),
                code,
                0,
                "src/lib.rs",
                Deadline::starting_now(Duration::ZERO)
            ),
            Err(ParseError::Timeout { limit_ms: 0, .. })
        ));
    }
}
