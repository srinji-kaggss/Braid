//! Canonical encoding: a deterministic CBOR subset (D8).
//!
//! One value ⇒ exactly one byte string; one byte string ⇒ at most one value.
//! The strict decoder rejects every non-canonical form *and* `decode_strict`
//! additionally re-encodes and compares as a belt-and-braces **bijection
//! guard** (threat T3).

pub mod decode;
pub mod encode;
pub mod error;

pub use self::decode::decode_strict;
pub use self::encode::encode;
pub use self::error::{CanonError, MAX_DEPTH};

/// Canonical key order: length first, then bytewise — equals RFC 8949
/// deterministic encoding order for definite-length text keys.
pub fn key_cmp(a: &str, b: &str) -> core::cmp::Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.as_bytes().cmp(b.as_bytes()))
}
