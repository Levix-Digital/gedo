//! Identifiers shared by every port: object ids, collection entry ids and Gedo reference names.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// The hash function of a repository's object database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectHash {
    /// SHA-1, 20 bytes. The only format supported in this release.
    Sha1,
    /// SHA-256, 32 bytes. Detected and refused until SHA-256 support is delivered (FR-018).
    Sha256,
}

impl ObjectHash {
    /// Length in bytes of an object id of this hash.
    pub const fn len_bytes(self) -> usize {
        match self {
            ObjectHash::Sha1 => 20,
            ObjectHash::Sha256 => 32,
        }
    }

    /// Length in hexadecimal characters of an object id of this hash.
    pub const fn len_hex(self) -> usize {
        self.len_bytes() * 2
    }

    fn from_len_bytes(len: usize) -> Option<Self> {
        match len {
            20 => Some(ObjectHash::Sha1),
            32 => Some(ObjectHash::Sha256),
            _ => None,
        }
    }
}

impl fmt::Display for ObjectHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ObjectHash::Sha1 => "sha1",
            ObjectHash::Sha256 => "sha256",
        })
    }
}

const MAX_OID_BYTES: usize = 32;

/// The id of a Git object: its hash kind and its bytes.
///
/// Displayed and parsed as lowercase hexadecimal; ordered by its bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GitOid {
    hash: ObjectHash,
    bytes: [u8; MAX_OID_BYTES],
}

impl GitOid {
    /// Builds an id from raw bytes; the hash kind follows from the length (20 or 32).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ParseIdError> {
        let hash = ObjectHash::from_len_bytes(bytes.len()).ok_or_else(|| ParseIdError {
            kind: "object id",
            value: hex(bytes),
            reason: "must be 20 or 32 bytes",
        })?;
        let mut buffer = [0u8; MAX_OID_BYTES];
        buffer[..bytes.len()].copy_from_slice(bytes);
        Ok(GitOid { hash, bytes: buffer })
    }

    /// Parses a lowercase hexadecimal id of 40 (SHA-1) or 64 (SHA-256) characters.
    pub fn from_hex(text: &str) -> Result<Self, ParseIdError> {
        let invalid = |reason| ParseIdError {
            kind: "object id",
            value: text.to_owned(),
            reason,
        };
        if !is_lowercase_hex(text) {
            return Err(invalid("must be lowercase hexadecimal"));
        }
        if text.len() != ObjectHash::Sha1.len_hex() && text.len() != ObjectHash::Sha256.len_hex() {
            return Err(invalid("must be 40 or 64 characters"));
        }
        let bytes: Vec<u8> = text
            .as_bytes()
            .chunks(2)
            .map(|pair| (hex_value(pair[0]) << 4) | hex_value(pair[1]))
            .collect();
        GitOid::from_bytes(&bytes)
    }

    /// The hash kind of this id.
    pub fn object_hash(&self) -> ObjectHash {
        self.hash
    }

    /// The raw bytes of this id (20 or 32).
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.hash.len_bytes()]
    }

    /// Lowercase hexadecimal form.
    pub fn to_hex(&self) -> String {
        hex(self.as_bytes())
    }
}

impl Ord for GitOid {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl PartialOrd for GitOid {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for GitOid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for GitOid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GitOid({})", self.to_hex())
    }
}

impl serde::Serialize for GitOid {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.as_bytes())
    }
}

impl<'de> serde::Deserialize<'de> for GitOid {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        GitOid::from_bytes(&bytes).map_err(serde::de::Error::custom)
    }
}

impl FromStr for GitOid {
    type Err = ParseIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        GitOid::from_hex(text)
    }
}

/// Shortest entry id: two characters for the fan-out directory and at least two for the file name.
pub const MIN_ENTRY_ID_LEN: usize = 4;
/// Longest entry id, in characters.
pub const MAX_ENTRY_ID_LEN: usize = 128;
/// Characters of an entry id used as its fan-out directory.
pub const FAN_OUT_PREFIX_LEN: usize = 2;

/// The id of an entry in a collection: lowercase hexadecimal, 4 to 128 characters.
///
/// Lowercase-only ids keep fan-out paths distinct on case-insensitive file systems.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct EntryId(String);

