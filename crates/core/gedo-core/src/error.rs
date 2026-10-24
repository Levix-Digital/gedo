//! The error type of every storage port.

use std::path::PathBuf;

use crate::ids::{GitOid, ParseIdError};

/// A storage failure, with the operation and the path or reference involved.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The repository uses an object format Gedo does not support yet (SHA-256, FR-018).
    #[error("repository {repository} uses the {found} object format; Gedo supports only SHA-1 for now")]
    UnsupportedObjectFormat {
        /// The repository's path.
        repository: PathBuf,
        /// The object format found.
        found: String,
    },

    /// A write outside `refs/gedo/` was requested.
    #[error("reference {name} is outside refs/gedo/; Gedo writes only there")]
    NotInGedoNamespace {
        /// The refused name.
        name: String,
    },

    /// An identifier did not parse.
    #[error(transparent)]
    InvalidId(#[from] ParseIdError),

    /// A caller passed a value the operation cannot accept.
    #[error("{operation}: {reason}")]
    InvalidInput {
        /// The operation.
        operation: &'static str,
        /// What was wrong with the input.
        reason: String,
    },

    /// The object does not exist (or is not of the expected kind).
    #[error("object {oid} not found{}", if expected.is_empty() { String::new() } else { format!(" as a {expected}") })]
    ObjectNotFound {
        /// The missing object.
        oid: GitOid,
        /// The kind that was expected (`blob`, `tree`, `commit`), or empty.
        expected: &'static str,
    },

    /// No user identity is configured, so a commit cannot be written.
    #[error("no Git identity configured in {repository}; set user.name and user.email")]
    MissingIdentity {
        /// The repository's path.
        repository: PathBuf,
    },

    /// Another process holds the write-ahead log.
    #[error("write-ahead log {path} is busy: another Gedo process is writing to it")]
    WalBusy {
        /// The lock file.
        path: PathBuf,
    },

    /// Stored data is damaged.
    #[error("corrupted data in {path}{}: {reason}", offset.map(|offset| format!(" at offset {offset}")).unwrap_or_default())]
    Corrupted {
        /// The file or reference holding the data.
        path: String,
        /// The byte offset of the damage, when known.
        offset: Option<u64>,
        /// What was found.
        reason: String,
    },

    /// A record has a format version this build does not know.
    #[error("unsupported record format {found} in {path} (supported: {supported:?})")]
    UnsupportedFormat {
        /// The file or reference holding the record.
        path: String,
        /// The version found.
        found: u16,
        /// The versions this build reads.
        supported: Vec<u16>,
    },

    /// Data is larger than the limit for its kind.
    #[error("{what} of {size} bytes exceeds the limit of {limit} bytes")]
    TooLarge {
        /// What was being read or written.
        what: String,
        /// Its size.
        size: u64,
        /// The limit.
        limit: u64,
    },

    /// An operating system I/O failure.
    #[error("{operation} {}: {source}", path.display())]
    Io {
        /// The operation.
        operation: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },

    /// A failure inside a storage library (gitoxide, redb).
    #[error("{operation}: {message}")]
    Backend {
        /// The operation.
        operation: &'static str,
        /// The library's message.
        message: String,
    },
}

impl StoreError {
    /// Shorthand for [`StoreError::Io`].
    pub fn io(operation: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        StoreError::Io {
            operation,
            path: path.into(),
            source,
        }
    }

    /// Shorthand for [`StoreError::Backend`].
    pub fn backend(operation: &'static str, message: impl std::fmt::Display) -> Self {
        StoreError::Backend {
            operation,
            message: message.to_string(),
        }
    }

    /// Shorthand for [`StoreError::InvalidInput`].
    pub fn invalid_input(operation: &'static str, reason: impl Into<String>) -> Self {
        StoreError::InvalidInput {
            operation,
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_place_of_the_failure() {
        let error = StoreError::Corrupted {
            path: ".git/gedo/wal.log".into(),
            offset: Some(42),
            reason: "checksum mismatch".into(),
        };
        assert_eq!(
            error.to_string(),
            "corrupted data in .git/gedo/wal.log at offset 42: checksum mismatch"
        );

        let error = StoreError::io(
            "open",
            "/repo/.git/gedo/wal.log",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        );
        assert_eq!(error.to_string(), "open /repo/.git/gedo/wal.log: denied");
    }
}
