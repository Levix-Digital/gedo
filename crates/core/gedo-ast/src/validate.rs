//! Structural checks of a [`ParsedFile`] before it is normalized.

use gedo_core::syntax::{ParsedFile, RawEntity};

/// Checks the rules of `data-model.md`; the error says which rule broke where.
pub fn check(file: &ParsedFile, source_len: usize) -> Result<(), String> {
    check_tokens(file, source_len)?;
    for (index, entity) in file.entities.iter().enumerate() {
        check_entity(file, index, entity, source_len)?;
    }
    check_siblings(file)
}

fn check_tokens(file: &ParsedFile, source_len: usize) -> Result<(), String> {
    let mut previous_end = 0;
    for (index, token) in file.tokens.iter().enumerate() {
        let range = token.byte_range;
        if range.start > range.end || range.end as usize > source_len {
            return Err(format!(
                "token {index} has the byte range {range:?} outside the source"
            ));
        }
        if range.start < previous_end {
            return Err(format!("token {index} overlaps or precedes the token before it"));
        }
        previous_end = range.end;
    }
    Ok(())
}

fn check_entity(
    file: &ParsedFile,
    index: usize,
    entity: &RawEntity,
    source_len: usize,
) -> Result<(), String> {
    let tokens = entity.tokens;
    if tokens.start > tokens.end || tokens.end as usize > file.tokens.len() {
        return Err(format!(
            "entity {index} has the token range {tokens:?} outside the file"
        ));
    }
    let bytes = entity.byte_range;
    if bytes.start > bytes.end || bytes.end as usize > source_len {
        return Err(format!(
            "entity {index} has the byte range {bytes:?} outside the source"
        ));
    }
    if let Some(body) = entity.body_start
        && !(tokens.start..=tokens.end).contains(&body)
    {
        return Err(format!(
            "entity {index} has its body start {body} outside its tokens"
        ));
    }
    if let Some(container) = entity.container {
        let Some(parent) = file
            .entities
            .get(container as usize)
            .filter(|_| (container as usize) < index)
        else {
            return Err(format!(
                "entity {index} has the container {container}, which does not precede it"
            ));
        };
        if tokens.start < parent.tokens.start || tokens.end > parent.tokens.end {
            return Err(format!(
                "entity {index} is not inside the tokens of its container {container}"
            ));
        }
    }
    Ok(())
}

/// Entities with the same container must not overlap, so each token belongs to one of them.
fn check_siblings(file: &ParsedFile) -> Result<(), String> {
    let mut last_end_by_container = std::collections::HashMap::new();
    for (index, entity) in file.entities.iter().enumerate() {
        let last_end = last_end_by_container.entry(entity.container).or_insert(0);
        if entity.tokens.start < *last_end {
            return Err(format!(
                "entity {index} overlaps the entity before it in the same container"
            ));
        }
        *last_end = entity.tokens.end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use gedo_core::syntax::{
        ByteRange, EntityKind, Language, ParsedFile, RawEntity, Token, TokenClass, TokenRange,
    };

    use super::check;

    fn token(start: u32, end: u32) -> Token {
        Token {
            class: TokenClass::Identifier,
            byte_range: ByteRange::new(start, end),
        }
    }

    fn entity(tokens: (u32, u32), container: Option<u32>) -> RawEntity {
        RawEntity {
            kind: EntityKind::Function,
            language_kind: "function_item".into(),
            name: "f".into(),
            qualified_path: "crate::f".into(),
            occurrence: 0,
            byte_range: ByteRange::new(0, 4),
            container,
            tokens: TokenRange {
                start: tokens.0,
                end: tokens.1,
            },
            body_start: None,
            has_error: false,
        }
    }

    fn file(tokens: Vec<Token>, entities: Vec<RawEntity>) -> ParsedFile {
        ParsedFile {
            language: Language::Rust,
            path: "src/lib.rs".into(),
            tokens,
            entities,
        }
    }

    #[test]
    fn accepts_a_well_formed_file() {
        let parsed = file(
            vec![token(0, 1), token(2, 3)],
            vec![entity((0, 2), None), entity((1, 2), Some(0))],
        );
        assert_eq!(check(&parsed, 4), Ok(()));
    }

    #[test]
    fn refuses_a_token_outside_the_source() {
        assert!(
            check(&file(vec![token(0, 9)], vec![]), 4)
                .expect_err("the check fails")
                .contains("outside the source")
        );
    }

    #[test]
    fn refuses_overlapping_tokens() {
        let parsed = file(vec![token(0, 2), token(1, 3)], vec![]);
        assert!(
            check(&parsed, 4)
                .expect_err("the check fails")
                .contains("overlaps")
        );
    }

    #[test]
    fn refuses_an_entity_outside_the_tokens() {
        let parsed = file(vec![token(0, 1)], vec![entity((0, 2), None)]);
        assert!(
            check(&parsed, 4)
                .expect_err("the check fails")
                .contains("token range")
        );
    }

    #[test]
    fn refuses_a_container_after_its_child() {
        let parsed = file(
            vec![token(0, 1)],
            vec![entity((0, 1), Some(1)), entity((0, 1), None)],
        );
        assert!(
            check(&parsed, 4)
                .expect_err("the check fails")
                .contains("does not precede")
        );
    }

    #[test]
    fn refuses_a_child_outside_its_container() {
        let parsed = file(
            vec![token(0, 1), token(2, 3)],
            vec![entity((0, 1), None), entity((1, 2), Some(0))],
        );
        assert!(
            check(&parsed, 4)
                .expect_err("the check fails")
                .contains("not inside")
        );
    }

    #[test]
    fn refuses_a_body_start_outside_the_entity() {
        let mut function = entity((0, 1), None);
        function.body_start = Some(2);
        let parsed = file(vec![token(0, 1), token(2, 3)], vec![function]);
        assert!(
            check(&parsed, 4)
                .expect_err("the check fails")
                .contains("body start")
        );
    }

    #[test]
    fn refuses_overlapping_siblings() {
        let parsed = file(
            vec![token(0, 1), token(2, 3)],
            vec![entity((0, 2), None), entity((1, 2), None)],
        );
        assert!(
            check(&parsed, 4)
                .expect_err("the check fails")
                .contains("overlaps the entity")
        );
    }
}