impl EntryId {
    /// Parses an entry id.
    pub fn new(text: &str) -> Result<Self, ParseIdError> {
        let invalid = |reason| ParseIdError {
            kind: "entry id",
            value: text.to_owned(),
            reason,
        };
        if !is_lowercase_hex(text) {
            return Err(invalid("must be lowercase hexadecimal"));
        }
        if !(MIN_ENTRY_ID_LEN..=MAX_ENTRY_ID_LEN).contains(&text.len()) {
            return Err(invalid("must be 4 to 128 characters"));
        }
        Ok(EntryId(text.to_owned()))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The fan-out directory and the file name of this entry in a collection tree (`ab`, `cdef...`).
    pub fn fan_out(&self) -> (&str, &str) {
        self.0.split_at(FAN_OUT_PREFIX_LEN)
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Namespace of every reference Gedo writes.
pub const GEDO_REF_PREFIX: &str = "refs/gedo/";
/// Namespace `git fetch` mirrors remote Gedo references into; Gedo only reads it.
pub const GEDO_REMOTE_REF_PREFIX: &str = "refs/gedo-remote/";

/// A full reference name under `refs/gedo/`; the only kind of name Gedo writes.
///
/// After the prefix, components are non-empty and made of `[a-z0-9-]`, separated by `/`.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct GedoRefName(String);

impl GedoRefName {
    /// Validates a full name such as `refs/gedo/tapes`.
    pub fn new(name: &str) -> Result<Self, ParseIdError> {
        let invalid = |reason| ParseIdError {
            kind: "Gedo reference name",
            value: name.to_owned(),
            reason,
        };
        let rest = name
            .strip_prefix(GEDO_REF_PREFIX)
            .ok_or_else(|| invalid("must start with refs/gedo/"))?;
        validate_ref_components(rest).map_err(invalid)?;
        Ok(GedoRefName(name.to_owned()))
    }

    /// The name of a collection: `refs/gedo/<collection>`.
    pub fn collection(collection: &str) -> Result<Self, ParseIdError> {
        GedoRefName::new(&format!("{GEDO_REF_PREFIX}{collection}"))
    }

    /// The full name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The part after `refs/gedo/`, e.g. `tapes`.
    pub fn short_name(&self) -> &str {
        &self.0[GEDO_REF_PREFIX.len()..]
    }

    /// Whether `self` equals `prefix` or lies below it (`prefix/...`).
    pub fn starts_with(&self, prefix: &GedoRefName) -> bool {
        self.0 == prefix.0
            || (self.0.starts_with(prefix.as_str()) && self.0.as_bytes().get(prefix.0.len()) == Some(&b'/'))
    }
}

impl fmt::Display for GedoRefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validates a remote name used in `refs/gedo-remote/<remote>/`.
pub fn validate_remote_name(remote: &str) -> Result<(), ParseIdError> {
    let valid = !remote.is_empty()
        && remote != "."
        && remote != ".."
        && !remote.contains(['/', '\\', ' ', ':', '*', '?', '[', '~', '^'])
        && !remote.chars().any(char::is_control);
    if valid {
        Ok(())
    } else {
        Err(ParseIdError {
            kind: "remote name",
            value: remote.to_owned(),
            reason: "not a valid remote name",
        })
    }
}

fn validate_ref_components(rest: &str) -> Result<(), &'static str> {
    if rest.is_empty() {
        return Err("must name something after refs/gedo/");
    }
    for component in rest.split('/') {
        if component.is_empty() {
            return Err("must not contain empty components");
        }
        if component.contains("..") {
            return Err("must not contain ..");
        }
        if !component
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err("must use only a-z, 0-9, - and /");
        }
    }
    Ok(())
}

/// A text that is not a valid identifier of its kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} {value:?}: {reason}")]
pub struct ParseIdError {
    /// What was being parsed.
    pub kind: &'static str,
    /// The rejected text.
    pub value: String,
    /// Why it was rejected.
    pub reason: &'static str,
}

fn is_lowercase_hex(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        _ => digit - b'a' + 10,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[usize::from(byte >> 4)] as char);
        text.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn sha1_hex_round_trip() {
        let text = "0123456789abcdef0123456789abcdef01234567";
        let oid = GitOid::from_hex(text).expect("valid id");
        assert_eq!(oid.object_hash(), ObjectHash::Sha1);
        assert_eq!(oid.as_bytes().len(), 20);
        assert_eq!(oid.to_string(), text);
    }

