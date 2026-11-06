//! The rules of virtual streams (`specs/009-virtual-streams/research.md` R6, R8): which streams a
//! filter keeps, and the recording context of the events recorded inside a stream or outside any.

use gedo_core::event::{AgentId, LabelError, LabelKey, LabelValue};
use gedo_core::stream::Stream;

use crate::labels::{RecordingContext, parse_labels};

/// Conditions on streams; every condition given must hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamFilter {
    /// Labels the stream must carry with exactly these values.
    pub labels: Vec<(LabelKey, LabelValue)>,
    /// The agent of the stream.
    pub agent: Option<AgentId>,
}

impl StreamFilter {
    /// The filter named by the text forms of labels (`key=value,...`, as `GEDO_LABELS`) and an
    /// agent identifier; absent or empty texts set no condition.
    pub fn from_texts(labels: Option<&str>, agent: Option<&str>) -> Result<Self, LabelError> {
        let labels = parse_labels(labels.unwrap_or(""))?
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let agent = agent
            .filter(|agent| !agent.is_empty())
            .map(AgentId::new)
            .transpose()?;
        Ok(StreamFilter { labels, agent })
    }

    /// Whether `stream` satisfies every condition.
    pub fn matches(&self, stream: &Stream) -> bool {
        let labels = self
            .labels
            .iter()
            .all(|(key, value)| stream.labels.get(key.as_str()) == Some(value));
        let agent = self
            .agent
            .as_ref()
            .is_none_or(|wanted| stream.agent.as_ref() == Some(wanted));
        labels && agent
    }
}

/// The context of the events recorded inside `stream` by the Git identity with `email`: the
/// stream's agent (or the person) and the stream's labels. Refused only for a stream whose labels
/// use the reserved prefix, which creation and reading never let through.
pub fn stream_context(stream: &Stream, email: &str) -> Result<RecordingContext, LabelError> {
    let labels = stream.labels.clone();
    match &stream.agent {
        Some(agent) => RecordingContext::agent(email, agent.clone(), labels),
        None => RecordingContext::person(email, labels),
    }
}

/// The recording context: the active stream's when one is given, otherwise the one named by the
/// texts of `GEDO_AGENT` and `GEDO_LABELS`, which are ignored inside a stream.
pub fn recording_context(
    active: Option<&Stream>,
    email: &str,
    agent: Option<&str>,
    labels: Option<&str>,
) -> Result<RecordingContext, LabelError> {
    match active {
        Some(stream) => stream_context(stream, email),
        None => RecordingContext::from_texts(email, agent, labels),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gedo_core::event::{AuthorKind, Labels};
    use gedo_core::stream::StreamName;
    use proptest::prelude::*;

    fn stream(name: &str, agent: Option<&str>, labels: &[(&str, &str)]) -> Stream {
        Stream::new(
            StreamName::new(name).expect("name"),
            agent.map(|agent| AgentId::new(agent).expect("agent")),
            Labels::from_pairs(labels).expect("labels"),
            None,
        )
    }

    #[test]
    fn filters_need_every_label_and_the_agent() {
        let milestone = stream(
            "dc/m2",
            Some("implementer"),
            &[("dc.plan", "p"), ("dc.milestone", "2")],
        );
        let person = stream("mine", None, &[("dc.plan", "p")]);
        let plan = StreamFilter::from_texts(Some("dc.plan=p"), None).expect("filter");
        assert!(plan.matches(&milestone) && plan.matches(&person));
        let both = StreamFilter::from_texts(Some("dc.plan=p,dc.milestone=2"), None).expect("filter");
        assert!(both.matches(&milestone) && !both.matches(&person));
        let other = StreamFilter::from_texts(Some("dc.plan=q"), None).expect("filter");
        assert!(!other.matches(&milestone));
        let agent = StreamFilter::from_texts(None, Some("implementer")).expect("filter");
        assert!(agent.matches(&milestone) && !agent.matches(&person));
        assert!(StreamFilter::default().matches(&person));
        assert!(StreamFilter::from_texts(Some("x"), None).is_err());
        assert!(StreamFilter::from_texts(None, Some("Bad")).is_err());
    }

    #[test]
    fn inside_a_stream_its_context_wins() {
        let milestone = stream("dc/m2", Some("implementer"), &[("dc.plan", "p")]);
        let context =
            recording_context(Some(&milestone), "a@b.c", Some("other"), Some("x=1")).expect("context");
        assert_eq!(
            context.author().kind,
            AuthorKind::Agent(AgentId::new("implementer").expect("agent"))
        );
        assert_eq!(context.labels(), &milestone.labels);

        let outside = recording_context(None, "a@b.c", Some("other"), Some("x=1")).expect("context");
        assert_eq!(
            outside,
            RecordingContext::from_texts("a@b.c", Some("other"), Some("x=1")).expect("context")
        );
        let person = recording_context(None, "a@b.c", None, None).expect("context");
        assert_eq!(person.author().kind, AuthorKind::Person);
        assert!(recording_context(None, "a@b.c", None, Some("gedo.x=1")).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn a_streams_context_carries_its_agent_and_labels(
            agent in proptest::option::of("[a-z][a-z0-9._-]{0,20}"),
            pairs in proptest::collection::btree_map("dc\\.[a-z]{1,8}", "[ -~]{1,20}", 0..16),
            email in "[a-z]{1,8}@[a-z]{1,8}\\.org",
        ) {
            let pairs: Vec<(String, String)> = pairs.into_iter().collect();
            let stream = Stream::new(
                StreamName::new("s").expect("name"),
                agent.as_deref().map(|agent| AgentId::new(agent).expect("agent")),
                Labels::from_pairs(&pairs).expect("labels"),
                None,
            );
            let context = stream_context(&stream, &email).expect("context");
            let expected = match &stream.agent {
                Some(agent) => AuthorKind::Agent(agent.clone()),
                None => AuthorKind::Person,
            };
            prop_assert_eq!(context.author().kind, expected);
            prop_assert_eq!(context.author().fingerprint, crate::fingerprint(&email));
            prop_assert_eq!(context.labels(), &stream.labels);
        }
    }
}
