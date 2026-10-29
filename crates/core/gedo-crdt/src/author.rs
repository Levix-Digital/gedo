//! The author fingerprint (research R4).

use gedo_core::event::AuthorFingerprint;

/// Key-derivation context of fingerprints. Never change it: every fingerprint would change.
const AUTHOR_CONTEXT_V1: &str = "gedo author v1";

/// The fingerprint of a person's Git e-mail: ASCII whitespace trimmed and ASCII letters
/// lowercased, so one person keeps one fingerprint; the e-mail itself is never stored.
pub fn fingerprint(email: &str) -> AuthorFingerprint {
    let normalized = email
        .trim_matches(|character: char| character.is_ascii_whitespace())
        .to_ascii_lowercase();
    AuthorFingerprint(blake3::derive_key(AUTHOR_CONTEXT_V1, normalized.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_and_surrounding_whitespace_do_not_change_the_fingerprint() {
        let base = fingerprint("levi@example.com");
        assert_eq!(fingerprint(" Levi@Example.COM\t"), base);
        assert_ne!(fingerprint("levi@example.org"), base);
        assert_ne!(fingerprint("le vi@example.com"), base, "inner spaces are kept");
    }
}
