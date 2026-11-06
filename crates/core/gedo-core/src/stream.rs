//! Virtual streams (FEAT-068, `specs/009-virtual-streams/research.md` R3, R4): named units of work
//! kept on the developer's machine, with a base commit, an author kind, labels set at creation, a
//! saved work tree and their own pending events.
//!
//! This module holds the model and its stored forms; the rules are in `gedo-crdt::streams`, the use
//! cases in `gedo-app::streams` and the storage behind [`crate::ports::StreamStore`].

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::StoreError;
use crate::event::{AgentId, Labels};
use crate::ids::{GitOid, ParseIdError};
use crate::record::RecordEnvelope;

/// Largest stream name, in bytes.
pub const MAX_STREAM_NAME_BYTES: usize = 64;
/// Format of stream records.
pub const STREAM_FORMAT: u16 = 1;
/// Format of working tree states.
pub const WORKTREE_STATE_FORMAT: u16 = 1;

/// The name of a stream: 1 to 64 bytes of `/`-separated segments, each non-empty, of
/// `[a-z0-9._-]`, starting with a letter or digit, and not `.` or `..` (research R3).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StreamName(String);

impl StreamName {
    /// Validates a stream name.
    pub fn new(name: &str) -> Result<Self, ParseIdError> {
        let invalid = |reason| ParseIdError {
            kind: "stream name",
            value: name.to_owned(),
            reason,
        };
        if name.is_empty() || name.len() > MAX_STREAM_NAME_BYTES {
            return Err(invalid("must be 1 to 64 bytes"));
        }
        for segment in name.split('/') {
            let bytes = segment.as_bytes();
            let Some(first) = bytes.first() else {
                return Err(invalid("must not have an empty segment"));
            };
            if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
                return Err(invalid("each segment must start with a-z or 0-9"));
            }
            if !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
            }) {
                return Err(invalid("segments may hold only a-z, 0-9, '.', '-' and '_'"));
            }
        }
        Ok(StreamName(name.to_owned()))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The hexadecimal form of the name's bytes: a storage key valid on every file system.
    pub fn key(&self) -> String {
        self.0.bytes().map(|byte| format!("{byte:02x}")).collect()
    }

    /// The name stored under `key` (the inverse of [`StreamName::key`]).
    pub fn from_key(key: &str) -> Result<Self, ParseIdError> {
        let invalid = || ParseIdError {
            kind: "stream key",
            value: key.to_owned(),
            reason: "must be the lowercase hexadecimal form of a stream name",
        };
        if !key.len().is_multiple_of(2) || !key.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(invalid());
        }
        let bytes = (0..key.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&key[index..index + 2], 16).map_err(|_| invalid()))
            .collect::<Result<Vec<u8>, _>>()?;
        let text = String::from_utf8(bytes).map_err(|_| invalid())?;
        StreamName::new(&text)
    }
}

impl fmt::Display for StreamName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for StreamName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StreamName({})", self.0)
    }
}

/// A virtual stream. Name, agent and labels are set at creation and never change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream {
    /// The stream's name.
    pub name: StreamName,
    /// The agent working in the stream; `None` for the person of the Git identity.
    pub agent: Option<AgentId>,
    /// The labels every event recorded inside the stream carries; no key is reserved.
    pub labels: Labels,
    /// The commit the stream's work is on; `None` for an empty history.
    pub base: Option<GitOid>,
    /// The saved work tree; `None` when the work is the base's tree.
    pub work: Option<GitOid>,
    /// The commit of a publish in progress (research R9).
    pub publishing: Option<GitOid>,
}

impl Stream {
    /// A new stream on `base`, with no saved work.
    pub fn new(name: StreamName, agent: Option<AgentId>, labels: Labels, base: Option<GitOid>) -> Self {
        Stream {
            name,
            agent,
            labels,
            base,
            work: None,
            publishing: None,
        }
    }
}

/// The streams state of one working tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorktreeState {
    /// The stream the working tree is in.
    pub active: Option<StreamName>,
    /// The work saved when the working tree entered a stream from outside any stream.
    pub outside: Option<GitOid>,
}

