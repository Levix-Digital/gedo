//! Conformance suite of the Rust support (User Story 1, 5): every fixture's syntax map is
//! compared with its reviewed snapshot. The snapshots are also the cross-platform reference of
//! User Story 3: they must match byte for byte on Windows and Linux.

mod common;

use gedo_core::ports::{ParseError, Parser};
use gedo_core::syntax::EntityKind;
use gedo_parse::TreeSitterParser;

#[test]
fn every_fixture_matches_its_snapshot() {
    let fixtures = common::fixtures();
    assert!(fixtures.len() >= 10, "fixtures found: {}", fixtures.len());
    for (path, source) in fixtures {
        let snapshot_name = path.replace(['/', '.'], "_");
        insta::assert_snapshot!(snapshot_name, common::render(&common::syntax_map(&path, &source)));
    }
}

#[test]
fn entities_have_the_expected_kinds_and_paths() {
    let source = std::fs::read(common::fixture_root().join("src/lib.rs")).expect("fixture");
    let map = common::syntax_map("src/lib.rs", &source);
    let find = |path: &str| {
        map.entities
            .iter()
            .filter(|entity| entity.qualified_path == path)
            .map(|entity| entity.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(find("crate::a"), [EntityKind::Module]);
    assert_eq!(find("crate::LIMIT"), [EntityKind::Constant]);
    assert_eq!(find("crate::COUNTER"), [EntityKind::Static]);
    assert_eq!(find("crate::Map"), [EntityKind::TypeAlias]);
    assert_eq!(find("crate::Invoice"), [EntityKind::Struct]);
    assert_eq!(find("crate::Status"), [EntityKind::Enum]);
    assert_eq!(find("crate::Bits"), [EntityKind::Union]);
    assert_eq!(find("crate::Priced"), [EntityKind::Trait]);
    assert_eq!(find("crate::Priced::CURRENCY"), [EntityKind::AssociatedConstant]);
    assert_eq!(find("crate::Priced::Amount"), [EntityKind::AssociatedType]);
    assert_eq!(find("crate::Priced::price"), [EntityKind::Method]);
    assert_eq!(find("crate::<impl Invoice>"), [EntityKind::Impl]);
    assert_eq!(find("crate::Invoice::total"), [EntityKind::Method]);
    assert_eq!(find("crate::Invoice::total::helper"), [EntityKind::Function]);
    assert_eq!(find("crate::Invoice::total::Local"), [EntityKind::Struct]);
    assert_eq!(find("crate::<Invoice as Priced>::price"), [EntityKind::Method]);
    assert_eq!(
        find("crate::<Invoice as Priced>::Amount"),
        [EntityKind::AssociatedType]
    );
    assert_eq!(find("crate::<Invoice as Display>::fmt"), [EntityKind::Method]);
    assert_eq!(find("crate::Stack<T>::push"), [EntityKind::Method]);
    assert_eq!(find("crate::Stack<u8>::push"), [EntityKind::Method]);
    assert_eq!(
        find("crate::<Stack<T> as From<&'a[T]>>::from"),
        [EntityKind::Method]
    );
    assert_eq!(
        find("crate::platform"),
        [EntityKind::Function, EntityKind::Function]
    );
    assert_eq!(find("crate::inline::nested::deep"), [EntityKind::Function]);
    assert_eq!(find("crate::inline::r#type"), [EntityKind::Function]);
    assert_eq!(find("crate::inline::größe"), [EntityKind::Function]);
    assert_eq!(find("crate::square"), [EntityKind::Macro]);
    assert_eq!(find("crate::abs"), [EntityKind::Function]);
    assert_eq!(find("crate::later"), [EntityKind::Function]);
    assert!(
        map.entities
            .iter()
            .all(|entity| entity.kind != EntityKind::Import)
    );
    assert!(map.entities.iter().all(|entity| !entity.has_error));
}

#[test]
fn duplicates_are_told_apart_by_occurrence() {
    let source = std::fs::read(common::fixture_root().join("src/lib.rs")).expect("fixture");
    let map = common::syntax_map("src/lib.rs", &source);
    let occurrences: Vec<u32> = map
        .entities
        .iter()
        .filter(|entity| entity.qualified_path == "crate::platform")
        .map(|entity| entity.occurrence)
        .collect();
    assert_eq!(occurrences, [0, 1]);
}

#[test]
fn attributes_and_documentation_belong_to_their_entity() {
    let source = std::fs::read(common::fixture_root().join("src/lib.rs")).expect("fixture");
    let map = common::syntax_map("src/lib.rs", &source);
    let text = |path: &str| {
        let entity = map
            .entities
            .iter()
            .find(|entity| entity.qualified_path == path)
            .expect(path);
        String::from_utf8_lossy(&source[entity.byte_range.as_usize()]).into_owned()
    };
    assert!(text("crate::Invoice").starts_with("#[derive(Debug, Clone)]"));
    assert!(text("crate::LIMIT").starts_with("/// A constant with documentation."));
    assert!(text("crate::Bits").starts_with("#[repr(C)]"));
}

#[test]
fn the_byte_order_mark_is_skipped_and_ranges_stay_on_the_original_bytes() {
    let source = std::fs::read(common::fixture_root().join("src/bom.rs")).expect("fixture");
    let map = common::syntax_map("src/bom.rs", &source);
    let entity = &map.entities[0];
    assert_eq!(entity.qualified_path, "crate::bom::after_bom");
    assert_eq!(&source[entity.byte_range.as_usize()], b"pub fn after_bom() {}");
}

#[test]
fn other_languages_are_unsupported() {
    let parser = TreeSitterParser::new();
    assert_eq!(parser.language_of("README.md"), None);
    assert_eq!(
        parser.parse("README.md", b"# Gedo"),
        Err(ParseError::UnsupportedLanguage {
            path: "README.md".into()
        })
    );
}
