//! Rust qualified paths (research R7, clarification 3).

use tree_sitter::Node;

use super::kind_of;

/// Directories whose files belong to a crate rooted in them.
const CRATE_DIRECTORIES: &[&str] = &["src", "tests", "benches", "examples"];

/// The module path of a file from its repository path: `crate` followed by the modules below the
/// nearest crate directory.
pub fn module_path(file_path: &str) -> String {
    let components = path_components(file_path);
    let Some(root) = crate_directory(&components) else {
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
    // Cargo builds every directory of `src/bin/` as one binary rooted in its `main.rs`, so its
    // other files are modules of that binary. Below `tests/`, `benches/` and `examples/` a
    // directory may also be shared modules (`tests/common/`), which one file cannot tell apart.
    let below = match (components[root], below) {
        ("src", ["bin", _, rest @ ..]) => rest,
        _ => below,
    };
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

/// The repository path of a file's compilation unit: the nearest crate directory, followed by the
/// target's name for a target of its own (`tests/transport.rs` is `tests/transport`,
/// `src/bin/tool/main.rs` is `src/bin/tool`); the file's directory when there is no crate
/// directory (research R4 of `specs/003-node-identity`).
pub fn unit_of(file_path: &str) -> String {
    let components = path_components(file_path);
    let Some(root) = crate_directory(&components) else {
        return components[..components.len().saturating_sub(1)].join("/");
    };
    let base = components[..=root].join("/");
    let below = &components[root + 1..];
    let target = match (components[root], below) {
        ("src", ["bin", file]) => Some(format!("bin/{}", file.strip_suffix(".rs").unwrap_or(file))),
        ("src", ["bin", directory, _, ..]) => Some(format!("bin/{directory}")),
        ("tests" | "benches" | "examples", [file]) => {
            Some(file.strip_suffix(".rs").unwrap_or(file).to_owned())
        }
        ("tests" | "benches" | "examples", [directory, "main.rs"]) => Some((*directory).to_owned()),
        _ => None,
    };
    match target {
        Some(target) => format!("{base}/{target}"),
        None => base,
    }
}

fn path_components(file_path: &str) -> Vec<&str> {
    file_path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect()
}

/// Index of the nearest crate directory. A `tests/` directory inside `src/` is a module, so `src/`
/// wins over the other directories.
fn crate_directory(components: &[&str]) -> Option<usize> {
    components.iter().rposition(|part| *part == "src").or_else(|| {
        components
            .iter()
            .rposition(|part| CRATE_DIRECTORIES.contains(part))
    })
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
        let is_leaf = current.child_count() == 0 || super::kinds::is_atomic(kind_of(current));
        if !is_leaf && cursor.goto_first_child() {
            continue;
        }
        if is_leaf && !matches!(kind_of(current), "line_comment" | "block_comment") {
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
    use super::{module_path, unit_of};

    #[test]
    fn units_are_crate_directories_and_targets() {
        for (path, unit) in [
            ("src/lib.rs", "src"),
            ("src/a/b.rs", "src"),
            ("crates/core/gedo-ast/src/hash.rs", "crates/core/gedo-ast/src"),
            ("src/bin/tool.rs", "src/bin/tool"),
            ("src/bin/tool/main.rs", "src/bin/tool"),
            ("src/bin/tool/helper.rs", "src/bin/tool"),
            ("tests/transport.rs", "tests/transport"),
            ("tests/suite/main.rs", "tests/suite"),
            ("tests/common/mod.rs", "tests"),
            ("crates/x/benches/speed.rs", "crates/x/benches/speed"),
            ("build.rs", ""),
            ("crates/x/build.rs", "crates/x"),
            ("crates\\x\\src\\lib.rs", "crates/x/src"),
        ] {
            assert_eq!(unit_of(path), unit, "{path}");
        }
    }

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
    fn files_of_a_multi_file_binary_are_its_modules() {
        assert_eq!(module_path("src/bin/tool/helper.rs"), "crate::helper");
        assert_eq!(module_path("src/bin/tool/cli/mod.rs"), "crate::cli");
        assert_eq!(module_path("src/bin/tool/cli/args.rs"), "crate::cli::args");
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
