//! The versioned envelope around every record Gedo stores (FR-011, FR-012).
//!
//! Everything read back from `refs/gedo/*` may come from another machine, so decoding checks the
//! size limit before parsing, refuses unknown format versions and never panics on malformed input.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

/// Largest encoded record accepted, in bytes (64 MiB).
pub const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;

/// A record's format version and its encoded payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordEnvelope {
	/// Format version of the payload.
	pub format: u16,
	/// The payload, encoded by the record's owner.
	pub payload: Vec<u8>,
}

impl RecordEnvelope {
	/// Wraps `value` encoded with postcard under `format`.
	pub fn wrap<T: Serialize>(format: u16, value: &T) -> Result<Self, StoreError> {
		let payload =
			postcard::to_allocvec(value).map_err(|error| StoreError::backend("encode record", error))?;
		Ok(RecordEnvelope { format, payload })
	}

	/// Decodes the payload as `T`; `source` names where the record came from, for errors.
	pub fn unwrap<T: DeserializeOwned>(&self, source: &str) -> Result<T, StoreError> {
		postcard::from_bytes(&self.payload).map_err(|error| corrupted(source, &error))
	}

	/// Encodes the envelope, refusing results larger than [`MAX_RECORD_BYTES`].
	pub fn encode(&self) -> Result<Vec<u8>, StoreError> {
		let bytes =
			postcard::to_allocvec(self).map_err(|error| StoreError::backend("encode record", error))?;
		check_size("record", bytes.len())?;
		Ok(bytes)
	}

	/// Decodes an envelope whose format is one of `supported`.
	///
	/// `source` names where the bytes came from (a file, a reference), for errors.
	pub fn decode(bytes: &[u8], supported: &[u16], source: &str) -> Result<Self, StoreError> {
		check_size(source, bytes.len())?;
		let (envelope, rest): (RecordEnvelope, &[u8]) =
			postcard::take_from_bytes(bytes).map_err(|error| corrupted(source, &error))?;
		if !rest.is_empty() {
			return Err(StoreError::Corrupted {
				path: source.to_owned(),
				offset: Some((bytes.len() - rest.len()) as u64),
				reason: format!("{} unexpected bytes after the record", rest.len()),
			});
		}
		if !supported.contains(&envelope.format) {
			return Err(StoreError::UnsupportedFormat {
				path: source.to_owned(),
				found: envelope.format,
				supported: supported.to_vec(),
			});
		}
		Ok(envelope)
	}
}

fn check_size(what: &str, size: usize) -> Result<(), StoreError> {
	if size > MAX_RECORD_BYTES {
		return Err(StoreError::TooLarge {
			what: what.to_owned(),
			size: size as u64,
			limit: MAX_RECORD_BYTES as u64,
		});
	}
	Ok(())
}

fn corrupted(source: &str, error: &postcard::Error) -> StoreError {
	StoreError::Corrupted {
		path: source.to_owned(),
		offset: None,
		reason: format!("malformed record: {error}"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use proptest::prelude::*;

	#[derive(Debug, PartialEq, Serialize, Deserialize)]
	struct Sample {
		name: String,
		count: u64,
	}

	#[test]
	fn typed_payloads_round_trip() {
		let sample = Sample {
			name: "invoice".into(),
			count: 3,
		};
		let envelope = RecordEnvelope::wrap(1, &sample).expect("encodes");
		let bytes = envelope.encode().expect("encodes");
		let decoded = RecordEnvelope::decode(&bytes, &[1], "test").expect("decodes");
		assert_eq!(decoded.unwrap::<Sample>("test").expect("decodes"), sample);
	}

	#[test]
	fn unknown_formats_are_refused() {
		let bytes = RecordEnvelope {
			format: 7,
			payload: vec![1, 2, 3],
		}
		.encode()
		.expect("encodes");
		let error = RecordEnvelope::decode(&bytes, &[1, 2], "refs/gedo/tapes").expect_err("refused");
		assert!(
			matches!(error, StoreError::UnsupportedFormat { found: 7, .. }),
			"{error}"
		);
	}

	#[test]
	fn oversized_input_is_refused_before_decoding() {
		let bytes = vec![0u8; MAX_RECORD_BYTES + 1];
		let error = RecordEnvelope::decode(&bytes, &[1], "big").expect_err("refused");
		assert!(matches!(error, StoreError::TooLarge { .. }), "{error}");
	}

	#[test]
	fn trailing_bytes_are_corruption() {
		let mut bytes = RecordEnvelope {
			format: 1,
			payload: vec![9],
		}
		.encode()
		.expect("encodes");
		bytes.push(0);
		let error = RecordEnvelope::decode(&bytes, &[1], "test").expect_err("refused");
		assert!(matches!(error, StoreError::Corrupted { .. }), "{error}");
	}

	proptest! {
		#[test]
		fn envelopes_round_trip(format in any::<u16>(), payload in proptest::collection::vec(any::<u8>(), 0..512)) {
			let envelope = RecordEnvelope { format, payload };
			let bytes = envelope.encode().expect("encodes");
			prop_assert_eq!(RecordEnvelope::decode(&bytes, &[format], "test").expect("decodes"), envelope);
		}

		#[test]
		fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
			let _outcome = RecordEnvelope::decode(&bytes, &[0, 1], "fuzz");
		}

		#[test]
		fn arbitrary_payloads_never_panic(payload in proptest::collection::vec(any::<u8>(), 0..512)) {
			let envelope = RecordEnvelope { format: 1, payload };
			let _outcome = envelope.unwrap::<Sample>("fuzz");
		}
	}
}
