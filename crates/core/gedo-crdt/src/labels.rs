//! The text form of labels and agents, and the recording context (research R6).
//!
//! `GEDO_LABELS` holds `key=value` pairs separated by `,`; in a value, `\,` is a comma and `\\` a
//! backslash. `GEDO_AGENT` holds an agent identifier. Reading the environment belongs to the CLI.

use gedo_core::event::{AgentId, Author, AuthorKind, LabelError, LabelKey, LabelValue, Labels};

use crate::author::fingerprint;

/// Parses the text form of labels; an empty text is no labels.
///
/// Errors name the byte position in `text`. Reserved keys are accepted here; recording refuses
/// them through [`Labels::for_recording`].
pub fn parse_labels(text: &str) -> Result<Labels, LabelError> {
    let mut labels = Labels::new();
    if text.is_empty() {
        return Ok(labels);
    }
    for (start, pair) in split_pairs(text)? {
        let Some(equals) = pair.find('=') else {
            return Err(malformed(start, format!("{pair:?} is not key=value")));
        };
        let key = LabelKey::new(&pair[..equals]).map_err(|error| malformed(start, error.to_string()))?;
        let value = unescape(&pair[equals + 1..], start + equals + 1)?;
        let value = LabelValue::new(&key, &value)
            .map_err(|error| malformed(start + equals + 1, error.to_string()))?;
        labels
            .insert(key, value)
            .map_err(|error| malformed(start, error.to_string()))?;
    }
    Ok(labels)
}

/// The text form of `labels`, which [`parse_labels`] reads back unchanged.
pub fn format_labels(labels: &Labels) -> String {
    labels
        .iter()
        .map(|(key, value)| {
            let escaped = value.as_str().replace('\\', "\\\\").replace(',', "\\,");
            format!("{key}={escaped}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// The pairs of `text` with their byte offsets, split at commas that are not escaped.
fn split_pairs(text: &str) -> Result<Vec<(usize, &str)>, LabelError> {
    let mut pairs = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        match character {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            ',' => {
                pairs.push((start, &text[start..index]));
                start = index + 1;
            }
            _ => {}
        }
    }
    pairs.push((start, &text[start..]));
    if let Some((position, _)) = pairs.iter().find(|(_, pair)| pair.is_empty()) {
        return Err(malformed(*position, "empty pair".into()));
    }
    Ok(pairs)
}

fn unescape(value: &str, offset: usize) -> Result<String, LabelError> {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.char_indices();
    while let Some((index, character)) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        match characters.next() {
            Some((_, escaped @ ('\\' | ','))) => result.push(escaped),
            Some((_, other)) => {
                return Err(malformed(offset + index, format!("unknown escape \\{other}")));
            }
            None => return Err(malformed(offset + index, "a value ends with a lone \\".into())),
        }
    }
    Ok(result)
}

fn malformed(position: usize, reason: String) -> LabelError {
    LabelError::Malformed { position, reason }
}

/// The author kind and labels that apply to the events recorded while it is in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingContext {
    email: String,
    agent: Option<AgentId>,
    labels: Labels,
}

impl RecordingContext {
    /// A person working under the Git identity with `email`.
    pub fn person(email: &str, labels: Labels) -> Result<Self, LabelError> {
        Ok(RecordingContext {
            email: email.to_owned(),
            agent: None,
            labels: labels.for_recording()?,
        })
    }

    /// The agent `agent` working under the Git identity with `email`.
    pub fn agent(email: &str, agent: AgentId, labels: Labels) -> Result<Self, LabelError> {
        Ok(RecordingContext {
            email: email.to_owned(),
            agent: Some(agent),
            labels: labels.for_recording()?,
        })
    }

    /// The context named by the texts of `GEDO_AGENT` and `GEDO_LABELS` (absent or empty: none).
    pub fn from_texts(email: &str, agent: Option<&str>, labels: Option<&str>) -> Result<Self, LabelError> {
        let labels = parse_labels(labels.unwrap_or(""))?;
        match agent.filter(|agent| !agent.is_empty()) {
            Some(agent) => RecordingContext::agent(email, AgentId::new(agent)?, labels),
            None => RecordingContext::person(email, labels),
        }
    }

    /// The author of the events recorded under this context.
    pub fn author(&self) -> Author {
        Author {
            fingerprint: fingerprint(&self.email),
            kind: match &self.agent {
                Some(agent) => AuthorKind::Agent(agent.clone()),
                None => AuthorKind::Person,
            },
        }
    }

    /// The labels of the events recorded under this context.
    pub fn labels(&self) -> &Labels {
        &self.labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_are_parsed_with_escapes() {
        let labels = parse_labels(r"dc.plan=gedo_event_model,dc.milestone=M2,note=a\,b\\c").expect("valid");
        assert_eq!(labels.len(), 3);
        assert_eq!(labels.get("note").map(LabelValue::as_str), Some(r"a,b\c"));
        assert_eq!(parse_labels(&format_labels(&labels)).expect("valid"), labels);
        assert!(parse_labels("").expect("valid").is_empty());
        assert_eq!(
            parse_labels("k=a=b")
                .expect("valid")
                .get("k")
                .map(LabelValue::as_str),
            Some("a=b"),
            "only the first = separates"
        );
    }

    #[test]
    fn malformed_texts_name_the_position() {
        let cases = [
            ("k", 0),
            ("a=1,,b=2", 4),
            ("a=1,", 4),
            ("a=1,B=2", 4),
            ("a=", 2),
            (r"a=x\n", 3),
            (r"a=x\", 3),
            ("a=1,a=2", 4),
        ];
        for (text, position) in cases {
            match parse_labels(text) {
                Err(LabelError::Malformed { position: found, .. }) => {
                    assert_eq!(found, position, "{text}");
                }
                other => panic!("{text}: {other:?}"),
            }
        }
    }

    #[test]
    fn contexts_come_from_texts() {
        let person =
            RecordingContext::from_texts("levi@example.com", None, Some("dc.plan=p")).expect("valid");
        assert_eq!(person.author().kind, AuthorKind::Person);
        assert_eq!(person.author().fingerprint, fingerprint("levi@example.com"));
        assert_eq!(person.labels().len(), 1);

        let agent = RecordingContext::from_texts("levi@example.com", Some("worker-1"), None).expect("valid");
        assert_eq!(
            agent.author().kind,
            AuthorKind::Agent(AgentId::new("worker-1").expect("valid"))
        );
        assert_eq!(agent.author().fingerprint, person.author().fingerprint);
        assert_eq!(
            RecordingContext::from_texts("e@x", Some(""), None)
                .expect("valid")
                .author()
                .kind,
            AuthorKind::Person
        );

        assert!(RecordingContext::from_texts("e@x", Some("Bad Agent"), None).is_err());
        assert!(matches!(
            RecordingContext::from_texts("e@x", None, Some("gedo.stream=s")),
            Err(LabelError::Reserved { .. })
        ));
    }
}
