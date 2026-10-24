//! Rust qualified paths (research R7, clarification 3).

use tree_sitter::Node;

/// Directories whose files belong to a crate rooted in them.
const CRATE_DIRECTORIES: &[&str] = &["src", "tests", "benches", "examples"];

/// The module path of a file from its repository path: `crate` followed by the modules below the
/// nearest crate directory.
pub fn module_path(file_path: &str) -> String {
    let components: Vec<&str> = file_path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    // A `tests/` directory inside `src/` is a module, so `src/` wins over the other directories.
    let root = components.iter().rposition(|part| *part == "src").or_else(|| {
        components
            .iter()
            .rposition(|part| CRATE_DIRECTORIES.contains(part))
    });
    let Some(root) = root else {
        return "crate".to_owned();
    };
    let below = &components[root + 1..];
    let is_own_crate_root = matches!(
        (components[root], below),
        (_, [])
            | ("src", ["lib.rs" | "main.rs"])
            | ("src", ["bin", _] | ["bin", _, "main.rs"])
            | ("tests" | "benches" | "examples", [_] | [_, "main.rs"])
    );
    if is_own_crate_root {
        return "crate".to_owned();
    }
    let mut modules: Vec<&str> = below.to_vec();
    if let Some(last) = modules.pop() {
        let stem = last.strip_suffix(".rs").unwrap_or(last);
        if stem != "mod" {
            modules.push(stem);
        }
    }
    std::iter::once("crate")
        .chain(modules)
        .collect::<Vec<_>>()
        .join("::")
}

/// The qualified path of an `impl` block and the scope of its items.
///
/// Inherent: `scope::<impl Type>`, items under `scope::Type`. Trait: `scope::<Type as Trait>`,
/// items under the same name.
pub fn impl_paths(node: Node<'_>, source: &[u8], scope: &str) -> (String, String) {
    let self_type = node
        .child_by_field_name("type")
        .map(|ty| compact_text(ty, source))
        .unwrap_or_default();
    match node.child_by_field_name("trait") {
        Some(trait_node) => {
            let path = format!("{scope}::<{self_type} as {}>", compact_text(trait_node, source));
            (path.clone(), path)
        }
        None => (
            format!("{scope}::<impl {self_type}>"),
            format!("{scope}::{self_type}"),
        ),
    }
}

/// The node's code as its tokens joined without trivia: a space only between two word tokens
/// (`&'a mut Vec<u8>`).
pub fn compact_text(node: Node<'_>, source: &[u8]) -> String {
    let mut text = String::new();
    let mut cursor = node.walk();
    loop {
        let current = cursor.node();
        let is_leaf = current.child_count() == 0 || super::kinds::is_atomic(current.kind());
        if !is_leaf && cursor.goto_first_child() {
            continue;
        }
        if is_leaf && !matches!(current.kind(), "line_comment" | "block_comment") {
            let piece = String::from_utf8_lossy(&source[current.byte_range()]);
            let joins_words = text.chars().last().is_some_and(is_word_char)
                && piece.chars().next().is_some_and(is_word_char);
            if joins_words {
                text.push(' ');
            }
            text.push_str(&piece);
        }
        // Next node: a sibling, or the sibling of the nearest ancestor below `node`.
        loop {
            if cursor.node() == node {
                return text;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return text;
            }
        }
    }
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '\''
}

#[cfg(test)]
mod tests {
    use super::module_path;

    #[test]
    fn crate_roots_are_crate() {
        for path in [
            "src/lib.rs",
            "src/main.rs",
            "crates/core/gedo-core/src/lib.rs",
            "src/bin/tool.rs",
            "src/bin/tool/main.rs",
            "tests/transport.rs",
            "benches/wal_append.rs",
            "examples/roundtrip.rs",
            "build.rs",
            "crates/x/build.rs",
        ] {
            assert_eq!(module_path(path), "crate", "{path}");
        }
    }

    #[test]
    fn modules_follow_the_directories() {
        assert_eq!(module_path("src/ids.rs"), "crate::ids");
        assert_eq!(module_path("src/ports/mod.rs"), "crate::ports");
        assert_eq!(module_path("src/ports/wal.rs"), "crate::ports::wal");
        assert_eq!(
            module_path("crates/core/gedo-core/src/memory/parser.rs"),
            "crate::memory::parser"
        );
        assert_eq!(module_path("tests/common/mod.rs"), "crate::common");
        assert_eq!(module_path("tests/common/repo.rs"), "crate::common::repo");
    }

    #[test]
    fn the_nearest_crate_directory_wins() {
        assert_eq!(module_path("src/tests/helpers.rs"), "crate::tests::helpers");
        assert_eq!(module_path("tools/xtask/src/arch_check.rs"), "crate::arch_check");
    }

    #[test]
    fn windows_separators_are_accepted() {
        assert_eq!(module_path("src\\ports\\wal.rs"), "crate::ports::wal");
    }
}