#[derive(Serialize, Deserialize)]
struct StoredStream {
    name: StreamName,
    agent: Option<AgentId>,
    labels: Labels,
    base: Option<Vec<u8>>,
    work: Option<Vec<u8>>,
    publishing: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
struct StoredState {
    active: Option<StreamName>,
    outside: Option<Vec<u8>>,
}

fn oid_bytes(oid: Option<GitOid>) -> Option<Vec<u8>> {
    oid.map(|oid| oid.as_bytes().to_vec())
}

fn oid_from(bytes: Option<Vec<u8>>, source: &str) -> Result<Option<GitOid>, StoreError> {
    bytes
        .map(|bytes| GitOid::from_bytes(&bytes).map_err(|error| refused(source, error.to_string())))
        .transpose()
}

fn refused(source: &str, reason: String) -> StoreError {
    StoreError::Corrupted {
        path: source.to_owned(),
        offset: None,
        reason,
    }
}

/// The stored form of a stream record.
pub fn encode_stream(stream: &Stream) -> Result<Vec<u8>, StoreError> {
    RecordEnvelope::wrap(
        STREAM_FORMAT,
        &StoredStream {
            name: stream.name.clone(),
            agent: stream.agent.clone(),
            labels: stream.labels.clone(),
            base: oid_bytes(stream.base),
            work: oid_bytes(stream.work),
            publishing: oid_bytes(stream.publishing),
        },
    )?
    .encode()
}

/// Reads the record of the stream `name`, checking its format, its name and its labels.
pub fn decode_stream(name: &StreamName, bytes: &[u8]) -> Result<Stream, StoreError> {
    let source = format!("stream {name}");
    let stored: StoredStream = RecordEnvelope::decode(bytes, &[STREAM_FORMAT], &source)?.unwrap(&source)?;
    if stored.name != *name {
        return Err(refused(
            &source,
            format!("the record is for stream {}", stored.name),
        ));
    }
    StreamName::new(stored.name.as_str()).map_err(|error| refused(&source, error.to_string()))?;
    if let Some(agent) = &stored.agent {
        AgentId::new(agent.as_str()).map_err(|error| refused(&source, error.to_string()))?;
    }
    stored
        .labels
        .check()
        .and_then(|()| stored.labels.clone().for_recording())
        .map_err(|error| refused(&source, error.to_string()))?;
    Ok(Stream {
        name: stored.name,
        agent: stored.agent,
        labels: stored.labels,
        base: oid_from(stored.base, &source)?,
        work: oid_from(stored.work, &source)?,
        publishing: oid_from(stored.publishing, &source)?,
    })
}

/// The stored form of a working tree state.
pub fn encode_state(state: &WorktreeState) -> Result<Vec<u8>, StoreError> {
    RecordEnvelope::wrap(
        WORKTREE_STATE_FORMAT,
        &StoredState {
            active: state.active.clone(),
            outside: oid_bytes(state.outside),
        },
    )?
    .encode()
}

/// Reads a working tree state.
pub fn decode_state(bytes: &[u8]) -> Result<WorktreeState, StoreError> {
    let source = "stream state";
    let stored: StoredState =
        RecordEnvelope::decode(bytes, &[WORKTREE_STATE_FORMAT], source)?.unwrap(source)?;
    if let Some(active) = &stored.active {
        StreamName::new(active.as_str()).map_err(|error| refused(source, error.to_string()))?;
    }
    Ok(WorktreeState {
        active: stored.active,
        outside: oid_from(stored.outside, source)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn oid(byte: u8) -> GitOid {
        GitOid::from_bytes(&[byte; 20]).expect("oid")
    }

    #[test]
    fn valid_names_are_accepted() {
        for name in ["a", "dc/m2", "agent/metrics", "feat-retry", "0.x_y", "a/b/c"] {
            assert_eq!(StreamName::new(name).expect(name).as_str(), name);
        }
        assert!(StreamName::new(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn invalid_names_are_refused() {
        for name in [
            "", "Dc/M2", "a//b", "/a", "a/", "-a", "a b", ".", "a/..", "a/.b", "é", "a\\b",
        ] {
            assert!(StreamName::new(name).is_err(), "{name:?} should be refused");
        }
        assert!(StreamName::new(&"a".repeat(65)).is_err());
    }

    #[test]
    fn keys_round_trip() {
        let name = StreamName::new("dc/m2").expect("name");
        assert_eq!(name.key(), "64632f6d32");
        assert_eq!(StreamName::from_key(&name.key()).expect("key"), name);
        assert!(StreamName::from_key("2f").is_err());
        assert!(StreamName::from_key("ABCD").is_err());
        assert!(StreamName::from_key("6").is_err());
    }

    #[test]
    fn records_round_trip() {
        let mut stream = Stream::new(
            StreamName::new("dc/m2").expect("name"),
            Some(AgentId::new("implementer").expect("agent")),
            Labels::from_pairs(&[("dc.plan", "p"), ("dc.milestone", "2")]).expect("labels"),
            Some(oid(1)),
        );
        stream.work = Some(oid(2));
        stream.publishing = Some(oid(3));
        let bytes = encode_stream(&stream).expect("encode");
        assert_eq!(decode_stream(&stream.name, &bytes).expect("decode"), stream);

        let state = WorktreeState {
            active: Some(stream.name.clone()),
            outside: Some(oid(4)),
        };
        assert_eq!(
            decode_state(&encode_state(&state).expect("encode")).expect("decode"),
            state
        );
        let empty = WorktreeState::default();
        assert_eq!(
            decode_state(&encode_state(&empty).expect("encode")).expect("decode"),
            empty
        );
    }

    #[test]
    fn bad_records_are_refused() {
        let name = StreamName::new("a").expect("name");
        let stream = Stream::new(name.clone(), None, Labels::new(), None);
        let bytes = encode_stream(&stream).expect("encode");
        assert!(decode_stream(&StreamName::new("b").expect("name"), &bytes).is_err());
        assert!(decode_stream(&name, &bytes[..bytes.len() - 1]).is_err());
        assert!(decode_stream(&name, b"garbage").is_err());
        let wrong_format = RecordEnvelope {
            format: 9,
            payload: vec![],
        }
        .encode()
        .expect("encode");
        assert!(decode_stream(&name, &wrong_format).is_err());

        let reserved = Stream::new(
            name.clone(),
            None,
            Labels::from_pairs(&[("gedo.x", "1")]).expect("labels"),
            None,
        );
        let bytes = encode_stream(&reserved).expect("encode");
        assert!(decode_stream(&name, &bytes).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn generated_names_follow_the_rule(text in "[a-z0-9./_-]{0,70}") {
            let expected = !text.is_empty()
                && text.len() <= MAX_STREAM_NAME_BYTES
                && text.split('/').all(|segment| {
                    segment.as_bytes().first().is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
                });
            let result = StreamName::new(&text);
            prop_assert_eq!(result.is_ok(), expected);
            if let Ok(name) = result {
                prop_assert_eq!(StreamName::from_key(&name.key()).expect("key"), name);
            }
        }
    }
}
