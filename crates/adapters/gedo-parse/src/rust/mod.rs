//! The Rust mapping: entity kinds, token classes and qualified paths.

pub mod kinds;
pub mod paths;
pub mod tokens;

use std::sync::OnceLock;

use tree_sitter::{Language, Node};

/// The Rust grammar, loaded once.
pub fn language() -> &'static Language {
    static LANGUAGE: OnceLock<Language> = OnceLock::new();
    LANGUAGE.get_or_init(|| tree_sitter_rust::LANGUAGE.into())
}

/// The kind of `node`, read from a table built once per process instead of through
/// `Node::kind`, which measures and validates the name on every call.
pub fn kind_of<'tree>(node: Node<'tree>) -> &'tree str {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        let language = language();
        (0..language.node_kind_count())
            .map(|id| {
                u16::try_from(id)
                    .ok()
                    .and_then(|id| language.node_kind_for_id(id))
                    .unwrap_or_default()
            })
            .collect()
    });
    names
        .get(usize::from(node.kind_id()))
        .copied()
        .filter(|name| !name.is_empty())
        // The error symbol and any id past the table.
        .unwrap_or_else(|| node.kind())
}

#[cfg(test)]
mod tests {
    use super::{kind_of, language};

    /// Checks every node of `source`, in depth-first order; returns the kinds visited.
    fn assert_kinds_match(source: &str) -> Vec<String> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language()).expect("grammar");
        let tree = parser.parse(source, None).expect("tree");
        let mut cursor = tree.walk();
        let mut visited = Vec::new();
        loop {
            let node = cursor.node();
            assert_eq!(kind_of(node), node.kind(), "node {node:?}");
            visited.push(node.kind().to_owned());
            if cursor.goto_first_child() || cursor.goto_next_sibling() {
                continue;
            }
            while cursor.goto_parent() {
                if cursor.goto_next_sibling() {
                    break;
                }
            }
            if cursor.node() == tree.root_node() {
                break;
            }
        }
        assert!(visited.len() > 1);
        visited
    }

    #[test]
    fn the_kind_table_names_every_node_like_tree_sitter() {
        assert_kinds_match(include_str!("../../tests/fixtures/rust/src/lib.rs"));
    }

    #[test]
    fn the_kind_table_names_error_nodes() {
        let kinds =
            assert_kinds_match("fn broken( { let x = ; }\nstruct A { a: }\n#[derive(Debug]\nstruct B;");
        // The error symbol lies outside the table and goes through the fallback.
        assert!(kinds.iter().any(|kind| kind == "ERROR"), "{kinds:?}");
    }
}
