//! Property tests of the normalization and hash rules, on synthetic files (no grammar).

use gedo_ast::normalize;
use gedo_core::syntax::{
    ByteRange, EntityKind, Language, ParsedFile, RawEntity, StructuralHash, SyntaxMap, Token, TokenClass,
    TokenRange,
};
use proptest::prelude::*;

/// One element of a synthetic module body.
#[derive(Clone, Debug)]
enum Item {
    Token(TokenClass, String),
    /// A function with its own tokens.
    Function(String, Vec<(TokenClass, String)>),
    /// An import with its own tokens.
    Import(Vec<(TokenClass, String)>),
}

/// Builds a file holding one module (`mod m {` ... `}`) whose body is `items`.
fn build(items: &[Item], line_ending: &str) -> (ParsedFile, Vec<u8>) {
    let mut source = Vec::new();
    let mut tokens = Vec::new();
    let mut push = |class: TokenClass, text: &str, tokens: &mut Vec<Token>| {
        let start = source.len() as u32;
        source.extend_from_slice(text.replace('\n', line_ending).as_bytes());
        tokens.push(Token {
            class,
            byte_range: ByteRange::new(start, source.len() as u32),
        });
        source.extend_from_slice(line_ending.as_bytes());
    };
    let mut entities = vec![entity(EntityKind::Module, "m", None, 0)];
    push(TokenClass::Keyword, "mod", &mut tokens);
    push(TokenClass::Identifier, "m", &mut tokens);
    entities[0].body_start = Some(tokens.len() as u32);
    push(TokenClass::OpenDelimiter, "{", &mut tokens);
    for item in items {
        match item {
            Item::Token(class, text) => push(*class, text, &mut tokens),
            Item::Function(_, body) | Item::Import(body) => {
                let (kind, name) = match item {
                    Item::Function(name, _) => (EntityKind::Function, name.as_str()),
                    _ => (EntityKind::Import, ""),
                };
                let mut child = entity(kind, name, Some(0), tokens.len() as u32);
                push(
                    TokenClass::Keyword,
                    if kind == EntityKind::Import { "use" } else { "fn" },
                    &mut tokens,
                );
                child.body_start = (kind == EntityKind::Function).then_some(tokens.len() as u32);
                for (class, text) in body {
                    push(*class, text, &mut tokens);
                }
                child.tokens.end = tokens.len() as u32;
                entities.push(child);
            }
        }
    }
    push(TokenClass::CloseDelimiter, "}", &mut tokens);
    entities[0].tokens.end = tokens.len() as u32;
    let length = source.len() as u32;
    for entity in &mut entities {
        let first = tokens[entity.tokens.start as usize].byte_range.start;
        let last = tokens[(entity.tokens.end - 1) as usize].byte_range.end;
        entity.byte_range = ByteRange::new(first, last.min(length));
    }
    let file = ParsedFile {
        language: Language::Rust,
        path: "src/lib.rs".into(),
        unit: "src".into(),
        tokens,
        entities,
        has_error: false,
    };
    (file, source)
}

fn entity(kind: EntityKind, name: &str, container: Option<u32>, start: u32) -> RawEntity {
    RawEntity {
        kind,
        language_kind: kind.name().into(),
        name: name.into(),
        qualified_path: format!("crate::m::{name}"),
        occurrence: 0,
        byte_range: ByteRange::new(0, 0),
        container,
        tokens: TokenRange { start, end: start },
        body_start: None,
        has_error: false,
    }
}

fn syntax_map(items: &[Item]) -> SyntaxMap {
    let (file, source) = build(items, "\n");
    normalize(&file, &source).expect("synthetic files are well formed")
}

fn hashes(map: &SyntaxMap) -> Vec<(String, StructuralHash, StructuralHash, StructuralHash)> {
    map.entities
        .iter()
        .map(|e| {
            (
                e.qualified_path.clone(),
                e.structural_hash,
                e.signature_hash,
                e.body_hash,
            )
        })
        .collect()
}

fn significant_class() -> impl Strategy<Value = TokenClass> {
    prop_oneof![
        Just(TokenClass::Identifier),
        Just(TokenClass::Keyword),
        Just(TokenClass::Literal),
        Just(TokenClass::Punctuation),
    ]
}

fn token() -> impl Strategy<Value = (TokenClass, String)> {
    (significant_class(), "[a-z0-9+*=]{1,4}")
}

fn item() -> impl Strategy<Value = Item> {
    prop_oneof![
        3 => token().prop_map(|(class, text)| Item::Token(class, text)),
        1 => ("[a-z]{1,3}", prop::collection::vec(token(), 1..6))
            .prop_map(|(name, body)| Item::Function(format!("f_{name}"), body)),
        1 => prop::collection::vec(token(), 1..4).prop_map(Item::Import),
    ]
}

fn items() -> impl Strategy<Value = Vec<Item>> {
    prop::collection::vec(item(), 0..12)
}

/// Puts a comment after the item at each chosen position, and inside function bodies.
fn with_comments(items: &[Item], positions: &[usize]) -> Vec<Item> {
    let mut result = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let item = match item {
            Item::Function(name, body) => {
                let mut body = body.clone();
                body.insert(0, (TokenClass::DocComment, "/// doc".into()));
                Item::Function(name.clone(), body)
            }
            other => other.clone(),
        };
        result.push(item);
        if positions.contains(&index) {
            result.push(Item::Token(TokenClass::Comment, "// note".into()));
        }
    }
    result
}