    #[test]
    fn sha256_ids_are_recognized() {
        let oid = GitOid::from_hex(&"ab".repeat(32)).expect("valid id");
        assert_eq!(oid.object_hash(), ObjectHash::Sha256);
    }

    #[test]
    fn invalid_object_ids_are_refused() {
        assert!(GitOid::from_hex("").is_err());
        assert!(GitOid::from_hex(&"A".repeat(40)).is_err());
        assert!(GitOid::from_hex(&"a".repeat(41)).is_err());
        assert!(GitOid::from_hex(&"g".repeat(40)).is_err());
        assert!(GitOid::from_bytes(&[0; 19]).is_err());
    }

    #[test]
    fn ids_order_by_bytes() {
        let low = GitOid::from_hex(&format!("00{}", "f".repeat(38))).expect("valid id");
        let high = GitOid::from_hex(&format!("01{}", "0".repeat(38))).expect("valid id");
        assert!(low < high);
    }

    #[test]
    fn entry_ids_fan_out_after_two_characters() {
        let id = EntryId::new("abcdef").expect("valid id");
        assert_eq!(id.fan_out(), ("ab", "cdef"));
        assert!(EntryId::new("abc").is_err());
        assert!(EntryId::new("ABCD").is_err());
        assert!(EntryId::new(&"a".repeat(129)).is_err());
    }

    #[test]
    fn gedo_ref_names_stay_in_the_namespace() {
        let name = GedoRefName::new("refs/gedo/ast-map").expect("valid name");
        assert_eq!(name.short_name(), "ast-map");
        for refused in [
            "refs/heads/main",
            "refs/gedo/",
            "refs/gedo",
            "refs/gedo/Tapes",
            "refs/gedo/a//b",
            "refs/gedo/a/",
            "refs/gedo/../heads/main",
            "refs/gedo/a..b",
            "refs/gedo/a b",
            "refs/gedo/a_b",
            "refs/gedo-remote/origin/tapes",
        ] {
            assert!(GedoRefName::new(refused).is_err(), "{refused} must be refused");
        }
    }

    #[test]
    fn prefixes_match_whole_components() {
        let tapes = GedoRefName::new("refs/gedo/tapes").expect("valid name");
        assert!(
            GedoRefName::new("refs/gedo/tapes/x")
                .expect("valid name")
                .starts_with(&tapes)
        );
        assert!(tapes.starts_with(&tapes));
        assert!(
            !GedoRefName::new("refs/gedo/tapes-old")
                .expect("valid name")
                .starts_with(&tapes)
        );
    }

    #[test]
    fn remote_names_cannot_escape_their_namespace() {
        assert!(validate_remote_name("origin").is_ok());
        assert!(validate_remote_name("my-fork.2").is_ok());
        for refused in ["", ".", "..", "a/b", "a b", "a*"] {
            assert!(
                validate_remote_name(refused).is_err(),
                "{refused:?} must be refused"
            );
        }
    }

    proptest! {
        #[test]
        fn any_oid_bytes_round_trip_through_hex(bytes in prop_oneof![
            proptest::collection::vec(any::<u8>(), 20),
            proptest::collection::vec(any::<u8>(), 32),
        ]) {
            let oid = GitOid::from_bytes(&bytes).expect("valid length");
            let parsed = GitOid::from_hex(&oid.to_hex()).expect("own output parses");
            prop_assert_eq!(parsed, oid);
            prop_assert_eq!(parsed.as_bytes(), &bytes[..]);
        }

        #[test]
        fn valid_gedo_names_contain_only_allowed_characters(text in "\\PC{0,40}") {
            let name = format!("refs/gedo/{text}");
            if let Ok(parsed) = GedoRefName::new(&name) {
                prop_assert!(parsed.short_name().bytes().all(|byte| byte.is_ascii_lowercase()
                    || byte.is_ascii_digit() || byte == b'-' || byte == b'/'));
                prop_assert!(!parsed.as_str().contains(".."));
            }
        }

        #[test]
        fn hex_entry_ids_are_accepted(text in "[0-9a-f]{4,128}") {
            let id = EntryId::new(&text).expect("valid id");
            let (directory, file) = id.fan_out();
            prop_assert_eq!(format!("{directory}{file}"), text);
        }
    }
}
