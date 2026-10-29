//! Event filters (research R12).

use gedo_core::event::{AgentId, AuthorFingerprint, AuthorKind, Event, LabelKey, LabelValue};

/// The kind of author a filter asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorKindFilter {
    /// Only people.
    Person,
    /// Only agents.
    Agent,
}

/// Conditions on events; every condition given must hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventFilter {
    /// Labels the event must carry with exactly these values.
    pub labels: Vec<(LabelKey, LabelValue)>,
    /// The kind of author.
    pub author_kind: Option<AuthorKindFilter>,
    /// The agent that made the event.
    pub agent: Option<AgentId>,
    /// The author's fingerprint.
    pub fingerprint: Option<AuthorFingerprint>,
}

impl EventFilter {
    /// Whether `event` satisfies every condition.
    pub fn matches(&self, event: &Event) -> bool {
        let labels = self
            .labels
            .iter()
            .all(|(key, value)| event.labels.get(key.as_str()) == Some(value));
        let kind = match (self.author_kind, &event.author.kind) {
            (None, _)
            | (Some(AuthorKindFilter::Person), AuthorKind::Person)
            | (Some(AuthorKindFilter::Agent), AuthorKind::Agent(_)) => true,
            (Some(_), _) => false,
        };
        let agent = match (&self.agent, &event.author.kind) {
            (None, _) => true,
            (Some(wanted), AuthorKind::Agent(agent)) => wanted == agent,
            (Some(_), AuthorKind::Person) => false,
        };
        let fingerprint = self
            .fingerprint
            .is_none_or(|wanted| wanted == event.author.fingerprint);
        labels && kind && agent && fingerprint
    }
}