proptest! {
    #[test]
    fn comments_never_change_a_hash(items in items(), positions in prop::collection::vec(0usize..12, 0..6)) {
        prop_assert_eq!(hashes(&syntax_map(&items)), hashes(&syntax_map(&with_comments(&items, &positions))));
    }

    #[test]
    fn line_endings_never_change_a_hash(items in items()) {
        let (lf, lf_source) = build(&items, "\n");
        let (crlf, crlf_source) = build(&items, "\r\n");
        let lf = normalize(&lf, &lf_source).expect("well formed");
        let crlf = normalize(&crlf, &crlf_source).expect("well formed");
        prop_assert_eq!(hashes(&lf), hashes(&crlf));
    }

    #[test]
    fn permuting_imports_never_changes_a_hash(items in items(), rotation in 0usize..12) {
        let imports: Vec<Item> = items.iter().filter(|item| matches!(item, Item::Import(_))).cloned().collect();
        let mut rotated_imports = imports.clone();
        if !rotated_imports.is_empty() {
            let shift = rotation % rotated_imports.len();
            rotated_imports.rotate_left(shift);
        }
        let mut replacements = rotated_imports.into_iter();
        let permuted: Vec<Item> = items
            .iter()
            .map(|item| match item {
                Item::Import(_) => replacements.next().expect("same number of imports"),
                other => other.clone(),
            })
            .collect();
        prop_assert_eq!(hashes(&syntax_map(&items)), hashes(&syntax_map(&permuted)));
    }

    #[test]
    fn a_trailing_separator_never_changes_a_hash(items in items()) {
        let mut wrapped = items.clone();
        wrapped.insert(0, Item::Token(TokenClass::OpenDelimiter, "(".into()));
        wrapped.push(Item::Token(TokenClass::CloseDelimiter, ")".into()));
        let mut with_separator = items.clone();
        with_separator.insert(0, Item::Token(TokenClass::OpenDelimiter, "(".into()));
        with_separator.push(Item::Token(TokenClass::Separator, ",".into()));
        with_separator.push(Item::Token(TokenClass::Comment, "// end".into()));
        with_separator.push(Item::Token(TokenClass::CloseDelimiter, ")".into()));
        prop_assert_eq!(hashes(&syntax_map(&wrapped)), hashes(&syntax_map(&with_separator)));
    }

    #[test]
    fn changing_a_function_token_changes_it_and_its_container_only(
        items in items(),
        function in ("[a-z]{1,3}", prop::collection::vec(token(), 1..6)),
        changed_index in 0usize..6,
    ) {
        let (name, body) = function;
        let name = format!("target_{name}");
        let changed_index = changed_index % body.len();
        let mut changed_body = body.clone();
        changed_body[changed_index].1.push('x');
        let mut original = items.clone();
        original.push(Item::Function(name.clone(), body));
        let mut changed = items;
        changed.push(Item::Function(name.clone(), changed_body));
        let before = syntax_map(&original);
        let after = syntax_map(&changed);
        for (old, new) in before.entities.iter().zip(&after.entities) {
            let should_change = old.name == name || old.kind == EntityKind::Module;
            prop_assert_eq!(old.structural_hash != new.structural_hash, should_change, "{}", old.qualified_path);
        }
        let target = after.entities.iter().position(|e| e.name == name).expect("target present");
        prop_assert_eq!(before.entities[target].signature_hash, after.entities[target].signature_hash);
        prop_assert_ne!(before.entities[target].body_hash, after.entities[target].body_hash);
    }

    #[test]
    fn changing_a_token_class_changes_the_hash(text in "[a-z]{1,4}") {
        let identifier = syntax_map(&[Item::Token(TokenClass::Identifier, text.clone())]);
        let keyword = syntax_map(&[Item::Token(TokenClass::Keyword, text)]);
        prop_assert_ne!(identifier.entities[0].structural_hash, keyword.entities[0].structural_hash);
    }

    #[test]
    fn token_boundaries_are_part_of_the_hash(left in "[a-z]{1,3}", right in "[a-z]{1,3}") {
        let split = syntax_map(&[
            Item::Token(TokenClass::Identifier, left.clone()),
            Item::Token(TokenClass::Identifier, right.clone()),
        ]);
        let joined = syntax_map(&[Item::Token(TokenClass::Identifier, format!("{left}{right}"))]);
        prop_assert_ne!(split.entities[0].structural_hash, joined.entities[0].structural_hash);
    }
}

#[test]
fn imports_are_not_returned_and_containers_are_renumbered() {
    let map = syntax_map(&[
        Item::Import(vec![(TokenClass::Identifier, "a".into())]),
        Item::Function("f".into(), vec![(TokenClass::Identifier, "x".into())]),
    ]);
    let kinds: Vec<EntityKind> = map.entities.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [EntityKind::Module, EntityKind::Function]);
    assert_eq!(map.entities[1].container, Some(0));
}

#[test]
fn the_hash_of_a_fixed_input_is_stable() {
    // A change here means the V1 rules changed: that needs HashFormat::V2 (research R6).
    let map = syntax_map(&[Item::Function(
        "f".into(),
        vec![(TokenClass::Identifier, "x".into())],
    )]);
    assert_eq!(
        map.entities[1].structural_hash.to_string(),
        "402bce161553e8967d9c95bcc30dbc6e08cb7ed0c694533224852d9f6155f78f",
    );
}
