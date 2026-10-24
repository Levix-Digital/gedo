//! Rust grammar nodes that are code entities (research R7).

use gedo_core::syntax::EntityKind;
use tree_sitter::Node;

/// The entity kind of a grammar node, given the kind of the nearest enclosing entity.
pub fn entity_kind(node_kind: &str, enclosing: Option<EntityKind>) -> Option<EntityKind> {
    let in_type_scope = matches!(enclosing, Some(EntityKind::Impl | EntityKind::Trait));
    let kind = match node_kind {
        "mod_item" => EntityKind::Module,
        "function_item" | "function_signature_item" if in_type_scope => EntityKind::Method,
        "function_item" | "function_signature_item" => EntityKind::Function,
        "struct_item" => EntityKind::Struct,
        "enum_item" => EntityKind::Enum,
        "union_item" => EntityKind::Union,
        "trait_item" => EntityKind::Trait,
        "impl_item" => EntityKind::Impl,
        "const_item" if in_type_scope => EntityKind::AssociatedConstant,
        "const_item" => EntityKind::Constant,
        "static_item" => EntityKind::Static,
        "type_item" if in_type_scope => EntityKind::AssociatedType,
        "associated_type" => EntityKind::AssociatedType,
        "type_item" => EntityKind::TypeAlias,
        "macro_definition" => EntityKind::Macro,
        "use_declaration" => EntityKind::Import,
        _ => return None,
    };
    Some(kind)
}

/// The node where an entity's body starts, if it has one.
pub fn body_node<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    match node.kind() {
        "const_item" | "static_item" => node.child_by_field_name("value"),
        "type_item" => node.child_by_field_name("type"),
        // `macro_rules! name { ... }`: the rules follow the name.
        "macro_definition" => node.child_by_field_name("name")?.next_sibling(),
        _ => node.child_by_field_name("body"),
    }
}

/// Nodes emitted as one token, whatever their inner structure.
pub fn is_atomic(node_kind: &str) -> bool {
    matches!(
        node_kind,
        "line_comment"
            | "block_comment"
            | "string_literal"
            | "raw_string_literal"
            | "char_literal"
            | "integer_literal"
            | "float_literal"
            | "boolean_literal"
            | "lifetime"
            | "label"
    )
}

/// Whether a block holding a single expression is the body of a match arm or a closure, which a
/// formatter may write with or without braces: `x => { value }` means `x => value,` and
/// `|x| { value }` means `|x| value`. A lone `return`, `break` or `continue` counts as an
/// expression (research R5).
pub fn is_optional_block(node: Node<'_>, parent: Option<Node<'_>>) -> bool {
    let in_optional_position =
        parent.is_some_and(|parent| matches!(parent.kind(), "match_arm" | "closure_expression"));
    if node.kind() != "block" || !in_optional_position {
        return false;
    }
    let mut cursor = node.walk();
    let mut code = node
        .named_children(&mut cursor)
        .filter(|child| !matches!(child.kind(), "line_comment" | "block_comment"));
    let only = code.next();
    only.is_some_and(|child| {
        code.next().is_none()
            && (is_expression(child.kind()) || is_jump_statement(child) || is_unterminated_statement(child))
    })
}

/// A block-like expression in statement position (`match`, `if`, `loop`) without `;`: the grammar
/// makes it a statement, but it is the block's value.
fn is_unterminated_statement(node: Node<'_>) -> bool {
    node.kind() == "expression_statement"
        && node
            .child(node.child_count().saturating_sub(1))
            .is_some_and(|last| last.kind() != ";")
}

/// Whether a statement is `return`, `break` or `continue` followed by `;`: a formatter adds or
/// removes that `;` when the statement ends a block, so it is optional (research R5).
pub fn is_jump_statement(node: Node<'_>) -> bool {
    node.kind() == "expression_statement"
        && node.named_child(0).is_some_and(|expression| {
            matches!(
                expression.kind(),
                "return_expression" | "break_expression" | "continue_expression"
            )
        })
}

fn is_expression(node_kind: &str) -> bool {
    !matches!(
        node_kind,
        "expression_statement"
            | "let_declaration"
            | "empty_statement"
            | "attribute_item"
            | "inner_attribute_item"
            | "macro_invocation_statement"
    ) && !node_kind.ends_with("_item")
        && !node_kind.ends_with("_declaration")
}
