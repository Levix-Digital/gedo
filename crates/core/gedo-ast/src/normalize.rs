//! Separates significant syntax from trivia (research R4, R5).

use gedo_core::syntax::{NormalizedToken, ParsedFile, TokenClass};

/// For each token of the file, its normalized form, or `None` when it carries no meaning:
/// comments, optional punctuation, and a separator directly followed (trivia aside) by a closing
/// delimiter.
pub fn significant_tokens(file: &ParsedFile, source: &[u8]) -> Vec<Option<NormalizedToken>> {
    let mut normalized: Vec<Option<NormalizedToken>> = file
        .tokens
        .iter()
        .map(|token| {
            token.class.is_significant().then(|| NormalizedToken {
                class: token.class,
                text: normalize_text(&source[token.byte_range.as_usize()]),
            })
        })
        .collect();
    drop_trailing_separators(&mut normalized);
    normalized
}

/// The token's text with CRLF and lone CR turned into LF; invalid UTF-8 is replaced
/// deterministically.
fn normalize_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.into_owned()
    }
}

fn drop_trailing_separators(tokens: &mut [Option<NormalizedToken>]) {
    let mut next_class = None;
    for token in tokens.iter_mut().rev() {
        let Some(current) = token else { continue };
        let class = current.class;
        if class == TokenClass::Separator && next_class == Some(TokenClass::CloseDelimiter) {
            *token = None;
            continue;
        }
        next_class = Some(class);
    }
}

#[cfg(test)]
mod tests {
    use gedo_core::syntax::{ByteRange, Language, ParsedFile, Token, TokenClass};

    use super::significant_tokens;

    fn parse(classes_and_text: &[(TokenClass, &str)]) -> (ParsedFile, Vec<u8>) {
        let mut source = Vec::new();
        let mut tokens = Vec::new();
        for (class, text) in classes_and_text {
            let start = source.len() as u32;
            source.extend_from_slice(text.as_bytes());
            tokens.push(Token {
                class: *class,
                byte_range: ByteRange::new(start, source.len() as u32),
            });
            source.push(b' ');
        }
        let file = ParsedFile {
            language: Language::Rust,
            path: "a.rs".into(),
            unit: "".into(),
            tokens,
            entities: vec![],
            has_error: false,
        };
        (file, source)
    }

    fn texts(classes_and_text: &[(TokenClass, &str)]) -> Vec<String> {
        let (file, source) = parse(classes_and_text);
        significant_tokens(&file, &source)
            .into_iter()
            .flatten()
            .map(|token| token.text)
            .collect()
    }

    #[test]
    fn drops_comments_and_optional_punctuation() {
        use TokenClass::*;
        let kept = texts(&[
            (Identifier, "a"),
            (Comment, "// x"),
            (DocComment, "/// y"),
            (OptionalPunctuation, ","),
        ]);
        assert_eq!(kept, ["a"]);
    }

    #[test]
    fn drops_a_separator_before_a_closing_delimiter_even_across_comments() {
        use TokenClass::*;
        let kept = texts(&[
            (OpenDelimiter, "("),
            (Identifier, "a"),
            (Separator, ","),
            (Comment, "// last"),
            (CloseDelimiter, ")"),
        ]);
        assert_eq!(kept, ["(", "a", ")"]);
    }

    #[test]
    fn keeps_a_separator_between_items() {
        use TokenClass::*;
        let kept = texts(&[(Identifier, "a"), (Separator, ","), (Identifier, "b")]);
        assert_eq!(kept, ["a", ",", "b"]);
    }

    #[test]
    fn normalizes_line_endings_inside_tokens() {
        let kept = texts(&[(TokenClass::Literal, "\"a\r\nb\rc\"")]);
        assert_eq!(kept, ["\"a\nb\nc\""]);
    }
}
